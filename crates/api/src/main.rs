use anyhow::Result;
use axum::{routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "callmor-api" }))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()?;

    let app = Router::new().route("/health", get(health));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Callmor API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
