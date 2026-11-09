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

type Database = Arc<Mutex<Store>>;
pub struct ApiError(Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            Error::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION"),
            Error::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Error::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Error::WipLimit => (StatusCode::CONFLICT, "WIP_LIMIT"),
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
            get(tickets).post(create_ticket),
        )
        .route("/api/v1/tickets/{id}", get(ticket))
        .route("/api/v1/tickets/{id}/move", post(move_ticket))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(middleware::from_fn(guard))
        .with_state(Arc::new(Mutex::new(store)))
}

// The development listener is loopback-only. Browser writes also require a
// same-origin Origin header. Forwarded headers never change authorization.
async fn guard(request: axum::extract::Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok());
    let valid_host = host.is_some_and(|host| {
        host == "127.0.0.1"
            || host == "localhost"
            || host == "[::1]"
            || host.strip_prefix("127.0.0.1:").is_some_and(valid_port)
            || host.strip_prefix("localhost:").is_some_and(valid_port)
            || host.strip_prefix("[::1]:").is_some_and(valid_port)
    });
    if !valid_host {
        return (StatusCode::FORBIDDEN, "Invalid host").into_response();
    }
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) {
        let expected = format!("http://{}", host.unwrap_or_default());
        if request
            .headers()
            .get(header::ORIGIN)
            .and_then(|h| h.to_str().ok())
            != Some(expected.as_str())
        {
            return (StatusCode::FORBIDDEN, "Same-origin Origin header required").into_response();
        }
    }
    let mut response = next.run(request).await;
    for (name, value) in [
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
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
    Json(input): Json<NewBoard>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(database(db, |db| db.create_board(input)).await?),
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
    Json(input): Json<NewColumn>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(database(db, move |db| db.create_column(&id, input)).await?),
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
    Json(input): Json<NewTicket>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(database(db, move |db| db.create_ticket(&id, input)).await?),
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
    Json(input): Json<MoveTicket>,
) -> Result<Json<kanlo_core::Ticket>, ApiError> {
    Ok(Json(
        database(db, move |db| db.move_ticket(&id, input)).await?,
    ))
}
