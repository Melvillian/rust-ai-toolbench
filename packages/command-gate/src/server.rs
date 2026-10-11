//! HTTP surface: an axum router serving the embedded UI and the JSON API.
//!
//! Every request passes [`guard`] first, which enforces, in order:
//! 1. **Network identity** — in Tailscale mode, a `Tailscale-User-Login`
//!    header (set by `tailscale serve`) naming an allowed login.
//! 2. **Same-origin writes** — every non-GET must carry `Origin` equal to the
//!    configured origin (CSRF defence on top of `SameSite=Strict`).
//! 3. **Hardening headers** — strict CSP, no framing, no caching, etc.
//!
//! Command listing and execution additionally require a passkey session.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::{self, HeaderMap, HeaderName, HeaderValue};
use axum::http::{Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Semaphore;
use tower_http::trace::TraceLayer;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

use crate::args::render_argv;
use crate::audit::{AuditEntry, AuditLog};
use crate::auth::{Auth, AuthError};
use crate::config::{Config, Mode, ParamSpec};
use crate::exec;

const MAX_BODY_BYTES: usize = 64 * 1024;
const TAILSCALE_LOGIN_HEADER: &str = "tailscale-user-login";

const INDEX_HTML: &str = include_str!("ui/index.html");
const APP_JS: &str = include_str!("ui/app.js");
const APP_CSS: &str = include_str!("ui/app.css");

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
                   img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

#[derive(Debug, Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    config: Config,
    auth: Auth,
    audit: AuditLog,
    /// One command at a time: a phone double-tap must not run things twice
    /// concurrently, and it bounds resource use.
    exec_slot: Arc<Semaphore>,
    inherited_env: Vec<(String, String)>,
}

impl AppState {
    pub fn new(config: Config, auth: Auth) -> Self {
        let audit = AuditLog::new(config.server.data_dir.join("audit.jsonl"));
        let inherited_env = config
            .server
            .pass_env
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| (k.clone(), v)))
            .collect();
        Self {
            inner: Arc::new(Inner {
                config,
                auth,
                audit,
                exec_slot: Arc::new(Semaphore::new(1)),
                inherited_env,
            }),
        }
    }

    pub fn auth(&self) -> &Auth {
        &self.inner.auth
    }

    fn dev(&self) -> bool {
        self.inner.config.server.mode == Mode::Dev
    }

    /// `__Host-` cookies must be `Secure`, which browsers refuse to set over
    /// plain http; dev mode on `http://localhost` uses an unprefixed name.
    fn cookie_name(&self) -> &'static str {
        if self.dev() {
            "cg_session"
        } else {
            "__Host-cg_session"
        }
    }
}

