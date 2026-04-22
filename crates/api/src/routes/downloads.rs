//! Installer downloads with per-tenant enrollment-token injection.
//!
//! The prebuilt installers (.deb, NSIS .exe, macOS .pkg) contain a fixed
//! 36-byte placeholder `cle_INSTALLER_TOKEN_PLACEHOLDER_XXXX` where the
//! tenant's real enrollment token would go. The Windows .exe stores strings
//! as UTF-16LE so the placeholder appears as 72 bytes there.
//!
//! On download, we:
//!   1. authenticate the caller (header or `?token=` query param)
//!   2. look up their tenant's enrollment_token (generated at signup)
//!   3. read the installer into memory
//!   4. byte-replace the placeholder with the real token
//!   5. stream the result
//!
//! This gives the end user a single file with zero manual config.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::jwt::Claims;
use crate::state::AppState;

/// The 36-byte placeholder baked into every installer. Real enrollment tokens
/// are also 36 bytes (`cle_` + 32 hex), so replacement is length-preserving
/// and doesn't shift any other bytes in the file.
const PLACEHOLDER_ASCII: &str = "cle_INSTALLER_TOKEN_PLACEHOLDER_XXXX";

#[derive(Deserialize)]
pub struct DownloadAuth {
    token: Option<String>,
}

/// Pull the user's JWT from either `Authorization: Bearer ...` or `?token=...`.
/// We need query-param auth because browser `<a href>` downloads can't set
/// headers.
fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    query: &DownloadAuth,
) -> Result<Claims, (StatusCode, String)> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(String::from)
        .or_else(|| query.token.clone())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".into()))?;

    state
        .jwt
        .validate_token(&token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token".into()))
}

async fn fetch_enrollment_token(db: &PgPool, tenant_id: Uuid) -> Result<String, (StatusCode, String)> {
    sqlx::query_scalar::<_, String>("SELECT enrollment_token FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))
}

pub async fn download_agent_deb(
    State(state): State<AppState>,
    Query(q): Query<DownloadAuth>,
    headers: HeaderMap,
) -> Result<Response<Body>, (StatusCode, String)> {
    let claims = authenticate(&state, &headers, &q)?;
    let token = fetch_enrollment_token(&state.db, claims.tenant_id).await?;
    serve_with_injection(
        "target/deb",
        "deb",
        "application/vnd.debian.binary-package",
        &token,
        Encoding::Ascii,
    )
    .await
}

pub async fn download_agent_windows(
    State(state): State<AppState>,
    Query(q): Query<DownloadAuth>,
    headers: HeaderMap,
) -> Result<Response<Body>, (StatusCode, String)> {
    let claims = authenticate(&state, &headers, &q)?;
    let token = fetch_enrollment_token(&state.db, claims.tenant_id).await?;
    // NSIS with `Unicode true` stores strings as UTF-16LE in the installer.
    serve_with_injection(
        "target/windows",
        "exe",
        "application/vnd.microsoft.portable-executable",
        &token,
        Encoding::Utf16Le,
    )
    .await
}

pub async fn download_agent_macos(
    State(state): State<AppState>,
    Query(q): Query<DownloadAuth>,
    headers: HeaderMap,
) -> Result<Response<Body>, (StatusCode, String)> {
    let claims = authenticate(&state, &headers, &q)?;
    let token = fetch_enrollment_token(&state.db, claims.tenant_id).await?;
    // macOS .pkg is still built via GitHub Actions; until that workflow is
    // switched to emit an uncompressed archive we inject as ASCII (works for
    // unsigned/uncompressed payload; harmless otherwise — if no match is
    // found we return the file unchanged rather than fail).
    serve_with_injection(
        "target/macos",
        "pkg",
        "application/vnd.apple.installer+xml",
        &token,
        Encoding::Ascii,
    )
    .await
}

#[derive(Clone, Copy)]
enum Encoding {
    Ascii,
    Utf16Le,
}

async fn serve_with_injection(
    dir: &str,
    ext: &str,
    content_type: &str,
    real_token: &str,
    encoding: Encoding,
) -> Result<Response<Body>, (StatusCode, String)> {
    let file_path = find_latest(std::path::Path::new(dir), ext).ok_or((
        StatusCode::NOT_FOUND,
        format!("No .{ext} installer found in {dir}. Run the build script."),
    ))?;
    let filename = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let mut data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {e}")))?;

    inject_token(&mut data, real_token, encoding);

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, data.len().to_string())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(data))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Response error: {e}")))
}

