use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;
use callmor_shared::Machine;

#[derive(Deserialize)]
pub struct CreateMachineRequest {
    pub name: String,
    /// 9-digit RustDesk ID from the Callmor-RustDesk client. Optional only
    /// for legacy WebRTC agents; new tenant machines set this.
    pub rustdesk_id: Option<String>,
    /// Permanent password the user set in RustDesk. Stored so authorized
    /// tenant users can launch the native client with credentials prefilled.
    pub rustdesk_password: Option<String>,
}

#[derive(Serialize)]
pub struct CreateMachineResponse {
    pub id: Uuid,
    pub name: String,
    pub agent_token: String,
    pub connection_type: String,
}

#[derive(Deserialize)]
pub struct UpdateMachineRequest {
    pub name: Option<String>,
    pub access_mode: Option<String>,
    pub rustdesk_id: Option<String>,
    pub rustdesk_password: Option<String>,
}

#[derive(Serialize)]
pub struct RdConnectResponse {
    pub rustdesk_id: String,
    pub password: String,
    /// Convenience — the exact `rustdesk://` URI the browser should
    /// navigate to in order to launch the native client.
    pub launch_uri: String,
}

fn generate_agent_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    format!("cmt_{}", hex::encode(bytes))
}

fn require_admin_or_owner(role: &str) -> Result<(), (StatusCode, String)> {
    match role {
        "owner" | "admin" => Ok(()),
        _ => Err((StatusCode::FORBIDDEN, "Requires admin or owner".into())),
    }
}

/// Check if a user has access to a machine.
/// Owner/admin always has access. Member must be in machine_access if machine is restricted.
pub async fn user_has_access(
    db: &sqlx::PgPool,
    user_id: Uuid,
    tenant_id: Uuid,
    machine_id: Uuid,
    role: &str,
) -> Result<bool, sqlx::Error> {
    // Owner/admin always has access
    if role == "owner" || role == "admin" {
        return Ok(true);
    }

    // Check machine's access mode
    let access_mode: Option<String> = sqlx::query_scalar(
        "SELECT access_mode FROM machines WHERE id = $1 AND tenant_id = $2",
    )
    .bind(machine_id)
    .bind(tenant_id)
    .fetch_optional(db)
    .await?;

    let Some(mode) = access_mode else { return Ok(false) };

    if mode == "public" {
        return Ok(true);
    }

    // Restricted: check machine_access table
    let has_access: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM machine_access WHERE machine_id = $1 AND user_id = $2)",
    )
    .bind(machine_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;

    Ok(has_access)
}

