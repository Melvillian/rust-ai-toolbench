//! Text summarisation over the OpenAI chat completions API.
//!
//! ```no_run
//! use openai_summarizer::{SummarizeOptions, summarize_text};
//!
//! # async fn run() -> Result<(), openai_summarizer::SummarizeError> {
//! let summary = summarize_text("long text …", &SummarizeOptions::default()).await?;
//! println!("{summary}");
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

/// Base URL used unless overridden (`OPENAI_BASE_URL` or
/// [`OpenAiSummarizer::with_base_url`]).
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com";
/// Model used by [`SummarizeOptions::default`].
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
/// Temperature used by [`SummarizeOptions::default`].
pub const DEFAULT_TEMPERATURE: f32 = 0.7;

const SYSTEM_PROMPT: &str =
    "You are a helpful assistant that creates concise, accurate summaries of text.";

/// Configuration for one summarisation call.
#[derive(Debug, Clone, PartialEq)]
pub struct SummarizeOptions {
    /// OpenAI API key; falls back to `OPENAI_API_KEY` when `None`.
    pub api_key: Option<String>,
    /// Model id (default [`DEFAULT_MODEL`]).
    pub model: String,
    /// Maximum tokens in the response; omitted from the request when `None`.
    pub max_tokens: Option<u32>,
    /// Sampling temperature, `0.0..=2.0` (default [`DEFAULT_TEMPERATURE`]).
    pub temperature: f32,
}

impl Default for SummarizeOptions {
    fn default() -> Self {
        Self {
            api_key: None,
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: None,
            temperature: DEFAULT_TEMPERATURE,
        }
    }
}

/// Errors returned by [`summarize_text`] and [`OpenAiSummarizer::summarize`].
#[derive(Debug, thiserror::Error)]
pub enum SummarizeError {
    /// The input was empty or whitespace-only.
    #[error("Text to summarize cannot be empty")]
    EmptyText,
    /// No API key was supplied and `OPENAI_API_KEY` is unset or empty.
    #[error("API key must be provided via options.api_key or OPENAI_API_KEY environment variable")]
    MissingApiKey,
    /// `temperature` was outside `0.0..=2.0`.
    #[error("temperature must be between 0 and 2, got {0}")]
    InvalidTemperature(f32),
    /// The HTTP request could not be sent or the body could not be read.
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The API answered with a non-2xx status.
    #[error("OpenAI API returned HTTP {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Raw response body.
        body: String,
    },
    /// The API answered 2xx but returned no summary text.
    #[error("Failed to generate summary from OpenAI")]
    EmptySummary,
    /// The API answered 2xx but the body was not a chat completion.
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

/// A configured OpenAI client for summarisation.
#[derive(Debug, Clone)]
pub struct OpenAiSummarizer {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAiSummarizer {
    /// A client targeting [`DEFAULT_BASE_URL`].
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
        }
    }

    /// Point the client at a different origin (proxies, mocks in tests).
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        let url: String = base_url.into();
        url.trim_end_matches('/').clone_into(&mut self.base_url);
        self
    }

    /// Summarise `text`. `options.api_key` is ignored here — the client's key
    /// is used.
    pub async fn summarize(
        &self,
        text: &str,
        options: &SummarizeOptions,
    ) -> Result<String, SummarizeError> {
        validate(text, options)?;
        let user_prompt = format!("Please summarize the following text:\n\n{text}");
        let body = ChatRequest {
            model: &options.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT,
                },
                ChatMessage {
                    role: "user",
                    content: &user_prompt,
                },
            ],
            max_tokens: options.max_tokens,
            temperature: options.temperature,
        };

        let response = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let raw = response.text().await?;
        if !status.is_success() {
            return Err(SummarizeError::Api {
                status: status.as_u16(),
                body: raw,
            });
        }

        let parsed: ChatResponse = serde_json::from_str(&raw)?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|summary| !summary.is_empty())
            .ok_or(SummarizeError::EmptySummary)
    }
}

fn validate(text: &str, options: &SummarizeOptions) -> Result<(), SummarizeError> {
    if text.trim().is_empty() {
        return Err(SummarizeError::EmptyText);
    }
    if !(0.0..=2.0).contains(&options.temperature) {
        return Err(SummarizeError::InvalidTemperature(options.temperature));
    }
    Ok(())
}

/// Pick the API key: an explicit value wins over the environment; empty
/// strings count as absent.
fn resolve_api_key(
    explicit: Option<&str>,
    from_env: Option<String>,
) -> Result<String, SummarizeError> {
    explicit
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .or(from_env)
        .ok_or(SummarizeError::MissingApiKey)
}

