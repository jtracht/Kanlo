//! Isolated browser acceptance fixture. Never used by the production binary.
use std::sync::{Arc, Mutex};
#[tokio::main]
async fn main() {
    let mut store = kanlo_db::Store::open(":memory:").unwrap();
    store
        .set_passcode("kanlo browser fixture passcode")
        .unwrap();
    let db = Arc::new(Mutex::new(store));
    let (stop, since) = tokio::sync::watch::channel(false);
    let mut tasks = tokio::task::JoinSet::new();
    for (port, policy) in [
        (
            18081,
            kanlo_api::ingress::Policy::external("https://kanlo.test:18443").unwrap(),
        ),
        (18082, kanlo_api::ingress::Policy::local()),
    ] {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let router =
            kanlo_api::router_shared(db.clone(), policy).layer(axum::Extension(since.clone()));
        let mut since = since.clone();
        tasks.spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = since.changed().await;
                })
                .await
                .unwrap();
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
    let _ = stop.send(true);
    while tasks.join_next().await.is_some() {}
}