pub async fn list_machines(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<Machine>>, (StatusCode, String)> {
    // Owner/admin see all, members see only accessible
    let machines: Vec<Machine> = if claims.role == "owner" || claims.role == "admin" {
        sqlx::query_as("SELECT * FROM machines WHERE tenant_id = $1 ORDER BY name")
            .bind(claims.tenant_id)
            .fetch_all(&state.db)
            .await
    } else {
        sqlx::query_as(
            "SELECT m.* FROM machines m
             WHERE m.tenant_id = $1
               AND (m.access_mode = 'public'
                    OR EXISTS (SELECT 1 FROM machine_access ma WHERE ma.machine_id = m.id AND ma.user_id = $2))
             ORDER BY m.name",
        )
        .bind(claims.tenant_id)
        .bind(claims.sub)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(machines))
}

pub async fn create_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<CreateMachineRequest>,
) -> Result<(StatusCode, Json<CreateMachineResponse>), (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    let token = generate_agent_token();

    // Normalize RustDesk ID — accept "123 456 789" / "123-456-789" / "123456789"
    let rd_id = req.rustdesk_id.as_deref().map(normalize_rd_id);
    let rd_password = req.rustdesk_password.as_deref().map(str::trim).map(String::from);

    // If rd_id is provided, validate it's 9 digits (RustDesk's canonical form)
    if let Some(id) = &rd_id {
        if id.len() != 9 || !id.chars().all(|c| c.is_ascii_digit()) {
            return Err((
                StatusCode::BAD_REQUEST,
                "rustdesk_id must be exactly 9 digits".into(),
            ));
        }
    }
    if rd_id.is_some() && rd_password.as_deref().map(str::is_empty).unwrap_or(true) {
        return Err((
            StatusCode::BAD_REQUEST,
            "rustdesk_password is required when rustdesk_id is provided".into(),
        ));
    }

    let connection_type = if rd_id.is_some() { "rustdesk" } else { "webrtc_legacy" };

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO machines (tenant_id, name, agent_token, rustdesk_id, rustdesk_password, connection_type)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(claims.tenant_id)
    .bind(&req.name)
    .bind(&token)
    .bind(rd_id.as_deref())
    .bind(rd_password.as_deref())
    .bind(connection_type)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        // Surface unique-constraint violation on rustdesk_id cleanly
        let msg = e.to_string();
        if msg.contains("idx_machines_rustdesk_id_unique") {
            (
                StatusCode::CONFLICT,
                "This RustDesk ID is already registered to another machine.".into(),
            )
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
        }
    })?;

    crate::audit::log(
        &state.db,
        &crate::audit::ctx_from_claims(&claims),
        "machine.created",
        Some("machine"),
        Some(id),
        serde_json::json!({
            "name": req.name,
            "rustdesk_id": rd_id,
            "connection_type": connection_type,
        }),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateMachineResponse {
            id,
            name: req.name,
            agent_token: token,
            connection_type: connection_type.into(),
        }),
    ))
}

/// Accept "123 456 789" / "123-456-789" / "123456789" — return canonical 9-digit form.
fn normalize_rd_id(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub async fn get_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<Machine>, (StatusCode, String)> {
    let machine: Option<Machine> =
        sqlx::query_as("SELECT * FROM machines WHERE id = $1 AND tenant_id = $2")
            .bind(machine_id)
            .bind(claims.tenant_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let machine = machine.ok_or((StatusCode::NOT_FOUND, "Machine not found".into()))?;

    // Check access
    let access = user_has_access(&state.db, claims.sub, claims.tenant_id, machine.id, &claims.role)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    if !access {
        return Err((StatusCode::FORBIDDEN, "No access to this machine".into()));
    }

    Ok(Json(machine))
}

pub async fn update_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
    Json(req): Json<UpdateMachineRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    if let Some(mode) = &req.access_mode {
        if !["public", "restricted"].contains(&mode.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "access_mode must be public or restricted".into()));
        }
    }

    let rd_id = req.rustdesk_id.as_deref().map(normalize_rd_id);
    if let Some(id) = &rd_id {
        if id.len() != 9 || !id.chars().all(|c| c.is_ascii_digit()) {
            return Err((
                StatusCode::BAD_REQUEST,
                "rustdesk_id must be exactly 9 digits".into(),
            ));
        }
    }

    // If we're setting a rustdesk_id, auto-promote connection_type.
    let promote_to_rustdesk = rd_id.is_some();

    let result = sqlx::query(
        "UPDATE machines SET
           name              = COALESCE($1, name),
           access_mode       = COALESCE($2, access_mode),
           rustdesk_id       = COALESCE($3, rustdesk_id),
           rustdesk_password = COALESCE($4, rustdesk_password),
           connection_type   = CASE WHEN $5::bool THEN 'rustdesk' ELSE connection_type END
         WHERE id = $6 AND tenant_id = $7",
    )
    .bind(req.name.as_deref())
    .bind(req.access_mode.as_deref())
    .bind(rd_id.as_deref())
    .bind(req.rustdesk_password.as_deref())
    .bind(promote_to_rustdesk)
    .bind(machine_id)
    .bind(claims.tenant_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("idx_machines_rustdesk_id_unique") {
            (
                StatusCode::CONFLICT,
                "This RustDesk ID is already registered to another machine.".into(),
            )
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
        }
    })?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "Machine not found".into()))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// Gated credential release for a tenant's RustDesk machine. Caller must
