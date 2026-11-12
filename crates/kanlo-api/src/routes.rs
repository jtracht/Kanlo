use super::*;
use axum::{
    Extension,
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
};
use serde::Deserialize;
use std::{collections::BTreeMap, convert::Infallible};

mod contract;

pub fn routes() -> Router<Database> {
    Router::new()
        .route("/api/v1/openapi.json", get(contract::openapi))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route(
            "/api/v1/display",
            get(display_state)
                .post(display_action)
                .layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/system/update", get(update_status))
        .route("/api/v1/integrations/verify", get(verify_integration))
        .route("/api/v1/boards/{id}/snapshot", get(snapshot))
        .route("/api/v1/commands", post(command))
        .route("/api/v1/commands/preview", post(preview))
        .route("/api/v1/commands/confirm", post(confirm))
        .route("/api/v1/search", post(search))
        .route("/api/v1/ticket-search", get(ticket_search))
        .route("/api/v1/tickets/{id}/comments", get(ticket_comments))
        .route("/api/v1/events", get(events))
        .route("/api/v1/statistics", get(statistics))
        .route("/api/v1/undo/{id}", get(history).post(undo))
        .route("/api/v1/exports/{id}", get(export_json))
        .route("/api/v1/exports/{id}/csv", get(export_csv))
        .route("/api/v1/imports/preview", post(import_preview))
        .route("/api/v1/imports/csv/preview", post(csv_preview))
        .route("/api/v1/recovery/confirm", post(recovery_confirm))
        .route("/api/v1/backups", post(backup))
        .route("/api/v1/backups/restore/preview", post(restore_preview))
        .route("/api/v1/backups/safety", get(safety))
        .route("/api/v1/backups/safety/{id}", get(safety_backup))
        .route("/api/v1/sync/handshake", get(handshake))
        .route("/api/v1/sync/receipts/{id}", get(receipt))
        .route("/api/v1/sync/patch", post(sync_patch))
        .route("/api/v1/sync/create", post(sync_create))
        .route("/api/v1/sync/comment", post(sync_comment))
        .route("/api/v1/settings/categories", get(setting_categories))
        .route(
            "/api/v1/login",
            post(login).layer(DefaultBodyLimit::max(1024)),
        )
        .route("/api/v1/logout", post(logout))
        .route("/api/v1/session", get(session))
        .route("/api/v1/security/passcode", post(set_passcode))
        .route("/api/v1/security/keys", get(keys).post(mint_key))
        .route("/api/v1/security/keys/{id}/revoke", post(revoke_key))
        .route("/api/v1/security/sessions/revoke", post(revoke_sessions))
}
#[derive(serde::Serialize, schemars::JsonSchema)]
struct UpdateStatus {
    version: &'static str,
    manual_updater_installed: bool,
}

async fn update_status() -> Json<UpdateStatus> {
    Json(UpdateStatus {
        version: env!("CARGO_PKG_VERSION"),
        manual_updater_installed: cfg!(target_os = "linux")
            && tokio::fs::try_exists("/usr/lib/kanlo/update.py")
                .await
                .unwrap_or(false),
    })
}

async fn bootstrap(State(db): State<Database>) -> Result<Json<Value>, ApiError> {
    Ok(Json(database(db,|db|{let state=db.workspace()?;let (instance,epoch)=db.identity()?;Ok(json!({"boards":state.boards,"types":state.types,"labels":state.labels,"settings":state.settings,"board_options":state.board_options,"views":state.views,"revision":db.revision()?,"event_cursor":db.event_cursor()?,"instance_id":instance,"epoch":epoch,"passcode_configured":db.has_passcode()?}))}).await?))
}
async fn display_state(
    State(db): State<Database>,
) -> Result<Json<kanlo_core::tv::DisplayState>, ApiError> {
    Ok(Json(database(db, |db| db.display()).await?))
}

async fn display_action(
    State(db): State<Database>,
    Json(action): Json<kanlo_core::tv::Action>,
) -> Result<Json<kanlo_core::tv::DisplayState>, ApiError> {
    Ok(Json(
        database(db, move |db| db.control_display(action)).await?,
    ))
}

async fn verify_integration(headers: axum::http::HeaderMap) -> Result<Json<Value>, ApiError> {
    // Ingress already authenticates any Bearer token. LAN requests without a
    // token must not be mistaken for a successful key check.
    if !headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("Bearer "))
    {
        return Err(ApiError(Error::Unauthorized));
    }
    Ok(Json(
        json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}),
    ))
}

