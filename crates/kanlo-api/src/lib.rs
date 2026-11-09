use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use kanlo_core::{Error, MoveTicket, NewBoard, NewColumn, NewTicket};
use kanlo_db::Store;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
pub mod ingress;
mod routes;

pub type Database = Arc<Mutex<Store>>;
pub struct ApiError(Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            Error::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION"),
            Error::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Error::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Error::WipLimit => (StatusCode::CONFLICT, "WIP_LIMIT"),
            Error::Blocked => (StatusCode::CONFLICT, "BLOCKED_BY_DEPENDENCY"),
            Error::Unauthorized => (StatusCode::UNAUTHORIZED, "AUTH_EXPIRED"),
            Error::Storage => (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE"),
        };
        (
            status,
            Json(json!({"error": {"code": code, "message": self.0.to_string()}})),
        )
            .into_response()
    }
}

async fn database<T: Send + 'static>(
    db: Database,
    action: impl FnOnce(&mut Store) -> kanlo_core::Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(move || {
        let mut db = db.lock().map_err(|_| Error::Storage)?;
        action(&mut db)
    })
    .await
    .map_err(|_| ApiError(Error::Storage))?
    .map_err(ApiError)
}

pub fn router(store: Store) -> Router {
    router_shared(Arc::new(Mutex::new(store)), ingress::Policy::local())
}

pub fn router_shared(db: Database, policy: ingress::Policy) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../../../web/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../web/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../../../web/style.css"),
                )
            }),
        )
        .route("/api/v1/health", get(health))
        .route("/api/v1/boards", get(boards).post(create_board))
        .route(
            "/api/v1/boards/{id}/columns",
            get(columns).post(create_column),
        )
        .route(
            "/api/v1/boards/{id}/tickets",
            get(tickets)
                .post(create_ticket)
                .layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route("/api/v1/tickets/{id}", get(ticket))
        .route("/api/v1/tickets/{id}/move", post(move_ticket))
        .merge(routes::routes())
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            ingress::GuardState::new(db.clone(), policy),
            ingress::guard,
        ))
        .with_state(db)
}

fn valid_port(value: &str) -> bool {
    value.parse::<u16>().is_ok_and(|port| port > 0)
}

async fn health(State(db): State<Database>) -> Result<Json<Value>, ApiError> {
    let (instance, epoch) = database(db, |db| db.identity()).await?;
    Ok(Json(
        json!({"status":"ok", "version":env!("CARGO_PKG_VERSION"), "instance_id":instance, "epoch":epoch}),
    ))
}
async fn boards(State(db): State<Database>) -> Result<Json<Vec<kanlo_core::Board>>, ApiError> {
    Ok(Json(database(db, |db| db.boards()).await?))
}
async fn create_board(
    State(db): State<Database>,
    headers: axum::http::HeaderMap,
    Json(input): Json<NewBoard>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            legacy_write(
                db,
                headers,
                kanlo_core::Command::CreateBoard {
                    name: input.name,
                    prefix: input.prefix,
                },
            )
            .await?,
        ),
    ))
}
async fn columns(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<Vec<kanlo_core::Column>>, ApiError> {
    Ok(Json(database(db, move |db| db.columns(&id)).await?))
}
async fn create_column(
    State(db): State<Database>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(input): Json<NewColumn>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            legacy_write(
                db,
                headers,
                kanlo_core::Command::CreateColumn {
                    board_id: id,
                    name: input.name,
                    is_done: input.is_done,
                    wip_limit: input.wip_limit,
                    wip_strict: input.wip_strict,
                },
            )
            .await?,
        ),
    ))
}
async fn tickets(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<Vec<kanlo_core::Ticket>>, ApiError> {
    Ok(Json(database(db, move |db| db.tickets(&id)).await?))
}
async fn create_ticket(
    State(db): State<Database>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(input): Json<NewTicket>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            legacy_write(
                db,
                headers,
                kanlo_core::Command::CreateTicket {
                    board_id: id,
                    title: input.title,
                    description: input.description,
                    importance: input.importance,
                    column_id: input.column_id,
                    parent_id: None,
                    type_id: None,
                },
            )
            .await?,
        ),
    ))
}
async fn ticket(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<kanlo_core::Ticket>, ApiError> {
    Ok(Json(database(db, move |db| db.ticket(&id)).await?))
}
async fn move_ticket(
    State(db): State<Database>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(input): Json<MoveTicket>,
) -> Result<Json<kanlo_core::Ticket>, ApiError> {
    Ok(Json({
        legacy_write(
            db.clone(),
            headers,
            kanlo_core::Command::MoveTicket {
                id: id.clone(),
                base_revision: input.base_revision,
                board_id: input.board_id,
                column_id: input.column_id,
                activate_types: false,
                convert_types: std::collections::BTreeMap::new(),
            },
        )
        .await?;
        database(db, move |db| db.ticket(&id)).await?
    }))
}

async fn legacy_write(
    db: Database,
    headers: axum::http::HeaderMap,
    command: kanlo_core::Command,
) -> Result<Value, ApiError> {
    let get = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| {
                ApiError(Error::Validation(format!(
                    "Missing header {name}; prefer /api/v1/commands"
                )))
            })
    };
    let operation = kanlo_db::Mutation {
        op_id: get("idempotency-key")?,
        client_id: get("x-kanlo-client")?,
        server_instance_id: get("x-kanlo-instance")?,
        base_epoch: get("x-kanlo-epoch")?,
        command,
    };
    database(db, move |db| {
        let receipt = db.mutate(operation)?;
        let state = db.workspace()?;
        let key = receipt.result.entity_id.as_ref().ok_or(Error::Storage)?;
        if let Ok(ticket) = state.find_ticket(key) {
            return serde_json::to_value(ticket).map_err(|_| Error::Storage);
        }
        if let Some(board) = state.boards.iter().find(|b| &b.id == key) {
            return serde_json::to_value(board).map_err(|_| Error::Storage);
        }
        if let Some(column) = state.columns.iter().find(|c| &c.id == key) {
            return serde_json::to_value(column).map_err(|_| Error::Storage);
        }
        Err(Error::Storage)
    })
    .await
}
