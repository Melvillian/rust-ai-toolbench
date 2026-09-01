//! Route definitions for the API server.
//!
//! [`create_app`] is runtime-agnostic: it returns a plain [`Router`] that
//! `main.rs` serves over a TCP listener and that the tests drive directly with
//! `tower::ServiceExt::oneshot` (no socket, no spawned server).

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use serde::Serialize;
use tower_http::trace::TraceLayer;

/// Maximum accepted request body size: 10 MiB — parity with the pre-migration
/// middleware's limit. Enforced by [`DefaultBodyLimit`] whenever a handler
/// actually reads the body (extractors such as `Bytes`, `String`, `Json`).
/// None of the current handlers read a body, so the layer is a guard rail for
/// future routes rather than something the tests can observe today.
pub const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct Health {
    status: &'static str,
    /// RFC 3339 UTC timestamp, e.g. `2026-08-31T12:34:56.789Z`.
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct Hello {
    message: &'static str,
}

#[derive(Debug, Serialize)]
struct Info {
    service: &'static str,
    version: &'static str,
    endpoints: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: &'static str,
}

/// Build the application router.
pub fn create_app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/hello", get(hello))
        .route("/api/info", get(info))
        // Unknown /api paths are JSON 404s. `/api/{*rest}` does not match bare
        // `/api`, so register both. The web app (apps/web) is NOT served here —
        // it deploys as a Render static site whose /api/* rewrite proxies to
        // this service (see the render-deploys skill).
        .route("/api", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        // Hono's `app.all('/api/*')` also swallowed wrong-method requests to the
        // known routes (e.g. `POST /api/hello`) with the JSON 404. axum would
        // answer 405 by default; this fallback restores the JSON 404 for parity.
        .method_not_allowed_fallback(api_not_found)
        // Everything else (`/`, `/about`, ...) gets axum's default empty 404.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    })
}

async fn hello() -> Json<Hello> {
    Json(Hello {
        message: "Hello from the api endpoint!",
    })
}

async fn info() -> Json<Info> {
    Json(Info {
        service: "api",
        version: env!("CARGO_PKG_VERSION"),
        endpoints: &["/health", "/api/hello", "/api/info"],
    })
}

async fn api_not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(ApiError { error: "not found" })).into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::create_app;

    async fn send(method: Method, path: &str) -> (StatusCode, Option<String>, Vec<u8>) {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .expect("valid request");
        let res = create_app().oneshot(req).await.expect("infallible service");
        let status = res.status();
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = res.into_body().collect().await.expect("body").to_bytes();
        (status, content_type, body.to_vec())
    }

    async fn get_json(path: &str) -> (StatusCode, Value) {
        let (status, content_type, body) = send(Method::GET, path).await;
        assert_eq!(
            content_type.as_deref(),
            Some("application/json"),
            "{path} should respond with JSON"
        );
        let value = serde_json::from_slice(&body).expect("JSON body");
        (status, value)
    }

    #[tokio::test]
    async fn health_returns_ok_with_a_valid_timestamp() {
        let (status, body) = get_json("/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        let ts = body["timestamp"].as_str().expect("timestamp is a string");
        chrono::DateTime::parse_from_rfc3339(ts).expect("timestamp is RFC 3339");
    }

    #[tokio::test]
    async fn hello_returns_the_greeting() {
        let (status, body) = get_json("/api/hello").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "message": "Hello from the api endpoint!" }));
    }

    #[tokio::test]
    async fn info_returns_service_metadata() {
        let (status, body) = get_json("/api/info").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({
                "service": "api",
                "version": env!("CARGO_PKG_VERSION"),
                "endpoints": ["/health", "/api/hello", "/api/info"],
            })
        );
    }

    #[tokio::test]
    async fn unknown_api_paths_return_a_json_404() {
        for path in ["/api", "/api/nope", "/api/nested/deeper"] {
            let (status, body) = get_json(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(body, json!({ "error": "not found" }), "{path}");
        }
    }

    #[tokio::test]
    async fn wrong_method_on_known_api_route_returns_a_json_404() {
        // Hono's `app.all('/api/*')` catch-all answered these with the JSON 404;
        // `method_not_allowed_fallback` keeps that behaviour instead of a 405.
        for (method, path) in [
            (Method::POST, "/api/hello"),
            (Method::DELETE, "/api/info"),
            (Method::PUT, "/api/nope"),
        ] {
            let (status, content_type, body) = send(method.clone(), path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
            assert_eq!(content_type.as_deref(), Some("application/json"));
            let body: Value = serde_json::from_slice(&body).expect("JSON body");
            assert_eq!(body, json!({ "error": "not found" }), "{method} {path}");
        }
    }

    #[tokio::test]
    async fn static_api_routes_win_over_the_wildcard_404() {
        // `/api/hello` and `/api/info` must not be shadowed by `/api/{*rest}`.
        let (status, body) = get_json("/api/hello").await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(body, json!({ "error": "not found" }));
        let (status, _) = get_json("/api/info").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn non_api_paths_404_because_the_web_app_is_a_static_site() {
        for path in ["/", "/about"] {
            let (status, _, body) = send(Method::GET, path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(body.is_empty(), "{path} should be axum's plain 404");
        }
    }
}
