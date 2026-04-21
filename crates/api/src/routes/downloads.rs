use axum::{
    body::Body,
    http::{header, StatusCode},
    response::Response,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub async fn download_agent_deb() -> Result<Response<Body>, (StatusCode, String)> {
    // Look for the .deb in the target/deb directory
    let deb_dir = std::path::Path::new("target/deb");
    let deb_file = find_latest_deb(deb_dir)
        .ok_or((StatusCode::NOT_FOUND, "Agent .deb not built yet. Run scripts/build-deb.sh first.".into()))?;

    let filename = deb_file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file = File::open(&deb_file)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cannot open file: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .header(header::CONTENT_TYPE, "application/vnd.debian.binary-package")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Response error: {e}")))
}

fn find_latest_deb(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "deb"))
        .max_by_key(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
}