/// Summarise `text` using OpenAI.
///
/// Validates the input first (empty text, temperature range), then resolves
/// the key from `options.api_key` or `OPENAI_API_KEY` (loading `.env` first)
/// and honours `OPENAI_BASE_URL` if set.
pub async fn summarize_text(
    text: &str,
    options: &SummarizeOptions,
) -> Result<String, SummarizeError> {
    validate(text, options)?;
    dotenvy::dotenv().ok();
    let api_key = resolve_api_key(
        options.api_key.as_deref(),
        common_lib::get_env("OPENAI_API_KEY"),
    )?;
    let mut client = OpenAiSummarizer::new(api_key);
    if let Some(base_url) = common_lib::get_env("OPENAI_BASE_URL") {
        client = client.with_base_url(base_url);
    }
    client.summarize(text, options).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn completion(content: &serde_json::Value) -> serde_json::Value {
        json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] })
    }

    #[test]
    fn defaults_match_the_typescript_package() {
        let opts = SummarizeOptions::default();
        assert_eq!(opts.model, "gpt-4o-mini");
        assert!((opts.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(opts.max_tokens, None);
        assert_eq!(opts.api_key, None);
    }

    #[test]
    fn resolve_api_key_prefers_explicit_then_env_then_errors() {
        assert_eq!(resolve_api_key(Some("k"), Some("e".into())).unwrap(), "k");
        assert_eq!(resolve_api_key(None, Some("e".into())).unwrap(), "e");
        assert!(matches!(
            resolve_api_key(None, None),
            Err(SummarizeError::MissingApiKey)
        ));
    }

    #[tokio::test]
    async fn empty_text_fails_before_any_request() {
        let server = MockServer::start().await;
        let client = OpenAiSummarizer::new("k").with_base_url(server.uri());
        for text in ["", "   \n\t"] {
            let err = client
                .summarize(text, &SummarizeOptions::default())
                .await
                .unwrap_err();
            assert!(matches!(err, SummarizeError::EmptyText), "{err:?}");
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_temperature_is_rejected() {
        let opts = SummarizeOptions {
            temperature: 2.5,
            ..SummarizeOptions::default()
        };
        let err = summarize_text("some text", &opts).await.unwrap_err();
        assert!(matches!(err, SummarizeError::InvalidTemperature(t) if t > 2.0));
    }

    #[tokio::test]
    async fn happy_path_sends_expected_request_and_returns_summary() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer secret"))
            .and(body_json(json!({
                "model": "gpt-4o-mini",
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": "Please summarize the following text:\n\nhello world"}
                ],
                "temperature": 0.7
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion(&json!("short"))))
            .expect(1)
            .mount(&server)
            .await;

        let summary = OpenAiSummarizer::new("secret")
            .with_base_url(server.uri())
            .summarize("hello world", &SummarizeOptions::default())
            .await
            .unwrap();
        assert_eq!(summary, "short");
    }

    #[tokio::test]
    async fn max_tokens_and_model_are_forwarded() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_json(json!({
                "model": "gpt-4o",
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": "Please summarize the following text:\n\nx"}
                ],
                "max_tokens": 42,
                "temperature": 0.2
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion(&json!("s"))))
            .expect(1)
            .mount(&server)
            .await;

        let opts = SummarizeOptions {
            api_key: None,
            model: "gpt-4o".into(),
            max_tokens: Some(42),
            temperature: 0.2,
        };
        OpenAiSummarizer::new("k")
            .with_base_url(server.uri())
            .summarize("x", &opts)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn server_error_is_an_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let err = OpenAiSummarizer::new("k")
            .with_base_url(server.uri())
            .summarize("x", &SummarizeOptions::default())
            .await
            .unwrap_err();
        assert!(
            matches!(err, SummarizeError::Api { status: 500, .. }),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn null_or_missing_content_is_an_empty_summary() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion(&json!(null))))
            .mount(&server)
            .await;
        let client = OpenAiSummarizer::new("k").with_base_url(server.uri());
        let err = client
            .summarize("x", &SummarizeOptions::default())
            .await
            .unwrap_err();
        assert!(matches!(err, SummarizeError::EmptySummary), "{err:?}");

        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices": []})))
            .mount(&server)
            .await;
        let err = client
            .summarize("x", &SummarizeOptions::default())
            .await
            .unwrap_err();
        assert!(matches!(err, SummarizeError::EmptySummary), "{err:?}");
    }
}
