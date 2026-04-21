use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    info!("Callmor Agent started");
    info!("Agent is a placeholder — screen capture and WebRTC will be added in Milestone 4");

    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;
    info!("Agent shutting down");

    Ok(())
}
