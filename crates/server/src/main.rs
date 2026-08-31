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
///
/// Enumerates interfaces directly (via `if-addrs`) rather than shelling out:
/// this must work with no default route and no `hostname -I` support, since
/// the restaurant network may have no internet uplink at all.
fn local_addresses() -> Vec<String> {
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter(|iface| !iface.is_loopback())
                .filter_map(|iface| match iface.addr {
                    if_addrs::IfAddr::V4(v4) => Some(v4.ip.to_string()),
                    if_addrs::IfAddr::V6(_) => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
