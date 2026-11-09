use std::{
    collections::BTreeSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

async fn shutdown(mut receiver: watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            break;
        }
    }
}
async fn signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler");
        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory =
        PathBuf::from(std::env::var_os("KANLO_DATA_DIR").unwrap_or_else(|| "data".into()));
    kanlo_db::maintenance::private_directory(&directory)?;
    let mut store = kanlo_db::Store::open(directory.join("kanlo.sqlite3"))?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--check") {
        println!("{}", store.integrity_check()?);
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--migrate-data") {
        let target = args
            .get(1)
            .ok_or("Provide a dedicated empty target directory")?;
        let path = store.relocate(&PathBuf::from(target))?;
        println!(
            "Verified data migration: {}. Set KANLO_DATA_DIR to its parent before restarting.",
            path.display()
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Supported commands: --check, --migrate-data <directory>".into());
    }
    let bind: SocketAddr = std::env::var("KANLO_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let hosts: BTreeSet<String> = std::env::var("KANLO_ALLOWED_HOSTS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if !bind.ip().is_loopback() && hosts.is_empty() {
        return Err(
            "LAN binding requires KANLO_ALLOWED_HOSTS with the exact LAN IP/hostname and port"
                .into(),
        );
    }
    let external = std::env::var("KANLO_EXTERNAL_BIND")
        .ok()
        .map(|s| s.parse::<SocketAddr>())
        .transpose()?;
    let external_policy = if let Some(address) = external {
        if !address.ip().is_loopback() || address == bind {
            return Err("Protected ingress must be a separate loopback socket behind the HTTPS reverse proxy".into());
        }
        if !store.has_passcode()? {
            return Err("Set the shared external passcode through the local settings before enabling protected ingress".into());
        }
        Some(kanlo_api::ingress::Policy::external(
            &std::env::var("KANLO_PUBLIC_ORIGIN")
                .map_err(|_| "Protected ingress requires KANLO_PUBLIC_ORIGIN=https://your-host")?,
        )?)
    } else {
        None
    };
    let backup_directory = PathBuf::from(
        std::env::var_os("KANLO_BACKUP_DIR")
            .unwrap_or_else(|| directory.join("backups").into_os_string()),
    );
    let db = Arc::new(Mutex::new(store));
    let (stop, since) = watch::channel(false);
    let signal_stop = stop.clone();
    tokio::spawn(async move {
        signal().await;
        let _ = signal_stop.send(true);
    });
    let maintenance_db = db.clone();
    let mut maintenance_stop = since.clone();
    let maintenance = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {_ = maintenance_stop.changed()=>break,_ = interval.tick()=>{let db=maintenance_db.clone();let directory=backup_directory.clone();let result=tokio::task::spawn_blocking(move||{let mut db=db.lock().map_err(|_|kanlo_core::Error::Storage)?;db.tick(chrono::Utc::now())?;db.scheduled_backup(&directory,chrono::Utc::now())?;Ok::<_,kanlo_core::Error>(())}).await;if !matches!(result,Ok(Ok(()))){eprintln!("Scheduled maintenance failed; retrying on the next tick");}}}
        }
    });
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!(
        "Kanlo LAN server listening on http://{}",
        listener.local_addr()?
    );
    let mut servers = tokio::task::JoinSet::new();
    let local = kanlo_api::router_shared(db.clone(), kanlo_api::ingress::Policy::lan(hosts))
        .layer(axum::Extension(since.clone()));
    let local_stop = since.clone();
    servers.spawn(async move {
        axum::serve(
            listener,
            local.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown(local_stop))
        .await
    });
    if let (Some(bind), Some(policy)) = (external, external_policy) {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        println!(
            "Kanlo protected proxy ingress listening on {}",
            listener.local_addr()?
        );
        let app = kanlo_api::router_shared(db, policy).layer(axum::Extension(since.clone()));
        let external_stop = since.clone();
        servers.spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown(external_stop))
            .await
        });
    }
    let mut failure = None;
    while let Some(result) = servers.join_next().await {
        if let Err(error) = result? {
            failure = Some(error);
        }
        let _ = stop.send(true);
    }
    let _ = maintenance.await;
    if let Some(error) = failure {
        return Err(error.into());
    }
    Ok(())
}
