//! Thin client for the Anthropic Messages API (`POST /v1/messages`).
//!
//! There is no official Anthropic SDK for Rust, so this crate talks to the
//! HTTP API directly with `reqwest` + `serde`. Callers supply their own system
//! prompt, messages and (optionally) tools; the API key is resolved from
//! [`CallClaudeOptions::api_key`] or the `ANTHROPIC_API_KEY` environment
//! variable (loaded from `.env` via `dotenvy`).
//!
//! ```no_run
//! use claude_api::{CallClaudeOptions, MessageParam, call_claude};
//!
//! # async fn run() -> Result<(), claude_api::ClaudeApiError> {
//! let options = CallClaudeOptions::new(
//!     "You are a terse assistant.",
//!     vec![MessageParam::user("Say hi.")],
//! );
//! let message = call_claude(&options).await?;
//! println!("{}", message.text());
//! # Ok(())
//! # }
//! ```

mod client;
mod errors;
mod types;

pub use client::{ClaudeClient, call_claude};
pub use errors::ClaudeApiError;
pub use types::{
    ANTHROPIC_VERSION, CallClaudeOptions, ContentBlock, DEFAULT_BASE_URL, DEFAULT_MAX_TOKENS,
    DEFAULT_MODEL, Fallbacks, Message, MessageParam, Role, StopReason, Tool, Usage,
};
