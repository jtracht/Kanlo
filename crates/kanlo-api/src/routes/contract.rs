use super::*;
use kanlo_core::{Board, Column, Ticket, ViewState, tv};
use kanlo_db::{
    Mutation, Preview, Receipt,
    recovery::{Backup, BoardExport, RecoveryPreview},
    synchronization::{OfflineComment, OfflineCreate, SyncResponse},
};
use schemars::JsonSchema;
use serde_json::Map;
use std::sync::OnceLock;

static OPENAPI: OnceLock<Value> = OnceLock::new();

pub(super) async fn openapi() -> Json<Value> {
    Json(OPENAPI.get_or_init(document).clone())
}

fn rewrite_references(value: &mut Value, scope: &str) {
    match value {
        Value::Object(fields) => {
            if let Some(reference) = fields.get("$ref").and_then(Value::as_str)
                && let Some(name) = reference.strip_prefix("#/$defs/")
            {
                let updated = format!("#/components/schemas/{scope}__{name}");
                fields.insert("$ref".into(), Value::String(updated));
            }
            for child in fields.values_mut() {
                rewrite_references(child, scope);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_references(item, scope);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Contract {
    paths: Map<String, Value>,
    schemas: Map<String, Value>,
}

impl Contract {
    fn schema<T: JsonSchema>(&mut self, name: &str) -> Value {
        let mut root = json!(schemars::schema_for!(T));
        if let Some(root) = root.as_object_mut() {
            root.remove("$schema");
            if let Some(Value::Object(definitions)) = root.remove("$defs") {
                for (key, mut value) in definitions {
                    rewrite_references(&mut value, name);
                    self.schemas.insert(format!("{name}__{key}"), value);
                }
            }
        }
        rewrite_references(&mut root, name);
        self.schemas.insert(name.into(), root);
        json!({"$ref": format!("#/components/schemas/{name}")})
    }

    fn add(
        &mut self,
        path: &str,
        method: &str,
        id: &str,
        request: Option<Value>,
        parameters: Option<Value>,
        response: Value,
    ) {
        let mut entries = Vec::new();
        for segment in path.split('/') {
            if let Some(name) = segment
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
            {
                entries.push(
                    json!({"in":"path","name":name,"required":true,"schema":{"type":"string"}}),
                );
            }
        }
        if let Some(schema) = parameters
            && let Some(fields) = schema.get("properties").and_then(Value::as_object)
        {
            for (name, property) in fields {
                let required = schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(name)));
                entries
                    .push(json!({"in":"query","name":name,"required":required,"schema":property}));
            }
        }
        if matches!(
            id,
            "createBoard" | "createColumn" | "createTicket" | "moveTicket"
        ) {
            for name in [
                "Idempotency-Key",
                "X-Kanlo-Client",
                "X-Kanlo-Instance",
                "X-Kanlo-Epoch",
            ] {
                entries.push(
                    json!({"in":"header","name":name,"required":true,"schema":{"type":"string"}}),
                );
            }
        }
        let (status, media) = match id {
            "createBoard" | "createColumn" | "createTicket" => ("201", "application/json"),
            "subscribeToEvents" => ("200", "text/event-stream"),
            "exportBoardCsv" => ("200", "text/csv"),
            _ => ("200", "application/json"),
        };
        let mut operation = json!({
            "operationId":id,
            "parameters":entries,
            "responses":{
                (status):{"description":"Successful response","content":{(media):{"schema":response}}},
                "default":{"description":"Request rejected or operation failed","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ApiError"}}}}
            }
        });
        if let Some(body) = request {
            operation["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":body}}});
        }
        if id == "verifyIntegration" {
            operation["security"] = json!([{"bearerKey":[]}]);
        }
        self.paths.entry(path).or_insert_with(|| json!({}))[method] = operation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn contract_covers_all_versioned_routes_and_references_resolve() {
        let document = document();
        let paths = document["paths"].as_object().unwrap();
        let mut expected = BTreeSet::new();
        for source in [include_str!("../routes.rs"), include_str!("../lib.rs")] {
            for route in source.split(".route(").skip(1) {
                let Some(path) = route
                    .split('"')
                    .nth(1)
                    .filter(|path| path.starts_with("/api/v1/"))
                else {
                    continue;
                };
                let mut depth = 1;
                let end = route
                    .char_indices()
                    .find_map(|(index, ch)| {
                        match ch {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        (depth == 0).then_some(index)
                    })
                    .unwrap();
                let route = &route[..end];
                for method in ["get", "post"] {
                    if route.contains(&format!("{method}(")) {
                        expected.insert((path.to_owned(), method.to_owned()));
                    }
                }
            }
        }
        let actual = paths
            .iter()
            .flat_map(|(path, methods)| {
                methods
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(move |method| (path.clone(), method.clone()))
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
        fn verify(value: &Value, root: &Value) {
            match value {
                Value::Object(fields) => {
                    if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
                        let pointer = reference.strip_prefix('#').unwrap();
                        assert!(root.pointer(pointer).is_some(), "{reference}");
                    }
                    fields.values().for_each(|value| verify(value, root));
                }
                Value::Array(items) => items.iter().for_each(|value| verify(value, root)),
                _ => {}
            }
        }
        verify(&document, &document);
        let serialized = document.to_string();
        for expected in [
            "patch_ticket",
            "set_settings",
            "show_ticket",
            "expires_at",
            "base_epoch",
        ] {
            assert!(serialized.contains(expected), "{expected}");
        }
    }
}

// These projections describe the compact JSON views returned by the read handlers.
#[allow(dead_code)]
mod responses {
    use super::*;
    use kanlo_core::{
        Comment, Workspace,
        model::{BoardOptions, Label, Settings, TicketType},
    };

    #[derive(JsonSchema)]
    pub struct Status {
        pub status: String,
        pub version: String,
    }

    #[derive(JsonSchema)]
    pub struct Health {
        #[serde(flatten)]
        pub status: Status,
        pub instance_id: String,
        pub epoch: String,
    }

    #[derive(JsonSchema)]
    pub struct Bootstrap {
        pub boards: Vec<Board>,
        pub types: Vec<TicketType>,
        pub labels: Vec<Label>,
        pub settings: Settings,
        pub board_options: BTreeMap<String, BoardOptions>,
        pub views: BTreeMap<String, ViewState>,
        pub revision: i64,
        pub event_cursor: i64,
        pub instance_id: String,
        pub epoch: String,
        pub passcode_configured: bool,
    }

    #[derive(JsonSchema)]
    pub struct BoardSummary {
        pub board_id: String,
        pub name: String,
        pub open: usize,
        pub done: usize,
        pub backlog: usize,
        pub blocked: usize,
    }

    #[derive(JsonSchema)]
    pub struct ExternalTicket {
        pub id: String,
        pub public_id: String,
        pub done: bool,
        pub trashed: bool,
    }

    #[derive(JsonSchema)]
    pub struct Snapshot {
        pub workspace: Workspace,
        pub external_references: Vec<ExternalTicket>,
        pub revision: i64,
        pub view_states: BTreeMap<String, ViewState>,
        pub settings: Settings,
        pub epoch: String,
    }

    #[derive(JsonSchema)]
    pub struct Page<T> {
        pub items: Vec<T>,
        pub next_offset: Option<usize>,
    }

    #[derive(JsonSchema)]
    pub struct TicketSummary {
        pub id: String,
        pub public_id: String,
        pub board_id: String,
        pub column_id: Option<String>,
        pub title: String,
        pub importance: u8,
        pub done: bool,
        pub revision: i64,
    }

    macro_rules! response {
        ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
            #[derive(JsonSchema)]
            pub struct $name { $(pub $field: $ty),* }
        };
    }

    response!(UndoEntry {
        id: i64,
        description: String,
        created_at: String
    });
    response!(SafetyBackup {
        id: String,
        created_at: String
    });
    response!(Handshake {
        instance_id: String,
        epoch: String,
        revision: i64,
        authenticated: bool
    });
    response!(Category {
        id: String,
        name: String
    });
    response!(Categories { categories: Vec<Category> });
    response!(SettingNames { names: Vec<String>, next_offset: Option<usize> });
    response!(SettingValues {
        values: BTreeMap<String, Value>,
        next_offset: Option<usize>,
        board_options: Option<BTreeMap<String, BoardOptions>>
    });
    response!(Login {
        authenticated: bool,
        csrf: String,
        expires_at: String
    });
    response!(Logout {
        authenticated: bool
    });
    response!(LocalSession {
        authenticated: bool,
        local: bool
    });
    response!(ExternalSession {
        authenticated: bool,
        csrf: String
    });
    response!(Configured { configured: bool });
    response!(Key {
        id: String,
        name: String,
        created_at: String,
        revoked: bool
    });
    response!(NewKey {
        id: String,
        name: String,
        token: String,
        shown_once: bool
    });
    response!(Revoked { revoked: bool });

    #[derive(JsonSchema)]
    #[serde(untagged)]
    pub enum SettingsPage {
        Categories(Categories),
        Names(SettingNames),
        Values(SettingValues),
    }

    #[derive(JsonSchema)]
    #[serde(untagged)]
    pub enum SessionStatus {
        Local(LocalSession),
        External(ExternalSession),
    }

    pub type Comments = Page<Comment>;
}

