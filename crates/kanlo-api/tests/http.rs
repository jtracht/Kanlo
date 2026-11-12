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
    let metadata = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header("Host", "localhost:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let metadata: Value =
        serde_json::from_slice(&metadata.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Host", "localhost:8080")
                .header("Origin", "http://localhost:8080")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
                .header("X-Kanlo-Client", "http-test")
                .header(
                    "X-Kanlo-Instance",
                    metadata["instance_id"].as_str().unwrap(),
                )
                .header("X-Kanlo-Epoch", metadata["epoch"].as_str().unwrap())
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
async fn settings_pages_expose_only_requested_values_and_public_security_totals() {
    let mut db = Store::open(":memory:").unwrap();
    let key = db.mint_api_key("Settings test").unwrap();
    let board = db.boards().unwrap().remove(0).id;
    let app = router(db);
    let (status, names) = call(
        app.clone(),
        "GET",
        "/api/v1/settings/categories?category=boards",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names["names"].as_array().unwrap().len(), 5);
    assert!(names.get("values").is_none());
    let (status, values) = call(
        app.clone(),
        "GET",
        &format!(
            "/api/v1/settings/categories?category=boards&board_id={board}&values=true&offset=5"
        ),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(values["values"].as_object().unwrap().len(), 2);
    assert!(values.get("board_options").is_none());
    let (status, security) = call(
        app,
        "GET",
        "/api/v1/settings/categories?category=security&values=true",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        security["values"],
        json!({"api_keys": 1, "passcode_configured": false, "sessions": 0})
    );
    assert!(
        !security
            .to_string()
            .contains(key["token"].as_str().unwrap())
    );
}

#[tokio::test]
async fn display_actions_restore_context_and_leave_board_content_unchanged() {
    let app = router(Store::open(":memory:").unwrap());
    let (_, boards) = call(app.clone(), "GET", "/api/v1/boards", Value::Null).await;
    let id = boards[0]["id"].as_str().unwrap();
    let (_, ticket) = call(
        app.clone(),
        "POST",
        &format!("/api/v1/boards/{id}/tickets"),
        json!({"title":"Backlog ticket"}),
    )
    .await;
    let ticket_id = ticket["id"].as_str().unwrap();
    let (_, before) = call(app.clone(), "GET", "/api/v1/bootstrap", Value::Null).await;
    let (_, display) = call(app.clone(), "GET", "/api/v1/display", Value::Null).await;
    assert_eq!(display["preferences"]["rotation"], json!([]));
    let (status, _) = call(app.clone(), "POST", "/api/v1/display", json!({"kind":"configure","revision":0,"preferences":{"view":"overview","rotation":[{"view":"overview","board_id":null},{"view":"board","board_id":id}],"overlay_minutes":0}})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"configure","revision":0,"preferences":{}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"show_backlog","board_id":id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, opened) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"show_ticket","ticket_id":ticket["public_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(opened["overlays"][0]["kind"], "backlog");
    assert_eq!(opened["overlays"][1]["ticket_id"], ticket_id);
    assert_eq!(opened["expires_at"], Value::Null);
    let (_, returned) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"back"}),
    )
    .await;
    assert_eq!(
        returned["overlays"],
        json!([{"kind":"backlog","board_id":id}])
    );
    let (_, highlighted) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"highlight","ticket_ids":[ticket_id],"seconds":0}),
    )
    .await;
    assert_eq!(highlighted["highlights"], json!([ticket_id]));
    assert_eq!(highlighted["overlays"], json!([]));
    let (_, scrolled) = call(
        app.clone(),
        "POST",
        "/api/v1/display",
        json!({"kind":"scroll","delta":2}),
    )
    .await;
    assert_eq!(scrolled["scroll_offset"], 2);
    let (_, overview) = call(app.clone(), "GET", "/api/v1/overview", Value::Null).await;
    assert_eq!(overview[0]["open"], 1);
    assert_eq!(overview[0]["backlog"], 1);
    let (_, after) = call(app, "GET", "/api/v1/bootstrap", Value::Null).await;
    assert_eq!(before["revision"], after["revision"]);
    assert!(after["event_cursor"].as_i64().unwrap() > before["event_cursor"].as_i64().unwrap());
}

#[tokio::test]
async fn ticket_search_pages_across_boards_and_validates_filters() {
    let app = router(Store::open(":memory:").unwrap());
    let (_, boards) = call(app.clone(), "GET", "/api/v1/boards", Value::Null).await;
    let (status, second) = call(
        app.clone(),
        "POST",
        "/api/v1/boards",
        json!({"name":"Second", "prefix":"SEC"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    for board in [&boards[0], &second] {
        let path = format!("/api/v1/boards/{}/tickets", board["id"].as_str().unwrap());
        let (status, _) = call(
            app.clone(),
            "POST",
            &path,
            json!({"title":"Find this", "description":"shared search term"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let path = "/api/v1/ticket-search?query=shared+search+term&status=open&limit=1";
    let (status, first) = call(app.clone(), "GET", path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["next_offset"], 1);
    let (status, last) = call(app.clone(), "GET", &format!("{path}&offset=1"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(last["next_offset"], Value::Null);
    assert_ne!(first["items"][0]["board_id"], last["items"][0]["board_id"]);
    assert!(first["items"][0].get("description").is_none());
    for query in ["limit=26", "status=unknown"] {
        let (status, _) = call(
            app.clone(),
            "GET",
            &format!("/api/v1/ticket-search?{query}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
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
