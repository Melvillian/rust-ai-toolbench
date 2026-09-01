//! Integration test: preview server against a temp dist dir + a fake api.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::Request;
use axum::routing::get;

async fn spawn(router: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

#[tokio::test]
async fn serves_static_files_spa_fallback_and_proxies_api() {
    let dist = tempfile::tempdir().unwrap();
    std::fs::write(dist.path().join("index.html"), "<html>index</html>").unwrap();
    std::fs::create_dir_all(dist.path().join("assets")).unwrap();
    std::fs::write(dist.path().join("assets/a.txt"), "asset").unwrap();

    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen_in_handler = seen.clone();
    let fake_api = Router::new().route(
        "/api/hello",
        get(move |req: Request| {
            let seen = seen_in_handler.clone();
            async move {
                seen.lock().unwrap().push(req.uri().to_string());
                axum::Json(serde_json::json!({ "message": "hi" }))
            }
        }),
    );
    let api_addr = spawn(fake_api).await;
    let preview_addr = spawn(xtask::preview::router(
        dist.path(),
        &format!("http://{api_addr}"),
    ))
    .await;

    let client = reqwest::Client::new();
    let base = format!("http://{preview_addr}");

    let index = client.get(format!("{base}/")).send().await.unwrap();
    assert_eq!(index.text().await.unwrap(), "<html>index</html>");

    let about = client.get(format!("{base}/about")).send().await.unwrap();
    assert_eq!(about.status(), 200);
    assert_eq!(about.text().await.unwrap(), "<html>index</html>");

    let asset = client
        .get(format!("{base}/assets/a.txt"))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.text().await.unwrap(), "asset");

    let hello = client
        .get(format!("{base}/api/hello?x=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(hello.status(), 200);
    assert_eq!(
        hello.headers()[reqwest::header::CONTENT_TYPE],
        "application/json"
    );
    assert_eq!(hello.text().await.unwrap(), r#"{"message":"hi"}"#);
    assert_eq!(seen.lock().unwrap().as_slice(), ["/api/hello?x=1"]);

    let missing = client.get(format!("{base}/api/nope")).send().await.unwrap();
    assert_eq!(missing.status(), 404);
}

#[tokio::test]
async fn unreachable_api_is_a_502_not_a_crash() {
    let dist = tempfile::tempdir().unwrap();
    std::fs::write(dist.path().join("index.html"), "x").unwrap();
    let preview_addr = spawn(xtask::preview::router(dist.path(), "http://127.0.0.1:9")).await;
    let res = reqwest::get(format!("http://{preview_addr}/health"))
        .await
        .unwrap();
    assert_eq!(res.status(), 502);
    assert!(res.text().await.unwrap().contains("is the api running?"));
}