/// Replace the placeholder bytes in-place with the real token. Both are the
/// same length (36 chars ASCII = 72 bytes UTF-16LE), so no byte offsets shift.
/// Silently leaves the file unchanged if no placeholder is found — makes the
/// handler safe to point at an installer that wasn't built with the placeholder.
fn inject_token(data: &mut [u8], real_token: &str, encoding: Encoding) {
    // Both placeholder and real token must be 36 ASCII chars.
    debug_assert_eq!(PLACEHOLDER_ASCII.len(), 36);

    match encoding {
        Encoding::Ascii => {
            let needle = PLACEHOLDER_ASCII.as_bytes();
            let replacement = real_token.as_bytes();
            if needle.len() != replacement.len() {
                tracing::warn!("enrollment token length mismatch, skipping injection");
                return;
            }
            replace_all_inplace(data, needle, replacement);
        }
        Encoding::Utf16Le => {
            let needle: Vec<u8> = PLACEHOLDER_ASCII.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
            let replacement: Vec<u8> = real_token.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
            if needle.len() != replacement.len() {
                tracing::warn!("enrollment token UTF-16 length mismatch, skipping injection");
                return;
            }
            replace_all_inplace(data, &needle, &replacement);
        }
    }
}

/// In-place, length-preserving byte replacement. Scans linearly; we don't
/// need anything fancier since files are ~20MB and the needle is unique.
fn replace_all_inplace(haystack: &mut [u8], needle: &[u8], replacement: &[u8]) {
    assert_eq!(needle.len(), replacement.len());
    if needle.is_empty() || haystack.len() < needle.len() {
        return;
    }
    let mut i = 0;
    let end = haystack.len() - needle.len();
    while i <= end {
        if &haystack[i..i + needle.len()] == needle {
            haystack[i..i + needle.len()].copy_from_slice(replacement);
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

fn find_latest(dir: &std::path::Path, ext: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .max_by_key(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_replacement_preserves_length_and_other_bytes() {
        let prefix = b"HEADER:";
        let suffix = b":TRAILER";
        let placeholder = PLACEHOLDER_ASCII.as_bytes();
        let real = "cle_aaaabbbbccccddddeeeeffff00001111".as_bytes();
        assert_eq!(real.len(), placeholder.len());

        let mut data = Vec::new();
        data.extend_from_slice(prefix);
        data.extend_from_slice(placeholder);
        data.extend_from_slice(suffix);

        let original_len = data.len();
        inject_token(&mut data, std::str::from_utf8(real).unwrap(), Encoding::Ascii);

        assert_eq!(data.len(), original_len);
        assert!(data.starts_with(prefix));
        assert!(data.ends_with(suffix));
        assert!(data.windows(real.len()).any(|w| w == real));
        assert!(!data.windows(placeholder.len()).any(|w| w == placeholder));
    }

    #[test]
    fn utf16_replacement_preserves_length() {
        let real = "cle_aaaabbbbccccddddeeeeffff00001111";
        let placeholder_u16: Vec<u8> =
            PLACEHOLDER_ASCII.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();

        let mut data = Vec::new();
        data.extend_from_slice(b"\x00\x00");
        data.extend_from_slice(&placeholder_u16);
        data.extend_from_slice(b"\xff\xff");
        let original_len = data.len();

        inject_token(&mut data, real, Encoding::Utf16Le);
        assert_eq!(data.len(), original_len);

        let real_u16: Vec<u8> = real.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        assert!(data.windows(real_u16.len()).any(|w| w == real_u16));
    }

    #[test]
    fn no_placeholder_is_noop() {
        let mut data = b"just some random bytes no placeholder here".to_vec();
        let before = data.clone();
        inject_token(&mut data, "cle_aaaabbbbccccddddeeeeffff00001111", Encoding::Ascii);
        assert_eq!(data, before);
    }
}
