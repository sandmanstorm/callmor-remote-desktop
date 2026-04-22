mod auth_extractor;
mod jwt;
mod routes;
mod state;

use anyhow::Result;
use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

use state::AppState;

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "callmor-api" }))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        // Fall back to a derived secret from the database password for dev
        let db_pass = std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "dev-secret".into());
        format!("callmor-jwt-{db_pass}")
    });

    let pool = callmor_shared::db::create_pool(&database_url).await?;
    info!("Connected to PostgreSQL");

    let state = AppState {
        db: pool.clone(),
        jwt: jwt::JwtKeys::from_secret(jwt_secret.as_bytes()),
    };

    // Background sweep: mark stale machines offline every 30s
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                routes::agent::sweep_stale(&pool).await;
            }
        });
    }

    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()?;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health
        .route("/health", get(health))
        // Auth
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/refresh", post(routes::auth::refresh))
        .route("/auth/logout", post(routes::auth::logout))
        // Machines
        .route("/machines", get(routes::machines::list_machines))
        .route("/machines", post(routes::machines::create_machine))
        .route("/machines/{id}", get(routes::machines::get_machine))
        .route("/machines/{id}", delete(routes::machines::delete_machine))
        // Sessions
        .route("/sessions", post(routes::sessions::create_session))
        .route("/sessions", get(routes::sessions::list_sessions))
        .route("/sessions/active", get(routes::sessions::list_active_sessions))
        // Downloads
        .route("/downloads/agent/linux/deb", get(routes::downloads::download_agent_deb))
        // Agent (agent-token auth, not user JWT)
        .route("/agent/heartbeat", post(routes::agent::heartbeat))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Callmor API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}
