use axum::{extract::State, http::StatusCode, Json};
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

fn generate_agent_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    format!("cmt_{}", hex::encode(bytes))
}

pub async fn list_machines(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<Machine>>, (StatusCode, String)> {
    let machines: Vec<Machine> =
        sqlx::query_as("SELECT * FROM machines WHERE tenant_id = $1 ORDER BY name")
            .bind(claims.tenant_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(machines))
}

pub async fn create_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<CreateMachineRequest>,
) -> Result<(StatusCode, Json<CreateMachineResponse>), (StatusCode, String)> {
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
    axum::extract::Path(machine_id): axum::extract::Path<Uuid>,
) -> Result<Json<Machine>, (StatusCode, String)> {
    let machine: Option<Machine> =
        sqlx::query_as("SELECT * FROM machines WHERE id = $1 AND tenant_id = $2")
            .bind(machine_id)
            .bind(claims.tenant_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    machine
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Machine not found".into()))
}

pub async fn delete_machine(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    axum::extract::Path(machine_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
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
