use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;
use callmor_shared::Machine;

#[derive(Deserialize)]
pub struct CreateMachineRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateMachineResponse {
    pub id: Uuid,
    pub name: String,
    pub agent_token: String,
}

#[derive(Deserialize)]
pub struct UpdateMachineRequest {
    pub name: Option<String>,
    pub access_mode: Option<String>,
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

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO machines (tenant_id, name, agent_token) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(claims.tenant_id)
    .bind(&req.name)
    .bind(&token)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateMachineResponse {
            id,
            name: req.name,
            agent_token: token,
        }),
    ))
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

    let result = sqlx::query(
        "UPDATE machines SET
           name = COALESCE($1, name),
           access_mode = COALESCE($2, access_mode)
         WHERE id = $3 AND tenant_id = $4",
    )
    .bind(req.name.as_deref())
    .bind(req.access_mode.as_deref())
    .bind(machine_id)
    .bind(claims.tenant_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "Machine not found".into()))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

pub async fn delete_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(machine_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    let result = sqlx::query("DELETE FROM machines WHERE id = $1 AND tenant_id = $2")
        .bind(machine_id)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "Machine not found".into()))
    } else {
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
