//! End-to-end: enroll a (software) passkey, sign in, run commands, and try to
//! break out — all through the real router, cookies and JSON wire format.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use command_gate::auth::Auth;
use command_gate::server::{AppState, router};
use command_gate::{Config, Mode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

const ORIGIN: &str = "http://localhost:8787";

fn build(dir: &Path) -> (Router, AppState) {
    let root = dir.join("files");
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::write(root.join("logs/app.log"), "line1\nline2\n").unwrap();
    let text = format!(
        r#"
[server]
rp_id = "localhost"
origin = "{ORIGIN}"
data_dir = {data:?}

[[command]]
id = "echo"
program = "/bin/echo"
args = ["{{word}}"]
[[command.param]]
name = "word"
kind = "word"

[[command]]
id = "tail"
program = "/usr/bin/tail"
args = ["-n", "{{lines}}", "--", "{{file}}"]
[[command.param]]
name = "lines"
kind = "integer"
min = 1
max = 500
[[command.param]]
name = "file"
kind = "path"
root = {root:?}
"#,
        data = dir.join("data").display().to_string(),
        root = root.display().to_string(),
    );
    let config = Config::from_toml(&text, Mode::Dev).unwrap();
    let auth = Auth::open(&config.server).unwrap();
    let state = AppState::new(config, auth);
    (router(state.clone()), state)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> (StatusCode, Option<String>, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    let req = match body {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string())),
        None => b.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, set_cookie, json)
}

async fn enroll_and_login(
    app: &Router,
    state: &AppState,
    authr: &mut WebauthnAuthenticator<SoftPasskey>,
) -> String {
    let origin = Url::parse(ORIGIN).unwrap();
    let token = state.auth().open_enrollment(Duration::from_secs(60));

    let (s, _, v) = call(
        app,
        "POST",
        "/api/register/start",
        Some(json!({ "token": token })),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let options: CreationChallengeResponse = serde_json::from_value(v["options"].clone()).unwrap();
    let credential = authr.do_registration(origin.clone(), options).unwrap();
    let (s, _, v) = call(
        app,
        "POST",
        "/api/register/finish",
        Some(json!({ "ceremony": v["ceremony"], "credential": credential })),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let (s, _, v) = call(app, "POST", "/api/login/start", Some(json!({})), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let options: RequestChallengeResponse = serde_json::from_value(v["options"].clone()).unwrap();
    let credential = authr.do_authentication(origin, options).unwrap();
    let (s, cookie, v) = call(
        app,
        "POST",
        "/api/login/finish",
        Some(json!({ "ceremony": v["ceremony"], "credential": credential })),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    cookie.expect("session cookie")
}

#[tokio::test]
async fn full_passkey_flow_and_injection_resistance() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, state) = build(tmp.path());
    let mut authr = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let cookie = enroll_and_login(&app, &state, &mut authr).await;
    let c = Some(cookie.as_str());

    // Enrollment token is single-use.
    assert!(!state.auth().enrollment_open());

    let (s, _, v) = call(&app, "GET", "/api/commands", None, c).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v.as_array().unwrap().len(), 2);

    let (s, _, v) = call(
        &app,
        "POST",
        "/api/exec",
        Some(json!({ "id": "echo", "args": { "word": "hello" } })),
        c,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["stdout"], "hello\n");
    assert_eq!(v["exit_code"], 0);

    let (s, _, v) = call(
        &app,
        "POST",
        "/api/exec",
        Some(json!({ "id": "tail", "args": { "lines": "1", "file": "logs/app.log" } })),
        c,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["stdout"], "line2\n");

    // Injection / traversal / option-smuggling attempts are rejected before exec.
    for (id, args) in [
        ("echo", json!({ "word": "x; touch /tmp/pwned" })),
        ("echo", json!({ "word": "$(id)" })),
        ("echo", json!({ "word": "--help" })),
        ("echo", json!({ "word": "hi", "extra": "1" })),
        ("tail", json!({ "lines": "1", "file": "../../etc/passwd" })),
        ("tail", json!({ "lines": "1", "file": "/etc/passwd" })),
        ("tail", json!({ "lines": "-f", "file": "logs/app.log" })),
        ("tail", json!({ "lines": "100000", "file": "logs/app.log" })),
    ] {
        let (s, _, v) = call(
            &app,
            "POST",
            "/api/exec",
            Some(json!({ "id": id, "args": args })),
            c,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{id} {args} -> {v}");
    }
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/exec",
        Some(json!({ "id": "rm", "args": {} })),
        c,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Audit log records both runs and rejections, and is private.
    let audit = tmp.path().join("data/audit.jsonl");
    let log = std::fs::read_to_string(&audit).unwrap();
    assert!(log.lines().count() >= 10, "{log}");
    assert_eq!(
        std::fs::metadata(&audit).unwrap().permissions().mode() & 0o777,
        0o600
    );

    // Logout kills the session server-side.
    let (s, _, _) = call(&app, "POST", "/api/logout", Some(json!({})), c).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = call(&app, "GET", "/api/commands", None, c).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn replayed_login_assertion_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, state) = build(tmp.path());
    let mut authr = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let _ = enroll_and_login(&app, &state, &mut authr).await;

    let origin = Url::parse(ORIGIN).unwrap();
    let (_, _, v) = call(&app, "POST", "/api/login/start", Some(json!({})), None).await;
    let options: RequestChallengeResponse = serde_json::from_value(v["options"].clone()).unwrap();
    let credential = authr.do_authentication(origin, options).unwrap();
    let body = json!({ "ceremony": v["ceremony"], "credential": credential });
    let (s, _, _) = call(&app, "POST", "/api/login/finish", Some(body.clone()), None).await;
    assert_eq!(s, StatusCode::OK);
    // Same ceremony + assertion again: the challenge was consumed.
    let (s, cookie, _) = call(&app, "POST", "/api/login/finish", Some(body), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(cookie.is_none());
}
