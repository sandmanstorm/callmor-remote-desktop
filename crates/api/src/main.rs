mod audit;
mod auth_extractor;
mod email;
mod jwt;
mod routes;
mod state;

use anyhow::Result;
use axum::{
    routing::{delete, get, patch, post},
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

    let public_url = std::env::var("PUBLIC_WEB_URL").unwrap_or_else(|_| "https://remote.callmor.ai".into());

    // Check SMTP config once at startup (for logging only; we load fresh from DB on each send)
    if email::EmailConfig::load(&pool).await.is_some() {
        info!("SMTP configured (email enabled)");
    } else {
        info!("SMTP not configured — emails disabled until you set it in the admin panel");
    }

    let state = AppState {
        db: pool.clone(),
        jwt: jwt::JwtKeys::from_secret(jwt_secret.as_bytes()),
        public_url,
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
        .route("/machines/{id}", patch(routes::machines::update_machine))
        .route("/machines/{id}", delete(routes::machines::delete_machine))
        .route("/machines/{id}/access", get(routes::machines::list_machine_access))
        .route("/machines/{id}/access", post(routes::machines::grant_machine_access))
        .route("/machines/{id}/access/{user_id}", delete(routes::machines::revoke_machine_access))
        // Users
        .route("/users", get(routes::users::list_users))
        .route("/users/{id}", patch(routes::users::update_user))
        .route("/users/{id}", delete(routes::users::delete_user))
        // Invitations (authenticated)
        .route("/invitations", get(routes::invitations::list_invitations))
        .route("/invitations", post(routes::invitations::create_invitation))
        .route("/invitations/{id}", delete(routes::invitations::delete_invitation))
        // Invitation accept (public)
        .route("/invitations/token/{token}", get(routes::invitations::get_invitation))
        .route("/invitations/token/{token}/accept", post(routes::invitations::accept_invitation))
        // Sessions
        .route("/sessions", post(routes::sessions::create_session))
        .route("/sessions", get(routes::sessions::list_sessions))
        .route("/sessions/active", get(routes::sessions::list_active_sessions))
        // Downloads
        .route("/downloads/agent/linux/deb", get(routes::downloads::download_agent_deb))
        .route("/downloads/agent/windows/zip", get(routes::downloads::download_agent_windows))
        // Audit log
        .route("/audit", get(routes::audit_log::list_tenant_audit))
        .route("/admin/audit", get(routes::audit_log::list_platform_audit))
        // Agent (agent-token auth, not user JWT)
        .route("/agent/heartbeat", post(routes::agent::heartbeat))
        // Admin (superadmin only)
        .route("/admin/test-email", post(routes::email_test::send_test_email))
        .route("/admin/stats", get(routes::admin::get_stats))
        .route("/admin/tenants", get(routes::admin::list_tenants))
        .route("/admin/tenants/{id}", delete(routes::admin::delete_tenant))
        .route("/admin/users", get(routes::admin::list_users))
        .route("/admin/users/{id}/superadmin", patch(routes::admin::set_superadmin))
        .route("/admin/machines", get(routes::admin::list_machines))
        .route("/admin/settings/smtp", get(routes::settings::get_smtp))
        .route("/admin/settings/smtp", axum::routing::put(routes::settings::update_smtp))
        .route("/admin/settings/smtp", delete(routes::settings::clear_smtp))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Callmor API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}
