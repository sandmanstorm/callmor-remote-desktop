use anyhow::{Context, Result};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone)]
pub struct EmailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub tls_mode: TlsMode,
}

#[derive(Clone, Copy, Debug)]
pub enum TlsMode {
    StartTls,
    Implicit,
    None,
}

impl EmailConfig {
    /// Load from environment. Returns None if SMTP_HOST is not set (graceful degradation).
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("SMTP_HOST").ok()?;
        if host.is_empty() {
            return None;
        }

        let port: u16 = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(587);

        let username = std::env::var("SMTP_USERNAME").unwrap_or_default();
        let password = std::env::var("SMTP_PASSWORD").unwrap_or_default();

        let from_email = std::env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| username.clone());
        let from_name = std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Callmor Remote".into());

        let tls_mode = match std::env::var("SMTP_TLS")
            .unwrap_or_else(|_| "starttls".into())
            .to_lowercase()
            .as_str()
        {
            "implicit" | "tls" => TlsMode::Implicit,
            "none" | "plain" => TlsMode::None,
            _ => TlsMode::StartTls,
        };

        Some(EmailConfig {
            host,
            port,
            username,
            password,
            from_email,
            from_name,
            tls_mode,
        })
    }

    fn build_transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let tls_params = TlsParameters::new(self.host.clone())?;

        let builder = match self.tls_mode {
            TlsMode::Implicit => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
                .port(self.port)
                .tls(Tls::Wrapper(tls_params)),
            TlsMode::StartTls => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
                .port(self.port)
                .tls(Tls::Required(tls_params)),
            TlsMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
                .port(self.port)
                .tls(Tls::None),
        };

        Ok(builder.credentials(creds).build())
    }

    fn from_mailbox(&self) -> Result<Mailbox> {
        Ok(format!("{} <{}>", self.from_name, self.from_email).parse()?)
    }

    /// Send a simple HTML + text email.
    pub async fn send(&self, to: &str, subject: &str, html: &str, text: &str) -> Result<()> {
        let to_mailbox: Mailbox = to.parse().context("Invalid recipient email")?;

        let email = Message::builder()
            .from(self.from_mailbox()?)
            .to(to_mailbox)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.to_string()),
                    ),
            )?;

        let transport = self.build_transport()?;
        transport.send(email).await.context("SMTP send failed")?;
        Ok(())
    }
}

/// Build the invitation email (HTML + plain text).
pub fn invitation_email(
    invitee_email: &str,
    inviter_name: &str,
    tenant_name: &str,
    role: &str,
    invite_link: &str,
) -> (String, String, String) {
    let subject = format!("{inviter_name} invited you to {tenant_name} on Callmor");

    let text = format!(
        "Hi,\n\n\
         {inviter_name} has invited you to join {tenant_name} on Callmor Remote Desktop as a {role}.\n\n\
         Accept the invitation here:\n{invite_link}\n\n\
         This link expires in 7 days.\n\n\
         If you weren't expecting this, you can ignore this email.\n\n\
         — Callmor Remote\n"
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head>
<body style="font-family:system-ui,sans-serif;background:#f5f5f5;padding:20px;margin:0;">
  <div style="max-width:560px;margin:0 auto;background:#fff;border-radius:8px;padding:32px;box-shadow:0 1px 3px rgba(0,0,0,0.1);">
    <h1 style="margin:0 0 16px;color:#111;font-size:22px;">You've been invited to join <strong>{tenant_name}</strong></h1>
    <p style="color:#444;line-height:1.5;font-size:15px;">
      <strong>{inviter_name}</strong> has invited <strong>{invitee_email}</strong> to join <strong>{tenant_name}</strong> on Callmor Remote Desktop as a <strong>{role}</strong>.
    </p>
    <p style="margin:28px 0;text-align:center;">
      <a href="{invite_link}"
         style="display:inline-block;background:#2563eb;color:#fff;padding:12px 24px;border-radius:6px;text-decoration:none;font-weight:600;">
        Accept Invitation
      </a>
    </p>
    <p style="color:#666;font-size:13px;">Or copy this link into your browser:</p>
    <p style="color:#2563eb;font-size:12px;word-break:break-all;background:#f0f4ff;padding:10px;border-radius:4px;border:1px solid #dbe4ff;">{invite_link}</p>
    <p style="color:#999;font-size:12px;margin-top:24px;">This invitation expires in 7 days. If you weren't expecting this, you can ignore this email.</p>
    <hr style="border:none;border-top:1px solid #eee;margin:24px 0;">
    <p style="color:#aaa;font-size:11px;">Callmor Remote Desktop</p>
  </div>
</body></html>"#
    );

    (subject, html, text)
}
