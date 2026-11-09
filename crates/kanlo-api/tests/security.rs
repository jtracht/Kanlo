use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kanlo_api::{ingress::Policy, router_shared};
use kanlo_db::Store;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

async fn request(
    app: axum::Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Value,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    app.oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn protected_listener_requires_auth_and_cannot_be_downgraded_with_forwarded_headers() {
    let db = Arc::new(Mutex::new(Store::open(":memory:").unwrap()));
    let app = router_shared(db, Policy::external("https://kanlo.example").unwrap());
    for headers in [
        vec![
            ("Host", "kanlo.example"),
            ("X-Forwarded-For", "127.0.0.1"),
            ("X-Forwarded-Proto", "http"),
        ],
        vec![("Host", "kanlo.example"), ("Cookie", "__Host-kanlo=fake")],
    ] {
        let response = request(
            app.clone(),
            "GET",
            "/api/v1/bootstrap",
            &headers,
            Value::Null,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = request(
        app,
        "GET",
        "/api/v1/bootstrap",
        &[("Host", "localhost")],
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn external_login_sets_secure_cookie_and_write_requires_csrf() {
    let mut store = Store::open(":memory:").unwrap();
    store.set_passcode("correct long shared passcode").unwrap();
    let db = Arc::new(Mutex::new(store));
    let app = router_shared(db, Policy::external("https://kanlo.example").unwrap());
    let response = request(
        app.clone(),
        "POST",
        "/api/v1/login",
        &[
            ("Host", "kanlo.example"),
            ("Origin", "https://kanlo.example"),
        ],
        json!({"passcode":"correct long shared passcode"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_string();
    for flag in ["Secure", "HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(flag));
    }
    let cookie = cookie.split(';').next().unwrap();
    let login = json_body(response).await;
    let csrf = login["csrf"].as_str().unwrap();
    let response = request(
        app.clone(),
        "GET",
        "/api/v1/bootstrap",
        &[("Host", "kanlo.example"), ("Cookie", cookie)],
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bootstrap = json_body(response).await;
    let body = json!({"op_id":uuid::Uuid::new_v4().to_string(),"client_id":"test","server_instance_id":bootstrap["instance_id"],"base_epoch":bootstrap["epoch"],"command":{"kind":"create_board","name":"External","prefix":"EXT"}});
    let response = request(
        app.clone(),
        "POST",
        "/api/v1/commands",
        &[
            ("Host", "kanlo.example"),
            ("Origin", "https://kanlo.example"),
            ("Cookie", cookie),
        ],
        body.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = request(
        app.clone(),
        "POST",
        "/api/v1/commands",
        &[
            ("Host", "kanlo.example"),
            ("Origin", "https://kanlo.example"),
            ("Cookie", cookie),
            ("X-Kanlo-CSRF", csrf),
        ],
        body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = request(
        app.clone(),
        "POST",
        "/api/v1/logout",
        &[
            ("Host", "kanlo.example"),
            ("Origin", "https://kanlo.example"),
            ("Cookie", cookie),
            ("X-Kanlo-CSRF", csrf),
        ],
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        request(
            app,
            "GET",
            "/api/v1/bootstrap",
            &[("Host", "kanlo.example"), ("Cookie", cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn api_keys_are_independent_and_revocation_is_immediate() {
    let mut store = Store::open(":memory:").unwrap();
    let one = store.mint_api_key("One").unwrap();
    let two = store.mint_api_key("Two").unwrap();
    let db = Arc::new(Mutex::new(store));
    let app = router_shared(
        db.clone(),
        Policy::external("https://kanlo.example").unwrap(),
    );
    let bearer = format!("Bearer {}", one["token"].as_str().unwrap());
    assert_eq!(
        request(
            app.clone(),
            "GET",
            "/api/v1/bootstrap",
            &[("Host", "kanlo.example"), ("Authorization", &bearer)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    db.lock()
        .unwrap()
        .revoke_api_key(one["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        request(
            app.clone(),
            "GET",
            "/api/v1/bootstrap",
            &[("Host", "kanlo.example"), ("Authorization", &bearer)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let bearer = format!("Bearer {}", two["token"].as_str().unwrap());
    assert_eq!(
        request(
            app,
            "GET",
            "/api/v1/bootstrap",
            &[("Host", "kanlo.example"), ("Authorization", &bearer)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn bulk_and_restore_require_authorization_and_restore_revokes_the_calling_key() {
    let mut store = Store::open(":memory:").unwrap();
    let board = store.boards().unwrap()[0].id.clone();
    let ticket = store
        .create_ticket(
            &board,
            kanlo_core::NewTicket {
                title: "Protected".into(),
                description: String::new(),
                importance: 1,
                column_id: None,
            },
        )
        .unwrap();
    let key = store.mint_api_key("Acceptance").unwrap();
    let bearer = format!("Bearer {}", key["token"].as_str().unwrap());
    let db = Arc::new(Mutex::new(store));
    let app = router_shared(
        db.clone(),
        Policy::external("https://kanlo.example").unwrap(),
    );
    let command = json!({"kind":"batch","commands":[{"kind":"patch_ticket","id":ticket.id,"base_revision":ticket.revision,"fields":{"importance":200}}]});
    let body = json!({"command":command,"base_context":[null]});
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/commands/preview",
            &[
                ("Host", "kanlo.example"),
                ("Origin", "https://kanlo.example")
            ],
            body.clone()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let headers = [
        ("Host", "kanlo.example"),
        ("Authorization", bearer.as_str()),
    ];
    let preview = json_body(
        request(
            app.clone(),
            "POST",
            "/api/v1/commands/preview",
            &headers,
            body,
        )
        .await,
    )
    .await;
    let response = request(
        app.clone(),
        "POST",
        "/api/v1/commands/confirm",
        &headers,
        json!({"token":preview["token"],"op_id":uuid::Uuid::new_v4(),"client_id":"security"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        db.lock().unwrap().ticket(&ticket.id).unwrap().importance,
        200
    );
    let backup =
        json_body(request(app.clone(), "POST", "/api/v1/backups", &headers, json!({})).await).await;
    assert!(!backup.to_string().contains(key["token"].as_str().unwrap()));
    let body = json!({"backup":backup,"boards":null});
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/backups/restore/preview",
            &[
                ("Host", "kanlo.example"),
                ("Origin", "https://kanlo.example")
            ],
            body.clone()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let preview = json_body(
        request(
            app.clone(),
            "POST",
            "/api/v1/backups/restore/preview",
            &headers,
            body,
        )
        .await,
    )
    .await;
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/recovery/confirm",
            &headers,
            json!({"token":preview["token"],"op_id":uuid::Uuid::new_v4(),"client_id":"security"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(app, "GET", "/api/v1/bootstrap", &headers, Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn lan_peer_check_uses_socket_peer_and_login_is_rate_limited() {
    let db = Arc::new(Mutex::new(Store::open(":memory:").unwrap()));
    let app = router_shared(
        db.clone(),
        Policy::lan(BTreeSet::from(["192.168.1.5:8080".into()])),
    );
    for (peer, expected) in [
        ("203.0.113.8:12345", StatusCode::FORBIDDEN),
        ("192.168.1.8:12345", StatusCode::OK),
    ] {
        let mut req = Request::builder()
            .uri("/api/v1/boards")
            .header("Host", "192.168.1.5:8080")
            .header("X-Forwarded-For", "127.0.0.1")
            .body(Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), expected);
    }
    let external = router_shared(db, Policy::external("https://kanlo.example").unwrap());
    for index in 0..11 {
        let response = request(
            external.clone(),
            "POST",
            "/api/v1/login",
            &[
                ("Host", "kanlo.example"),
                ("Origin", "https://kanlo.example"),
            ],
            json!({"passcode":"wrong"}),
        )
        .await;
        assert_eq!(
            response.status(),
            if index < 10 {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
}
