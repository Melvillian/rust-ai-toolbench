//! HTTP client for `POST /v1/messages`.

use serde::Serialize;

use crate::errors::ClaudeApiError;
use crate::types::{
    ANTHROPIC_VERSION, CallClaudeOptions, DEFAULT_BASE_URL, DEFAULT_MAX_TOKENS, DEFAULT_MODEL,
    FALLBACKS_BETA_HEADER, Fallbacks, Message, MessageParam, Tool,
};

/// Wire representation of a Messages API request body.
#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: &'a [MessageParam],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Tool]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallbacks: Option<Fallbacks>,
}

impl<'a> MessagesRequest<'a> {
    fn from_options(options: &'a CallClaudeOptions) -> Self {
        Self {
            model: options.model.as_deref().unwrap_or(DEFAULT_MODEL),
            max_tokens: options.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system: &options.system,
            messages: &options.messages,
            tools: options.tools.as_deref(),
            fallbacks: options.fallbacks,
        }
    }
}

/// A configured client for the Anthropic Messages API.
///
/// Holds the API key and base URL; reuse one client across calls so the
/// underlying connection pool is shared.
#[derive(Debug, Clone)]
pub struct ClaudeClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl ClaudeClient {
    /// A client targeting [`DEFAULT_BASE_URL`].
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
        }
    }

    /// Point the client at a different origin (proxies, mocks in tests).
    /// A trailing `/` is stripped.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        let url: String = base_url.into();
        url.trim_end_matches('/').clone_into(&mut self.base_url);
        self
    }

    /// Use a pre-configured `reqwest::Client` (timeouts, proxies, …).
    #[must_use]
    pub fn with_http_client(mut self, http: reqwest::Client) -> Self {
        self.http = http;
        self
    }

    /// Base URL requests are sent to.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Send one request to `POST /v1/messages`.
    ///
    /// `options.api_key` is ignored here — the client's key is used.
    pub async fn create_message(
        &self,
        options: &CallClaudeOptions,
    ) -> Result<Message, ClaudeApiError> {
        let body = MessagesRequest::from_options(options);

        let mut request = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body);
        if options.fallbacks.is_some() {
            request = request.header("anthropic-beta", FALLBACKS_BETA_HEADER);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(ClaudeApiError::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        Ok(serde_json::from_str(&text)?)
    }
}

/// Pick the API key: an explicit value wins over the environment; empty
/// strings count as absent.
pub(crate) fn resolve_api_key(
    explicit: Option<&str>,
    from_env: Option<String>,
) -> Result<String, ClaudeApiError> {
    explicit
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .or(from_env)
        .ok_or(ClaudeApiError::MissingApiKey)
}

/// One-shot convenience: resolve the key from `options.api_key` or
/// `ANTHROPIC_API_KEY` (loading `.env` first), honour `ANTHROPIC_BASE_URL` if
/// set, and call the API.
pub async fn call_claude(options: &CallClaudeOptions) -> Result<Message, ClaudeApiError> {
    dotenvy::dotenv().ok();
    let api_key = resolve_api_key(
        options.api_key.as_deref(),
        common_lib::get_env("ANTHROPIC_API_KEY"),
    )?;
    let mut client = ClaudeClient::new(api_key);
    if let Some(base_url) = common_lib::get_env("ANTHROPIC_BASE_URL") {
        client = client.with_base_url(base_url);
    }
    client.create_message(options).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn ok_message() -> serde_json::Value {
        json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "m",
            "content": [{"type": "text", "text": "hi"}],
            "stop_reason": "end_turn", "stop_sequence": null,
            "usage": {"input_tokens": 1, "output_tokens": 2}
        })
    }

    fn options() -> CallClaudeOptions {
        CallClaudeOptions::new("sys", vec![MessageParam::user("hi")])
    }

    #[test]
    fn resolve_api_key_prefers_explicit_then_env_then_errors() {
        assert_eq!(
            resolve_api_key(Some("explicit"), Some("env".into())).unwrap(),
            "explicit"
        );
        assert_eq!(resolve_api_key(None, Some("env".into())).unwrap(), "env");
        assert_eq!(
            resolve_api_key(Some("  "), Some("env".into())).unwrap(),
            "env"
        );
        assert!(matches!(
            resolve_api_key(None, None),
            Err(ClaudeApiError::MissingApiKey)
        ));
    }

    #[tokio::test]
    async fn sends_defaults_headers_and_fallbacks() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .and(header("anthropic-beta", FALLBACKS_BETA_HEADER))
            .and(header("content-type", "application/json"))
            .and(body_partial_json(json!({
                "model": DEFAULT_MODEL,
                "max_tokens": DEFAULT_MAX_TOKENS,
                "system": "sys",
                "messages": [{"role": "user", "content": "hi"}],
                "fallbacks": "default",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_message()))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClaudeClient::new("test-key").with_base_url(format!("{}/", server.uri()));
        let message = client.create_message(&options()).await.unwrap();
        assert_eq!(message.text(), "hi");
        assert_eq!(message.stop_reason, Some(crate::StopReason::EndTurn));
    }

    #[tokio::test]
    async fn forwards_overrides_untouched() {
        let server = MockServer::start().await;
        let tools = vec![Tool {
            name: "do".into(),
            description: Some("do".into()),
            input_schema: json!({"type": "object"}),
        }];
        Mock::given(method("POST"))
            .and(body_partial_json(json!({
                "model": "custom-model",
                "max_tokens": 12_345,
                "system": "sys",
                "messages": [{"role": "user", "content": "hi"}],
                "tools": [{"name": "do", "description": "do", "input_schema": {"type": "object"}}],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_message()))
            .expect(1)
            .mount(&server)
            .await;

        let opts = options()
            .model("custom-model")
            .max_tokens(12_345)
            .tools(tools);
        ClaudeClient::new("k")
            .with_base_url(server.uri())
            .create_message(&opts)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn without_fallbacks_omits_header_and_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_message()))
            .expect(1)
            .mount(&server)
            .await;

        ClaudeClient::new("k")
            .with_base_url(server.uri())
            .create_message(&options().without_fallbacks())
            .await
            .unwrap();

        let received: Vec<Request> = server.received_requests().await.unwrap();
        let req = &received[0];
        assert!(req.headers.get("anthropic-beta").is_none());
        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
        assert!(body.get("fallbacks").is_none());
        assert!(body.get("tools").is_none());
    }

    #[tokio::test]
    async fn non_success_status_is_an_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header_exists("x-api-key"))
            .respond_with(ResponseTemplate::new(429).set_body_string(r#"{"error":"rate"}"#))
            .mount(&server)
            .await;

        let err = ClaudeClient::new("k")
            .with_base_url(server.uri())
            .create_message(&options())
            .await
            .unwrap_err();
        match err {
            ClaudeApiError::Api { status, body } => {
                assert_eq!(status, 429);
                assert!(body.contains("rate"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn malformed_success_body_is_a_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = ClaudeClient::new("k")
            .with_base_url(server.uri())
            .create_message(&options())
            .await
            .unwrap_err();
        assert!(matches!(err, ClaudeApiError::Decode(_)), "{err:?}");
    }
}
