use axum::{
    body::Body,
    http::{header, StatusCode},
    response::Response,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub async fn download_agent_deb() -> Result<Response<Body>, (StatusCode, String)> {
    stream_latest("target/deb", "deb", "application/vnd.debian.binary-package").await
}

pub async fn download_agent_windows() -> Result<Response<Body>, (StatusCode, String)> {
    stream_latest("target/windows", "zip", "application/zip").await
}

async fn stream_latest(
    dir: &str,
    ext: &str,
    content_type: &str,
) -> Result<Response<Body>, (StatusCode, String)> {
    let path = std::path::Path::new(dir);
    let file_path = find_latest(path, ext).ok_or((
        StatusCode::NOT_FOUND,
        format!("No .{ext} file found in {dir}. Run the build script."),
    ))?;

    let filename = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file = File::open(&file_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cannot open file: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Response error: {e}")))
}

fn find_latest(dir: &std::path::Path, ext: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .max_by_key(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
}
