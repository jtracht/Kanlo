use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kanlo_api::router;
use kanlo_db::Store;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(app: axum::Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Host", "localhost:8080")
                .header("Origin", "http://localhost:8080")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn http_roundtrip_and_stale_revision() {
    let app = router(Store::open(":memory:").unwrap());
    let (status, boards) = call(app.clone(), "GET", "/api/v1/boards", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let id = boards[0]["id"].as_str().unwrap();
    let path = format!("/api/v1/boards/{id}/tickets");
    let (status, ticket) = call(
        app.clone(),
        "POST",
        &path,
        json!({"title":"API Ticket", "importance":255}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(ticket["public_id"], "KAN-1");
    let (status, _) = call(app.clone(), "GET", "/api/v1/tickets/KAN-1", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let (status, error) = call(
        app,
        "POST",
        "/api/v1/tickets/KAN-1/move",
        json!({"board_id":id,"base_revision":0}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn invalid_input_is_not_persisted() {
    let app = router(Store::open(":memory:").unwrap());
    for input in [
        json!({"name":" ","prefix":"DEV"}),
        json!({"name":"Dev","prefix":"dev"}),
    ] {
        let (status, _) = call(app.clone(), "POST", "/api/v1/boards", input).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (_, boards) = call(app, "GET", "/api/v1/boards", Value::Null).await;
    assert_eq!(boards.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn csrf_and_host_bypass_are_rejected_despite_forwarded_headers() {
    let app = router(Store::open(":memory:").unwrap());
    for (host, origin) in [
        ("evil.example", Some("http://evil.example")),
        ("localhost:8080", Some("https://evil.example")),
        ("localhost:8080", None),
        (
            "localhost.evil.example",
            Some("http://localhost.evil.example"),
        ),
    ] {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/v1/boards")
            .header("Host", host)
            .header("X-Forwarded-For", "127.0.0.1")
            .header("X-Forwarded-Host", "localhost:8080")
            .header("Content-Type", "application/json");
        if let Some(origin) = origin {
            builder = builder.header("Origin", origin);
        }
        let response = app
            .clone()
            .oneshot(
                builder
                    .body(Body::from(r#"{"name":"Injected","prefix":"BAD"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let (_, boards) = call(app, "GET", "/api/v1/boards", Value::Null).await;
    assert_eq!(boards.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn body_limit_unknown_fields_and_importance_range() {
    let app = router(Store::open(":memory:").unwrap());
    let (_, boards) = call(app.clone(), "GET", "/api/v1/boards", Value::Null).await;
    let path = format!(
        "/api/v1/boards/{}/tickets",
        boards[0]["id"].as_str().unwrap()
    );
    for body in [
        json!({"title":"Invalid", "importance":256}),
        json!({"title":"Invalid", "done":true}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header("Host", "localhost")
                    .header("Origin", "http://localhost")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("Host", "localhost")
                .header("Origin", "http://localhost")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"title":"Large", "description":"x".repeat(140000)}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn static_assets_have_csp_and_no_external_runtime() {
    let app = router(Store::open(":memory:").unwrap());
    for path in ["/", "/app.js", "/style.css"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("Host", "localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}
