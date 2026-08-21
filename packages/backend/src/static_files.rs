//! 将前端与 CLI 安装包嵌入二进制，供发布态同一进程托管。

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

const CLI_DOWNLOAD_URL_PLACEHOLDER: &str = "__CHUNSUN_CLI_DOWNLOAD_URL__";

#[derive(Embed)]
#[folder = "$OUT_DIR/web"]
struct WebAssets;

#[derive(Embed)]
#[folder = "$OUT_DIR/cli-scripts"]
struct CliScripts;

#[derive(Embed)]
#[folder = "$OUT_DIR/cli-dist"]
struct CliDist;

pub async fn fallback(req: Request) -> Response {
    if req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = req.uri().path();
    if path.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some(rest) = strip_cli_prefix(path) {
        return serve_cli(&req, rest);
    }
    serve_web(path, req.method() == axum::http::Method::HEAD)
}

fn strip_cli_prefix(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    if path == "/cli" {
        return Some("");
    }
    path.strip_prefix("/cli/")
}

fn serve_cli(req: &Request, relative: &str) -> Response {
    if relative.is_empty() || relative.contains("..") {
        return (StatusCode::NOT_FOUND, "Not Found\n").into_response();
    }
    if relative == "install.sh" || relative == "install.ps1" {
        let Some(file) = CliScripts::get(relative) else {
            return (StatusCode::NOT_FOUND, "Not Found\n").into_response();
        };
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost:11111");
        let proto = req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("http");
        let cli_base = format!("{proto}://{host}/cli");
        let body = String::from_utf8_lossy(&file.data).replace(CLI_DOWNLOAD_URL_PLACEHOLDER, &cli_base);
        return text_plain(body, req.method() == axum::http::Method::HEAD);
    }
    if relative == ".keep" {
        return (StatusCode::NOT_FOUND, "Not Found\n").into_response();
    }
    let Some(file) = CliDist::get(relative) else {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "[chunsun] CLI 产物未编入此二进制: {relative}\n请使用 pnpm run platform:release 发布（会自动交叉编译 CLI）。\n"
            ),
        )
            .into_response();
    };
    let res = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(header::CACHE_CONTROL, "no-store");
    if req.method() == axum::http::Method::HEAD {
        res.body(Body::empty()).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    } else {
        res.body(Body::from(file.data.into_owned()))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

fn is_console_path(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    p == "/console" || p.starts_with("/console/")
}

fn shell_html_for(path: &str) -> &'static str {
    if is_console_path(path) {
        "console/index.html"
    } else {
        "index.html"
    }
}

fn serve_web(path: &str, head: bool) -> Response {
    let rel = path.trim_start_matches('/');
    let candidate = if rel.is_empty() { "index.html" } else { rel };
    if let Some(file) = WebAssets::get(candidate) {
        return file_response(&file, mime_for(candidate), head);
    }
    let looks_like_file = std::path::Path::new(candidate).extension().is_some();
    if looks_like_file {
        return StatusCode::NOT_FOUND.into_response();
    }
    match WebAssets::get(shell_html_for(path)) {
        Some(file) => file_response(&file, "text/html; charset=utf-8", head),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn file_response(file: &rust_embed::EmbeddedFile, mime: &str, head: bool) -> Response {
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    if head {
        builder
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    } else {
        builder
            .body(Body::from(file.data.clone().into_owned()))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

fn mime_for(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff2" => "font/woff2",
        "json" => "application/json",
        "map" => "application/json",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn text_plain(body: String, head: bool) -> Response {
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store");
    if head {
        builder.body(Body::empty()).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    } else {
        builder
            .body(Body::from(body))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_prefix_strips() {
        assert_eq!(strip_cli_prefix("/cli/install.sh"), Some("install.sh"));
        assert_eq!(strip_cli_prefix("/cli"), Some(""));
        assert_eq!(strip_cli_prefix("/docs"), None);
    }

    #[test]
    fn shell_html_picks_console_for_product_paths() {
        assert_eq!(shell_html_for("/console"), "console/index.html");
        assert_eq!(shell_html_for("/console/auth/login"), "console/index.html");
        assert_eq!(shell_html_for("/console/projects"), "console/index.html");
        assert_eq!(shell_html_for("/"), "index.html");
        assert_eq!(shell_html_for("/docs"), "index.html");
        assert_eq!(shell_html_for("/unknown"), "index.html");
    }
}
