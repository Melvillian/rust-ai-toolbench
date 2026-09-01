/// Errors returned by [`crate::call_claude`] and [`crate::ClaudeClient`].
#[derive(Debug, thiserror::Error)]
pub enum ClaudeApiError {
    /// No API key was supplied and `ANTHROPIC_API_KEY` is unset or empty.
    #[error("ANTHROPIC_API_KEY missing — set it in .env or pass api_key in options")]
    MissingApiKey,

    /// The HTTP request could not be sent or the body could not be read.
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status.
    #[error("Anthropic API returned HTTP {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Raw response body (usually a JSON error object).
        body: String,
    },

    /// The API answered 2xx but the body was not a valid `Message`.
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),
}
