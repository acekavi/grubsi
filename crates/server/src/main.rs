use std::net::{Ipv4Addr, SocketAddr};

use grubsi_server::{AppState, build_router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,grubsi_server=debug")),
        )
        .init();

    let port: u16 = std::env::var("GRUBSI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let app = build_router(AppState::new());

    // Bind all interfaces: staff tablets and customer phones reach this
    // over the restaurant LAN, not over loopback.
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "grubsi server started");
    for ip in local_addresses() {
        tracing::info!("reachable at http://{ip}:{port}");
    }

    axum::serve(listener, app).await?;
    Ok(())
}

/// Best-effort list of this machine's LAN addresses, printed at startup so
/// staff know what to type into a tablet. Never fatal.
fn local_addresses() -> Vec<String> {
    use std::process::Command;
    let output = Command::new("hostname").arg("-I").output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter(|s| !s.contains(':'))
            .map(|s| s.to_owned())
            .collect(),
        _ => Vec::new(),
    }
}