fn document() -> Value {
    let mut contract = Contract::default();
    macro_rules! read {
        ($path:literal, $name:literal, $response:ty) => {{
            let response = contract.schema::<$response>(concat!($name, "Response"));
            contract.add($path, "get", $name, None, None, response);
        }};
        ($path:literal, $name:literal, $query:ty, $response:ty) => {{
            let response = contract.schema::<$response>(concat!($name, "Response"));
            let query_ref = contract.schema::<$query>(concat!($name, "Query"));
            let query = query_ref["$ref"]
                .as_str()
                .and_then(|path| path.rsplit('/').next())
                .and_then(|name| contract.schemas.get(name))
                .cloned();
            contract.add($path, "get", $name, None, query, response);
        }};
    }
    macro_rules! write {
        ($path:literal, $name:literal, (), $response:ty) => {{
            let response = contract.schema::<$response>(concat!($name, "Response"));
            contract.add($path, "post", $name, None, None, response);
        }};
        ($path:literal, $name:literal, $body:ty, $response:ty) => {{
            let request = contract.schema::<$body>(concat!($name, "Request"));
            let response = contract.schema::<$response>(concat!($name, "Response"));
            contract.add($path, "post", $name, Some(request), None, response);
        }};
    }
    read!("/api/v1/openapi.json", "getOpenApi", Value);
    read!("/api/v1/health", "getHealth", responses::Health);
    read!("/api/v1/bootstrap", "bootstrap", responses::Bootstrap);
    read!("/api/v1/display", "getDisplay", tv::DisplayState);
    write!(
        "/api/v1/display",
        "controlDisplay",
        tv::Action,
        tv::DisplayState
    );
    read!(
        "/api/v1/overview",
        "getOverview",
        Vec<responses::BoardSummary>
    );
    read!(
        "/api/v1/integrations/verify",
        "verifyIntegration",
        responses::Status
    );
    read!("/api/v1/boards", "listBoards", Vec<Board>);
    write!("/api/v1/boards", "createBoard", NewBoard, Board);
    read!("/api/v1/boards/{id}/columns", "listColumns", Vec<Column>);
    write!(
        "/api/v1/boards/{id}/columns",
        "createColumn",
        NewColumn, Column
    );
    read!("/api/v1/boards/{id}/tickets", "listTickets", Vec<Ticket>);
    write!(
        "/api/v1/boards/{id}/tickets",
        "createTicket",
        NewTicket, Ticket
    );
    read!(
        "/api/v1/boards/{id}/snapshot",
        "getBoardSnapshot",
        responses::Snapshot
    );
    read!("/api/v1/tickets/{id}", "getTicket", Ticket);
    write!(
        "/api/v1/tickets/{id}/move",
        "moveTicket",
        MoveTicket, Ticket
    );
    read!(
        "/api/v1/tickets/{id}/comments",
        "listComments",
        CommentPage,
        responses::Comments
    );
    write!("/api/v1/commands", "executeCommand", Mutation, Receipt);
    write!(
        "/api/v1/commands/preview",
        "previewCommand",
        PreviewRequest, Preview
    );
    write!(
        "/api/v1/commands/confirm",
        "confirmCommand",
        Confirmation, Receipt
    );
    write!("/api/v1/search", "searchWithView", ViewState, Vec<Ticket>);
    read!(
        "/api/v1/ticket-search",
        "searchTickets",
        TicketSearch,
        responses::Page<responses::TicketSummary>
    );
    read!("/api/v1/events", "subscribeToEvents", EventQuery, String);
    read!(
        "/api/v1/statistics",
        "getStatistics",
        StatisticsQuery,
        Vec<kanlo_db::recovery::StatisticSample>
    );
    read!(
        "/api/v1/undo/{id}",
        "getUndoHistory",
        Vec<responses::UndoEntry>
    );
    write!(
        "/api/v1/undo/{id}",
        "previewUndo",
        UndoRequest, RecoveryPreview
    );
    read!("/api/v1/exports/{id}", "exportBoard", BoardExport);
    read!("/api/v1/exports/{id}/csv", "exportBoardCsv", String);
    write!(
        "/api/v1/imports/preview",
        "previewImport",
        ImportRequest, RecoveryPreview
    );
    write!(
        "/api/v1/imports/csv/preview",
        "previewCsvImport",
        CsvRequest, RecoveryPreview
    );
    write!(
        "/api/v1/recovery/confirm",
        "confirmRecovery",
        Confirmation, Receipt
    );
    write!("/api/v1/backups", "createBackup", (), Backup);
    write!(
        "/api/v1/backups/restore/preview",
        "previewRestore",
        RestoreRequest, RecoveryPreview
    );
    read!(
        "/api/v1/backups/safety",
        "listSafetyBackups",
        Vec<responses::SafetyBackup>
    );
    read!("/api/v1/backups/safety/{id}", "getSafetyBackup", Backup);
    read!(
        "/api/v1/sync/handshake",
        "syncHandshake",
        responses::Handshake
    );
    read!(
        "/api/v1/sync/receipts/{id}",
        "getReceipt",
        ReceiptQuery,
        Receipt
    );
    write!(
        "/api/v1/sync/patch",
        "syncPatch",
        kanlo_core::sync::Operation,
        SyncResponse
    );
    write!("/api/v1/sync/create", "syncCreate", OfflineCreate, Receipt);
    write!(
        "/api/v1/sync/comment",
        "syncComment",
        OfflineComment, Receipt
    );
    read!(
        "/api/v1/settings/categories",
        "readSettings",
        SettingsQuery,
        responses::SettingsPage
    );
    write!("/api/v1/login", "login", Passcode, responses::Login);
    write!("/api/v1/logout", "logout", (), responses::Logout);
    read!("/api/v1/session", "getSession", responses::SessionStatus);
    write!(
        "/api/v1/security/passcode",
        "setPasscode",
        Passcode,
        responses::Configured
    );
    read!("/api/v1/security/keys", "listApiKeys", Vec<responses::Key>);
    write!(
        "/api/v1/security/keys",
        "mintApiKey",
        KeyName,
        responses::NewKey
    );
    write!(
        "/api/v1/security/keys/{id}/revoke",
        "revokeApiKey",
        (),
        responses::Revoked
    );
    write!(
        "/api/v1/security/sessions/revoke",
        "revokeSessions",
        (),
        responses::Revoked
    );
    contract.schemas.insert(
        "ApiError".into(),
        json!({
            "type":"object","required":["error"],"properties":{"error":{
                "type":"object","required":["code","message"],
                "properties":{"code":{"type":"string"},"message":{"type":"string"}}
            }}
        }),
    );
    json!({
        "openapi":"3.1.0",
        "info":{
            "title":"Kanlo API",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"Use the local same-origin listener or a protected HTTPS listener. Protected browser writes use a session cookie and X-Kanlo-CSRF. Programmatic callers use separate revocable Bearer keys. Destructive commands require preview and confirmation."
        },
        "servers":[{"url":"/"}],
        "security":[{"bearerKey":[]},{"sessionCookie":[]},{}],
        "paths":contract.paths,
        "components":{
            "securitySchemes":{
                "bearerKey":{"type":"http","scheme":"bearer"},
                "sessionCookie":{"type":"apiKey","in":"cookie","name":"__Host-kanlo"}
            },
            "schemas":contract.schemas
        }
    })
}
