use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("RELAY_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("Callmor Relay listening on {addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                info!("New connection from {peer}");
                tokio::spawn(async move {
                    // Milestone 2 will add WebSocket upgrade + message routing here.
                    // For now, just accept and drop the TCP connection.
                    drop(stream);
                });
            }
            Err(e) => warn!("Accept error: {e}"),
        }
    }
}
