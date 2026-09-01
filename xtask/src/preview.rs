//! The stand-in for `vite preview`: serves the production `dx bundle` output
//! directory with an SPA fallback and proxies `/api/*` and `/health` to the
//! api server — the same shape as the Render static site's prod rewrite.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use tower_http::services::{ServeDir, ServeFile};

/// Default listen port (matches `vite preview`).
pub const DEFAULT_PORT: u16 = 4173;
/// Default api origin the proxy forwards to.
pub const DEFAULT_API: &str = "http://localhost:8080";

/// True for paths the static site rewrites to the api service.
pub fn should_proxy(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/health"
}

/// Build the upstream URL for a proxied request.
pub fn proxy_target(api_base: &str, path: &str, query: Option<&str>) -> String {
    let base = api_base.trim_end_matches('/');
    match query {
        Some(q) if !q.is_empty() => format!("{base}{path}?{q}"),
        _ => format!("{base}{path}"),
    }
}

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    api_base: Arc<str>,
}

async fn proxy(State(state): State<ProxyState>, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let target = proxy_target(&state.api_base, parts.uri.path(), parts.uri.query());
    let Ok(body) = axum::body::to_bytes(body, 10 * 1024 * 1024).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };

    let mut upstream = state.client.request(parts.method, &target).body(body);
    for name in [header::CONTENT_TYPE, header::ACCEPT, header::AUTHORIZATION] {
        if let Some(value) = parts.headers.get(&name) {
            upstream = upstream.header(name, value);
        }
    }

    match upstream.send().await {
        Ok(res) => {
            let status = res.status();
            let content_type = res.headers().get(header::CONTENT_TYPE).cloned();
            match res.bytes().await {
                Ok(bytes) => {
                    let mut response = Response::new(Body::from(bytes));
                    *response.status_mut() = status;
                    if let Some(ct) = content_type {
                        response.headers_mut().insert(header::CONTENT_TYPE, ct);
                    }
                    response
                }
                Err(err) => bad_gateway(&target, &err),
            }
        }
        Err(err) => bad_gateway(&target, &err),
    }
}

fn bad_gateway(target: &str, err: &dyn std::fmt::Display) -> Response {
    let mut response = (
        StatusCode::BAD_GATEWAY,
        format!("preview proxy: {target} unreachable ({err}) — is the api running?"),
    )
        .into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

/// The preview router: proxy routes first, static files + SPA fallback last.
pub fn router(dir: &Path, api_base: &str) -> Router {
    let state = ProxyState {
        client: reqwest::Client::new(),
        api_base: Arc::from(api_base.trim_end_matches('/')),
    };
    let index = dir.join("index.html");
    let static_files = ServeDir::new(dir).fallback(ServeFile::new(index));
    Router::new()
        .route("/api", any(proxy))
        .route("/api/{*rest}", any(proxy))
        .route("/health", any(proxy))
        .with_state(state)
        .fallback_service(static_files)
}

/// Bind and serve. `dir` must contain an `index.html`.
pub async fn serve(dir: PathBuf, port: u16, api_base: &str) -> anyhow::Result<()> {
    if !dir.join("index.html").is_file() {
        bail!(
            "{} has no index.html — run `cargo xtask build` first",
            dir.display()
        );
    }
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .with_context(|| format!("failed to bind port {port}"))?;
    eprintln!("Preview server listening on http://localhost:{port} (proxying /api → {api_base})");
    axum::serve(listener, router(&dir, api_base)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_proxy_paths() {
        assert!(should_proxy("/api"));
        assert!(should_proxy("/api/hello"));
        assert!(should_proxy("/health"));
        assert!(!should_proxy("/apix"));
        assert!(!should_proxy("/"));
        assert!(!should_proxy("/healthz"));
    }

    #[test]
    fn builds_target_urls_with_optional_query() {
        assert_eq!(
            proxy_target("http://localhost:8080/", "/api/hello", None),
            "http://localhost:8080/api/hello"
        );
        assert_eq!(
            proxy_target("http://localhost:8080", "/api/hello", Some("x=1&y=2")),
            "http://localhost:8080/api/hello?x=1&y=2"
        );
        assert_eq!(
            proxy_target("http://localhost:8080", "/health", Some("")),
            "http://localhost:8080/health"
        );
    }
}
