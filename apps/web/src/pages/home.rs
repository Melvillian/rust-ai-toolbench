use dioxus::prelude::*;
use serde::Deserialize;

/// Shape of `GET /api/hello` (see `apps/api/src/app.rs`).
// Only the browser build fetches; native builds (SSR tests) never construct it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelloResponse {
    pub message: String,
}

/// Fetch by relative path so the dev proxy (`Dioxus.toml`) and the static
/// site's prod `/api/*` rewrite both work.
pub const HELLO_URL: &str = "/api/hello";

#[cfg(target_arch = "wasm32")]
async fn fetch_hello() -> Result<String, String> {
    let response = gloo_net::http::Request::get(HELLO_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body: HelloResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(body.message)
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_hello() -> Result<String, String> {
    Err("fetch is only available in the browser".to_owned())
}

#[component]
pub fn Home() -> Element {
    let api = use_resource(fetch_hello);

    rsx! {
        section {
            h1 { class: "mb-4 text-2xl font-bold", "Home" }
            p { class: "mb-2",
                "This starter page calls "
                code { "GET {HELLO_URL}" }
                " on the axum server to prove the web → api wiring:"
            }
            match &*api.read_unchecked() {
                None => rsx! { p { class: "text-gray-500", "Loading…" } },
                Some(Ok(message)) => rsx! {
                    p { class: "rounded border border-green-200 bg-green-50 p-3",
                        "API says: "
                        strong { "{message}" }
                    }
                },
                Some(Err(detail)) => rsx! {
                    p { class: "rounded border border-red-200 bg-red-50 p-3",
                        "API call failed ({detail}). In dev, make sure the api server is "
                        "running: "
                        code { "cargo dev" }
                        " starts both processes."
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;

    use super::{HelloResponse, Home};

    #[test]
    fn hello_response_matches_the_api_shape() {
        let parsed: HelloResponse =
            serde_json::from_str(r#"{"message":"Hello from the api endpoint!"}"#).unwrap();
        assert_eq!(parsed.message, "Hello from the api endpoint!");
    }

    #[test]
    fn renders_heading_and_loading_state() {
        let html = dioxus_ssr::render_element(rsx! { Home {} });
        assert!(html.contains(">Home<"), "{html}");
        assert!(html.contains("/api/hello"), "{html}");
        assert!(html.contains("Loading"), "{html}");
    }
}
