use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct TestEmailRequest {
    pub to: String,
}

#[derive(Serialize)]
pub struct TestEmailResponse {
    pub sent: bool,
    pub message: String,
}

pub async fn send_test_email(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<TestEmailRequest>,
) -> Result<Json<TestEmailResponse>, (StatusCode, String)> {
    if claims.role != "owner" && claims.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Only owners/admins can test email".into()));
    }

    let Some(smtp) = crate::email::EmailConfig::load(&state.db).await else {
        return Ok(Json(TestEmailResponse {
            sent: false,
            message: "SMTP not configured. Configure it in the admin panel → Settings.".into(),
        }));
    };

    let subject = "Callmor SMTP Test";
    let html = r#"<html><body style="font-family:system-ui,sans-serif">
        <h2>SMTP is working!</h2>
        <p>If you received this email, your Callmor SMTP configuration is correct.</p>
      </body></html>"#;
    let text = "SMTP is working!\n\nIf you received this email, your Callmor SMTP configuration is correct.";

    match smtp.send(&req.to, subject, html, text).await {
        Ok(()) => Ok(Json(TestEmailResponse {
            sent: true,
            message: format!("Test email sent to {}", req.to),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Send failed: {e:#}"),
        )),
    }
}