/// have access to the machine. Every call writes an audit event so the
/// tenant admin can see who launched what, when, from which IP. Returns
/// the password + a ready-to-use `rustdesk://` launch URI.
pub async fn rd_connect(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<RdConnectResponse>, (StatusCode, String)> {
    let row: Option<(String, Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT name, rustdesk_id, rustdesk_password, access_mode
         FROM machines WHERE id = $1 AND tenant_id = $2",
    )
    .bind(machine_id)
    .bind(claims.tenant_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (name, rd_id, rd_password, _access_mode) =
        row.ok_or((StatusCode::NOT_FOUND, "Machine not found".into()))?;

    let has_access =
        user_has_access(&state.db, claims.sub, claims.tenant_id, machine_id, &claims.role)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    if !has_access {
        return Err((StatusCode::FORBIDDEN, "No access to this machine".into()));
    }

    let rd_id = rd_id.ok_or((
        StatusCode::CONFLICT,
        "This machine has no RustDesk ID configured. Edit the machine to add one.".into(),
    ))?;
    let password = rd_password.ok_or((
        StatusCode::CONFLICT,
        "This machine has no RustDesk password configured.".into(),
    ))?;

    let launch_uri = format!(
        "rustdesk://connect/{}?password={}",
        rd_id,
        urlencoding_minimal(&password),
    );

    let ip = addr.ip().to_string();
    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext {
            tenant_id: Some(claims.tenant_id),
            actor_id: Some(claims.sub),
            actor_email: None,
            ip_address: Some(ip),
        },
        "machine.rd_connect",
        Some("machine"),
        Some(machine_id),
        serde_json::json!({"machine_name": name, "rustdesk_id": rd_id}),
    )
    .await;

    Ok(Json(RdConnectResponse {
        rustdesk_id: rd_id,
        password,
        launch_uri,
    }))
}

/// Minimal URL encoder for the password portion of a `rustdesk://` URI.
/// Avoid pulling in the full `url` crate — we only need to escape reserved
/// characters that could break a URI query string.
fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub async fn delete_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    // Grab name for audit before deleting
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM machines WHERE id = $1 AND tenant_id = $2")
        .bind(machine_id).bind(claims.tenant_id)
        .fetch_optional(&state.db).await.ok().flatten();

    let result = sqlx::query("DELETE FROM machines WHERE id = $1 AND tenant_id = $2")
        .bind(machine_id)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "Machine not found".into()))
    } else {
        crate::audit::log(
            &state.db,
            &crate::audit::ctx_from_claims(&claims),
            "machine.deleted",
            Some("machine"),
            Some(machine_id),
            serde_json::json!({"name": name}),
        ).await;
        Ok(StatusCode::NO_CONTENT)
    }
}

// --- Machine access list ---

#[derive(Serialize, sqlx::FromRow)]
pub struct AccessUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
}

pub async fn list_machine_access(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<Vec<AccessUser>>, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    let users: Vec<AccessUser> = sqlx::query_as(
        "SELECT u.id as user_id, u.email, u.display_name
         FROM machine_access ma
         JOIN users u ON ma.user_id = u.id
         WHERE ma.machine_id = $1 AND ma.tenant_id = $2
         ORDER BY u.display_name",
    )
    .bind(machine_id)
    .bind(claims.tenant_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct GrantAccessRequest {
    pub user_id: Uuid,
}

pub async fn grant_machine_access(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
    Json(req): Json<GrantAccessRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    sqlx::query(
        "INSERT INTO machine_access (tenant_id, machine_id, user_id) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(claims.tenant_id)
    .bind(machine_id)
    .bind(req.user_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke_machine_access(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path((machine_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    sqlx::query(
        "DELETE FROM machine_access WHERE machine_id = $1 AND user_id = $2 AND tenant_id = $3",
    )
    .bind(machine_id)
    .bind(user_id)
    .bind(claims.tenant_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}