/// The Tailscale identity of the caller (always `None` in dev mode).
#[derive(Debug, Clone)]
struct Identity(Option<String>);

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { static_file("text/html; charset=utf-8", INDEX_HTML) }),
        )
        .route(
            "/app.js",
            get(|| async { static_file("text/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/app.css",
            get(|| async { static_file("text/css; charset=utf-8", APP_CSS) }),
        )
        .route("/api/session", get(session_status))
        .route("/api/register/start", post(register_start))
        .route("/api/register/finish", post(register_finish))
        .route("/api/login/start", post(login_start))
        .route("/api/login/finish", post(login_finish))
        .route("/api/logout", post(logout))
        .route("/api/commands", get(list_commands))
        .route("/api/exec", post(execute))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn static_file(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl ApiError {
    fn new(status: StatusCode, msg: impl Into<String>) -> Self {
        Self(status, msg.into())
    }
    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "not signed in")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<AuthError> for ApiError {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::Denied => Self::new(StatusCode::UNAUTHORIZED, "authentication failed"),
            AuthError::NoPasskeys => Self::new(StatusCode::CONFLICT, e.to_string()),
            other => {
                tracing::error!("auth internal error: {other}");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

async fn guard(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let server = &state.inner.config.server;

    let identity = match server.mode {
        Mode::Dev => None,
        Mode::Tailscale => {
            let login = req
                .headers()
                .get(TAILSCALE_LOGIN_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            match login {
                Some(l) if server.allowed_tailscale_logins.contains(&l) => Some(l),
                other => {
                    tracing::warn!(login = ?other, path = %req.uri().path(), "rejected: tailscale identity");
                    return with_security_headers(
                        ApiError::new(StatusCode::FORBIDDEN, "forbidden").into_response(),
                        &state,
                    );
                }
            }
        }
    };

    if req.method() != Method::GET && req.method() != Method::HEAD {
        let origin_ok = req
            .headers()
            .get(header::ORIGIN)
            .is_some_and(|o| o.as_bytes() == server.origin_header.as_bytes());
        // Sec-Fetch-Site is a second, browser-enforced opinion when present.
        let site_ok = req
            .headers()
            .get("sec-fetch-site")
            .is_none_or(|v| v.as_bytes() == b"same-origin");
        if !origin_ok || !site_ok {
            tracing::warn!(path = %req.uri().path(), "rejected: cross-origin write");
            return with_security_headers(
                ApiError::new(StatusCode::FORBIDDEN, "cross-origin request rejected")
                    .into_response(),
                &state,
            );
        }
    }

    req.extensions_mut().insert(Identity(identity));
    let res = next.run(req).await;
    with_security_headers(res, &state)
}

fn with_security_headers(mut res: Response, state: &AppState) -> Response {
    let h = res.headers_mut();
    let mut set = |name: HeaderName, value: &'static str| {
        h.insert(name, HeaderValue::from_static(value));
    };
    set(header::CONTENT_SECURITY_POLICY, CSP);
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::X_FRAME_OPTIONS, "DENY");
    set(header::REFERRER_POLICY, "no-referrer");
    set(header::CACHE_CONTROL, "no-store");
    set(
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    );
    set(
        HeaderName::from_static("cross-origin-resource-policy"),
        "same-origin",
    );
    set(
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=(), payment=()",
    );
    if !state.dev() {
        set(header::STRICT_TRANSPORT_SECURITY, "max-age=31536000");
    }
    res
}

fn session_cookie(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let name = state.cookie_name();
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
}

fn require_session(
    state: &AppState,
    headers: &HeaderMap,
    identity: &Identity,
) -> Result<(), ApiError> {
    let token = session_cookie(state, headers).ok_or_else(ApiError::unauthorized)?;
    if state.auth().check_session(&token, identity.0.as_deref()) {
        Ok(())
    } else {
        Err(ApiError::unauthorized())
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct SessionStatus {
    authenticated: bool,
    enrolled_passkeys: usize,
    enrollment_open: bool,
    login: Option<String>,
}

async fn session_status(
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    headers: HeaderMap,
) -> Json<SessionStatus> {
    Json(SessionStatus {
        authenticated: require_session(&state, &headers, &identity).is_ok(),
        enrolled_passkeys: state.auth().passkey_count(),
        enrollment_open: state.auth().enrollment_open(),
        login: identity.0,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterStart {
    token: String,
}

async fn register_start(
    State(state): State<AppState>,
    Json(body): Json<RegisterStart>,
) -> Result<impl IntoResponse, ApiError> {
    let (ceremony, options) = state.auth().start_registration(&body.token)?;
    Ok(Json(json!({ "ceremony": ceremony, "options": options })))
}

#[derive(Debug, Deserialize)]
struct RegisterFinish {
    ceremony: String,
    credential: RegisterPublicKeyCredential,
}

async fn register_finish(
    State(state): State<AppState>,
    Json(body): Json<RegisterFinish>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .auth()
        .finish_registration(&body.ceremony, &body.credential)?;
    Ok(Json(json!({ "ok": true })))
}

async fn login_start(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (ceremony, options) = state.auth().start_login()?;
    Ok(Json(json!({ "ceremony": ceremony, "options": options })))
}

#[derive(Debug, Deserialize)]
struct LoginFinish {
    ceremony: String,
    credential: PublicKeyCredential,
}

async fn login_finish(
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(body): Json<LoginFinish>,
) -> Result<Response, ApiError> {
    let token = state
        .auth()
        .finish_login(&body.ceremony, &body.credential, identity.0.clone())?;
    tracing::info!(login = ?identity.0, "passkey login succeeded");
    let secure = if state.dev() { "" } else { "; Secure" };
    let cookie = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{secure}",
        state.cookie_name(),
        // Idle expiry is enforced (and slides) server-side; the cookie only
        // needs to outlive the absolute cap.
        crate::auth::SESSION_ABSOLUTE_MAX.as_secs()
    );
    let mut res = Json(json!({ "ok": true })).into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie)
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "cookie"))?,
    );
    Ok(res)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_cookie(&state, &headers) {
        state.auth().logout(&token);
    }
    let secure = if state.dev() { "" } else { "; Secure" };
    let clear = format!(
        "{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}",
        state.cookie_name()
    );
    let mut res = Json(json!({ "ok": true })).into_response();
    if let Ok(v) = HeaderValue::from_str(&clear) {
        res.headers_mut().insert(header::SET_COOKIE, v);
    }
    res
}

#[derive(Debug, Serialize)]
struct CommandView<'a> {
    id: &'a str,
    description: &'a str,
    /// Human-readable preview, e.g. `/usr/bin/systemctl --user restart {service}`.
    preview: String,
    timeout_secs: u64,
    params: &'a [ParamSpec],
}

async fn list_commands(
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    require_session(&state, &headers, &identity)?;
    let views: Vec<_> = state
        .inner
        .config
        .commands
        .iter()
        .map(|c| CommandView {
            id: &c.id,
            description: &c.description,
            preview: std::iter::once(c.program.display().to_string())
                .chain(c.args.iter().map(|a| match a {
                    crate::config::ArgTemplate::Literal(s) => s.clone(),
                    crate::config::ArgTemplate::Param(p) => format!("{{{p}}}"),
                }))
                .collect::<Vec<_>>()
                .join(" "),
            timeout_secs: c.timeout_secs,
            params: &c.params,
        })
        .collect();
    Ok(Json(views).into_response())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecRequest {
    id: String,
    #[serde(default)]
    args: BTreeMap<String, String>,
}

async fn execute(
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    headers: HeaderMap,
    Json(body): Json<ExecRequest>,
) -> Result<impl IntoResponse, ApiError> {
    require_session(&state, &headers, &identity)?;
    let login = identity.0.as_deref();
    let Some(spec) = state.inner.config.command(&body.id) else {
        // Log a bounded, escaped copy of the unknown id for forensics.
        let id: String = body.id.chars().take(64).collect();
        record(&state, login, &id, &[], "unknown-command", None, None).await;
        return Err(ApiError::new(StatusCode::NOT_FOUND, "unknown command"));
    };

    let argv = match render_argv(spec, &body.args) {
        Ok(a) => a,
        Err(e) => {
            record(&state, login, &spec.id, &[], "rejected-args", None, None).await;
            return Err(ApiError::new(StatusCode::BAD_REQUEST, e.to_string()));
        }
    };

    let Ok(permit) = Arc::clone(&state.inner.exec_slot).try_acquire_owned() else {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "another command is still running",
        ));
    };

    // Audit *before* running, so an action that takes effect is recorded even
    // if the server dies mid-command.
    record(&state, login, &spec.id, &argv, "started", None, None).await;

    // Run in a detached task: if the phone drops the connection, axum drops
    // this handler future, but the command still runs to completion/timeout
    // under exec's own process-group kill logic, the slot is held until it
    // truly finishes, and the outcome is still audited.
    let task_state = state.clone();
    let task_login = identity.0.clone();
    let task_spec = spec.clone();
    let task = tokio::spawn(async move {
        let _permit = permit;
        let res = exec::run(&task_spec, &argv, &task_state.inner.inherited_env).await;
        let login = task_login.as_deref();
        match &res {
            Ok(o) => {
                let label = if o.timed_out { "timeout" } else { "finished" };
                record(
                    &task_state,
                    login,
                    &task_spec.id,
                    &argv,
                    label,
                    o.exit_code,
                    Some(o.duration_ms),
                )
                .await;
            }
            Err(e) => {
                tracing::error!("{e}");
                record(
                    &task_state,
                    login,
                    &task_spec.id,
                    &argv,
                    "exec-failed",
                    None,
                    None,
                )
                .await;
            }
        }
        res
    });

    match task.await {
        Ok(Ok(outcome)) => Ok(Json(outcome)),
        Ok(Err(_)) | Err(_) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "command failed to run; see server log",
        )),
    }
}

async fn record(
    state: &AppState,
    login: Option<&str>,
    command: &str,
    argv: &[String],
    outcome: &str,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
) {
    let entry = AuditEntry {
        at: chrono::Utc::now().to_rfc3339(),
        login,
        command,
        argv,
        outcome,
        exit_code,
        duration_ms,
    };
    tracing::info!(command, outcome, ?exit_code, ?login, "audit");
    state.inner.audit.append(&entry).await;
}

/// Enrollment windows close after this long.
pub const ENROLLMENT_TTL: Duration = Duration::from_secs(15 * 60);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app(mode: Mode) -> (Router, AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let (rp, origin, logins) = match mode {
            Mode::Dev => ("localhost", "http://localhost:8787", ""),
            Mode::Tailscale => (
                "box.tail1.ts.net",
                "https://box.tail1.ts.net",
                "allowed_tailscale_logins=[\"alex@github\"]",
            ),
        };
        let text = format!(
            "[server]\nrp_id=\"{rp}\"\norigin=\"{origin}\"\n{logins}\ndata_dir={:?}\n\
             [[command]]\nid=\"echo\"\nprogram=\"/bin/echo\"\nargs=[\"{{word}}\"]\n\
             [[command.param]]\nname=\"word\"\nkind=\"word\"\n",
            tmp.path().join("d")
        );
        let config = Config::from_toml(&text, mode).unwrap();
        let auth = Auth::open(&config.server).unwrap();
        let state = AppState::new(config, auth);
        (router(state.clone()), state, tmp)
    }

    async fn body_json(res: Response) -> serde_json::Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn tailscale_mode_rejects_missing_or_unlisted_identity() {
        let (app, _, _t) = app(Mode::Tailscale);
        let res = app
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let res = app
            .clone()
            .oneshot(
                Request::get("/")
                    .header("Tailscale-User-Login", "mallory@github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let res = app
            .oneshot(
                Request::get("/")
                    .header("Tailscale-User-Login", "alex@github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert!(
            res.headers()
                .contains_key(header::STRICT_TRANSPORT_SECURITY)
        );
    }

    #[tokio::test]
    async fn writes_require_same_origin() {
        let (app, _, _t) = app(Mode::Dev);
        let req = |origin: Option<&str>| {
            let mut b = Request::post("/api/login/start");
            if let Some(o) = origin {
                b = b.header(header::ORIGIN, o);
            }
            b.body(Body::empty()).unwrap()
        };
        assert_eq!(
            app.clone().oneshot(req(None)).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.clone()
                .oneshot(req(Some("https://evil.example")))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        // Correct origin passes the guard; no passkeys yet -> 409.
        assert_eq!(
            app.oneshot(req(Some("http://localhost:8787")))
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn commands_and_exec_require_a_session() {
        let (app, _, _t) = app(Mode::Dev);
        let res = app
            .clone()
            .oneshot(Request::get("/api/commands").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let exec = Request::post("/api/exec")
            .header(header::ORIGIN, "http://localhost:8787")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, "cg_session=forged")
            .body(Body::from(r#"{"id":"echo","args":{"word":"hi"}}"#))
            .unwrap();
        let res = app.oneshot(exec).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_status_reports_enrollment_state() {
        let (app, state, _t) = app(Mode::Dev);
        let _ = state.auth().open_enrollment(ENROLLMENT_TTL);
        let res = app
            .oneshot(Request::get("/api/session").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let v = body_json(res).await;
        assert_eq!(v["authenticated"], false);
        assert_eq!(v["enrollment_open"], true);
        assert_eq!(v["enrolled_passkeys"], 0);
    }
}
