use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind: SocketAddr = std::env::var("KANLO_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    if !bind.ip().is_loopback() {
        return Err("This development slice only supports loopback; LAN/external ingress requires KAN-053–055".into());
    }
    let directory =
        PathBuf::from(std::env::var_os("KANLO_DATA_DIR").unwrap_or_else(|| "data".into()));
    std::fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let store = kanlo_db::Store::open(directory.join("kanlo.sqlite3"))?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!(
        "Kanlo development server listening on http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, kanlo_api::router(store))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