async fn overview(State(db): State<Database>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(database(db, |db| {
        let workspace = db.workspace()?;
        let mut counts = BTreeMap::<&str, [usize; 4]>::new();
        for ticket in workspace.tickets.iter().filter(|ticket| !ticket.details.archived && !ticket.details.trashed) {
            let count = counts.entry(&ticket.board_id).or_default();
            count[usize::from(ticket.done)] += 1;
            count[2] += usize::from(ticket.column_id.is_none());
            count[3] += usize::from(workspace.blocked(&ticket.id));
        }
        Ok(workspace.boards.iter().map(|board| {
            let [open, done, backlog, blocked] = counts.get(board.id.as_str()).copied().unwrap_or_default();
            json!({"board_id":board.id,"name":board.name,"open":open,"done":done,"backlog":backlog,"blocked":blocked})
        }).collect())
    }).await?))
}
async fn snapshot(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(database(db,move|db|{let mut export=db.export_board(&id)?;let state=db.workspace()?;export.workspace.settings=state.settings.clone();export.workspace.views=state.views.clone();export.workspace.types=state.types.clone();export.workspace.labels=state.labels.clone();let external:Vec<_>=export.external_references.iter().map(|reference|{let ticket=state.find_ticket(&reference.id)?;Ok(json!({"id":reference.id,"public_id":reference.public_id,"done":ticket.done,"trashed":ticket.details.trashed}))}).collect::<kanlo_core::Result<_>>()?;Ok(json!({"workspace":export.workspace,"external_references":external,"revision":db.revision()?,"view_states":state.views,"settings":state.settings,"epoch":db.identity()?.1}))}).await?))
}
async fn command(
    State(db): State<Database>,
    Json(body): Json<kanlo_db::Mutation>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(database(db, move |db| db.mutate(body)).await?))
}
async fn preview(
    State(db): State<Database>,
    Json(body): Json<PreviewRequest>,
) -> Result<Json<kanlo_db::Preview>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.preview_checked(body.command, body.base_context)
        })
        .await?,
    ))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreviewRequest {
    command: kanlo_core::Command,
    #[serde(default)]
    base_context: Value,
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Confirmation {
    token: String,
    op_id: String,
    client_id: String,
}
async fn confirm(
    State(db): State<Database>,
    Json(body): Json<Confirmation>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.confirm(&body.token, &body.op_id, &body.client_id)
        })
        .await?,
    ))
}
async fn search(
    State(db): State<Database>,
    Json(view): Json<kanlo_core::ViewState>,
) -> Result<Json<Vec<kanlo_core::Ticket>>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.workspace()?.search(&view, chrono::Utc::now())
        })
        .await?,
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TicketSearch {
    #[serde(default)]
    board_id: String,
    #[serde(default)]
    query: String,
    column_id: Option<String>,
    status: Option<String>,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

fn page_limit(limit: Option<usize>) -> Result<usize, ApiError> {
    let limit = limit.unwrap_or(10);
    if !(1..=25).contains(&limit) {
        return Err(ApiError(Error::Validation("limit must be 1–25".into())));
    }
    Ok(limit)
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommentPage {
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

async fn ticket_comments(
    State(db): State<Database>,
    Path(id): Path<String>,
    Query(input): Query<CommentPage>,
) -> Result<Json<Value>, ApiError> {
    let limit = page_limit(input.limit)?;
    Ok(Json(
        database(db, move |db| {
            let workspace = db.workspace()?;
            let ticket = workspace.find_ticket(&id)?;
            let mut comments: Vec<_> = workspace
                .comments
                .iter()
                .filter(|comment| comment.ticket_id == ticket.id)
                .collect();
            comments.sort_unstable_by(|a, b| (&a.created_at, &a.id).cmp(&(&b.created_at, &b.id)));
            let mut comments = comments.into_iter().skip(input.offset);
            let items: Vec<_> = comments.by_ref().take(limit).collect();
            let next_offset = comments.next().map(|_| input.offset.saturating_add(limit));
            Ok(json!({ "items": items, "next_offset": next_offset }))
        })
        .await?,
    ))
}

async fn ticket_search(
    State(db): State<Database>,
    Query(input): Query<TicketSearch>,
) -> Result<Json<Value>, ApiError> {
    let limit = page_limit(input.limit)?;
    let status = input.status.unwrap_or_else(|| "active".into());
    if !matches!(
        status.as_str(),
        "active"
            | "open"
            | "done"
            | "blocked"
            | "backlog"
            | "overdue"
            | "archive"
            | "trash"
            | "all"
    ) {
        return Err(ApiError(Error::Validation(
            "Invalid ticket search status".into(),
        )));
    }
    let view = kanlo_core::ViewState {
        board_id: input.board_id,
        query: input.query,
        status,
        ..Default::default()
    };
    Ok(Json(
        database(db, move |db| {
            let tickets = db.workspace()?.search(&view, chrono::Utc::now())?;
            let mut matching = tickets
                .iter()
                .filter(|ticket| {
                    input
                        .column_id
                        .as_ref()
                        .is_none_or(|column| ticket.column_id.as_ref() == Some(column))
                })
                .skip(input.offset);
            let items: Vec<_> = matching.by_ref().take(limit).map(|ticket| json!({
            "id": ticket.id, "public_id": ticket.public_id, "board_id": ticket.board_id,
            "column_id": ticket.column_id, "title": ticket.title, "importance": ticket.importance,
            "done": ticket.done, "revision": ticket.revision
        })).collect();
            let next_offset = matching.next().map(|_| input.offset.saturating_add(limit));
            Ok(json!({ "items": items, "next_offset": next_offset }))
        })
        .await?,
    ))
}
#[derive(Deserialize, schemars::JsonSchema)]
struct EventQuery {
    #[serde(default)]
    after: i64,
}
async fn events(
    State(db): State<Database>,
    Query(query): Query<EventQuery>,
    stop: Option<axum::Extension<tokio::sync::watch::Receiver<bool>>>,
    headers: axum::http::HeaderMap,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let key = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from);
    let token = ingress::session_token(&headers);
    let mut cursor = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(query.after);
    let mut stop = stop.map(|extension| extension.0);
    let stream = async_stream::stream! {loop{if stop.as_ref().is_some_and(|receiver|*receiver.borrow()){break;}let current=cursor;let key=key.clone();let token=token.clone();match database(db.clone(),move|db|{if let Some(key)=key{db.authorize_key(&key)?;}else if let Some(token)=token{db.authorize_session(&token,None,false)?;}db.events(current,1000)}).await{Ok(events)=>{for event in events{cursor=event["id"].as_i64().unwrap_or(cursor);yield Ok(Event::default().id(cursor.to_string()).event("change").data(event["data"].to_string()));}},Err(_)=>{yield Ok(Event::default().event("auth_expired").data("Authentication required"));break}}if let Some(receiver)=stop.as_mut(){tokio::select!{_=receiver.changed()=>break,_=tokio::time::sleep(std::time::Duration::from_secs(2))=>{}}}else{tokio::time::sleep(std::time::Duration::from_secs(2)).await;}}};
    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
}
#[derive(Deserialize, schemars::JsonSchema)]
struct StatisticsQuery {
    board_id: Option<String>,
    from: String,
    to: String,
}
async fn statistics(
    State(db): State<Database>,
    Query(query): Query<StatisticsQuery>,
) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.statistics(query.board_id.as_deref(), &query.from, &query.to)
        })
        .await?,
    ))
}
async fn history(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(database(db, move |db| db.undo_history(&id)).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UndoRequest {
    revision: i64,
    through_id: Option<i64>,
    #[serde(default = "one")]
    count: usize,
}
fn one() -> usize {
    1
}
async fn undo(
    State(db): State<Database>,
    Path(id): Path<String>,
    Json(body): Json<UndoRequest>,
) -> Result<Json<kanlo_db::recovery::RecoveryPreview>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            if let Some(through) = body.through_id {
                db.undo_through_preview(&id, through, body.revision)
            } else {
                db.undo_preview(&id, body.count, body.revision)
            }
        })
        .await?,
    ))
}
async fn export_json(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<kanlo_db::recovery::BoardExport>, ApiError> {
    Ok(Json(database(db, move |db| db.export_board(&id)).await?))
}
async fn export_csv(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=kanlo-board.csv",
            ),
        ],
        database(db, move |db| db.export_csv(&id)).await?,
    ))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportRequest {
    package: kanlo_db::recovery::BoardExport,
    #[serde(default)]
    mode: kanlo_db::recovery::ImportMode,
    #[serde(default)]
    prefix_overrides: BTreeMap<String, String>,
}
async fn import_preview(
    State(db): State<Database>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<kanlo_db::recovery::RecoveryPreview>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.import_preview(body.package, body.mode, body.prefix_overrides)
        })
        .await?,
    ))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CsvRequest {
    board_id: String,
    csv: String,
    #[serde(default)]
    mode: kanlo_db::recovery::ImportMode,
}
async fn csv_preview(
    State(db): State<Database>,
    Json(body): Json<CsvRequest>,
) -> Result<Json<kanlo_db::recovery::RecoveryPreview>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.csv_preview(&body.board_id, &body.csv, body.mode)
        })
        .await?,
    ))
}
async fn recovery_confirm(
    State(db): State<Database>,
    Json(body): Json<Confirmation>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.recovery_confirm(&body.token, &body.op_id, &body.client_id)
        })
        .await?,
    ))
}
async fn backup(State(db): State<Database>) -> Result<Json<kanlo_db::recovery::Backup>, ApiError> {
    Ok(Json(database(db, |db| db.backup()).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RestoreRequest {
    backup: kanlo_db::recovery::Backup,
    boards: Option<Vec<String>>,
}
async fn restore_preview(
    State(db): State<Database>,
    Json(body): Json<RestoreRequest>,
) -> Result<Json<kanlo_db::recovery::RecoveryPreview>, ApiError> {
    Ok(Json(
        database(db, move |db| db.restore_preview(body.backup, body.boards)).await?,
    ))
}
async fn safety(State(db): State<Database>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(database(db, |db| db.safety_snapshots()).await?))
}
async fn safety_backup(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<kanlo_db::recovery::Backup>, ApiError> {
    Ok(Json(database(db, move |db| db.safety_snapshot(&id)).await?))
}
async fn handshake(State(db): State<Database>) -> Result<Json<Value>, ApiError> {
    Ok(Json(database(db,|db|{let(instance,epoch)=db.identity()?;Ok(json!({"instance_id":instance,"epoch":epoch,"revision":db.revision()?,"authenticated":true}))}).await?))
}
async fn sync_patch(
    State(db): State<Database>,
    Json(body): Json<kanlo_core::sync::Operation>,
) -> Result<Json<kanlo_db::synchronization::SyncResponse>, ApiError> {
    Ok(Json(database(db, move |db| db.sync_patch(body)).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
struct ReceiptQuery {
    client_id: String,
}
async fn receipt(
    State(db): State<Database>,
    Path(id): Path<String>,
    Query(query): Query<ReceiptQuery>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(
        database(db, move |db| {
            db.lookup_receipt(&id, &query.client_id)?
                .ok_or(Error::NotFound)
        })
        .await?,
    ))
}
async fn sync_create(
    State(db): State<Database>,
    Json(body): Json<kanlo_db::synchronization::OfflineCreate>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(database(db, move |db| db.sync_create(body)).await?))
}
async fn sync_comment(
    State(db): State<Database>,
    Json(body): Json<kanlo_db::synchronization::OfflineComment>,
) -> Result<Json<kanlo_db::Receipt>, ApiError> {
    Ok(Json(database(db, move |db| db.sync_comment(body)).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
struct SettingsQuery {
    category: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    values: bool,
}
async fn setting_categories(
    State(db): State<Database>,
    Query(query): Query<SettingsQuery>,
) -> Result<Json<Value>, ApiError> {
    let categories = json!([{"id":"appearance","name":"Appearance"},{"id":"boards","name":"Boards and rules"},{"id":"backup","name":"Backups"},{"id":"security","name":"Security"},{"id":"locale","name":"Language and timezone"}]);
    let Some(category) = query.category else {
        return Ok(Json(json!({"categories":categories})));
    };
    let names: &[&str] = match category.as_str() {
        "appearance" => &["theme", "animation", "accent_color"],
        "backup" => &["backup_every_hours", "backup_keep"],
        "locale" => &["language", "timezone"],
        "boards" => &[
            "undo_depth",
            "importance_weight",
            "type_weight",
            "deadline_weight",
            "deadline_warning_days",
            "auto_archive_days",
            "trash_retention_days",
        ],
        "security" => &["passcode_configured", "api_keys", "sessions"],
        _ => return Err(ApiError(Error::NotFound)),
    };
    let page: Vec<_> = names
        .iter()
        .skip(query.offset)
        .take(5)
        .map(|s| s.to_string())
        .collect();
    let next_offset = query.offset.saturating_add(5);
    let next = if next_offset < names.len() {
        Some(next_offset)
    } else {
        None
    };
    if !query.values {
        return Ok(Json(json!({"names":page,"next_offset":next})));
    }
    Ok(Json(database(db,move|db|{let state=db.workspace()?;let settings=serde_json::to_value(state.settings).map_err(|_|Error::Storage)?;let values:BTreeMap<_,_>=page.iter().map(|name|(name.clone(),settings.get(name).cloned().unwrap_or(Value::Null))).collect();Ok(json!({"values":values,"next_offset":next,"board_options":if category=="boards"{json!(state.board_options)}else{Value::Null}}))}).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Passcode {
    passcode: String,
}
async fn login(
    State(db): State<Database>,
    Json(body): Json<Passcode>,
) -> Result<Response, ApiError> {
    let session = database(db, move |db| db.login(&body.passcode)).await?;
    let cookie = format!(
        "__Host-kanlo={}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=43200",
        session.token
    );
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(json!({"authenticated":true,"csrf":session.csrf,"expires_at":session.expires_at})),
    )
        .into_response())
}
async fn logout(
    State(db): State<Database>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let token = ingress::session_token(&headers).ok_or(ApiError(Error::Unauthorized))?;
    database(db, move |db| db.logout(&token)).await?;
    Ok((
        [(
            header::SET_COOKIE,
            "__Host-kanlo=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"authenticated":false})),
    )
        .into_response())
}
async fn session(
    State(db): State<Database>,
    Extension(context): Extension<ingress::Context>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>, ApiError> {
    if !context.external {
        return Ok(Json(json!({"authenticated":true,"local":true})));
    }
    let token = ingress::session_token(&headers).ok_or(ApiError(Error::Unauthorized))?;
    Ok(Json(
        database(db, move |db| db.session_status(&token)).await?,
    ))
}
async fn set_passcode(
    State(db): State<Database>,
    Json(body): Json<Passcode>,
) -> Result<Json<Value>, ApiError> {
    database(db, move |db| db.set_passcode(&body.passcode)).await?;
    Ok(Json(json!({"configured":true})))
}
async fn keys(State(db): State<Database>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(database(db, |db| db.list_api_keys()).await?))
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct KeyName {
    name: String,
}
async fn mint_key(
    State(db): State<Database>,
    Json(body): Json<KeyName>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        database(db, move |db| db.mint_api_key(&body.name)).await?,
    ))
}
async fn revoke_key(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    database(db, move |db| db.revoke_api_key(&id)).await?;
    Ok(Json(json!({"revoked":true})))
}
async fn revoke_sessions(State(db): State<Database>) -> Result<Json<Value>, ApiError> {
    database(db, |db| db.revoke_sessions()).await?;
    Ok(Json(json!({"revoked":true})))
}
