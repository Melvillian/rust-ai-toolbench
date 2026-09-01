use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Model used when [`CallClaudeOptions::model`] is `None`.
pub const DEFAULT_MODEL: &str = "claude-opus-5";
/// `max_tokens` used when [`CallClaudeOptions::max_tokens`] is `None`.
pub const DEFAULT_MAX_TOKENS: u32 = 16_000;
/// Value of the `anthropic-version` header.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Base URL used unless overridden (`ANTHROPIC_BASE_URL` or
/// [`crate::ClaudeClient::with_base_url`]).
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Beta header that enables server-side fallbacks.
pub(crate) const FALLBACKS_BETA_HEADER: &str = "server-side-fallback-2026-07-01";

/// Who authored a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The caller.
    User,
    /// The model.
    Assistant,
}

/// A single conversation turn sent to the API.
///
/// `content` is kept as raw JSON so callers can pass either a plain string or
/// an array of content blocks (text, images, `tool_result`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageParam {
    /// Author of the turn.
    pub role: Role,
    /// Either a JSON string or an array of content-block objects.
    pub content: Value,
}

impl MessageParam {
    /// A `user` turn with plain-text content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Value::String(text.into()),
        }
    }

    /// An `assistant` turn with plain-text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Value::String(text.into()),
        }
    }
}

/// A tool definition the model may call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name (what the model uses in `tool_use` blocks).
    pub name: String,
    /// Human-readable description shown to the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool's input.
    pub input_schema: Value,
}

/// Server-side fallback policy (the `fallbacks` request field).
///
/// Serialises to the string `"default"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fallbacks {
    /// Let the server re-route refusals and overloads to a fallback model.
    Default,
}

/// Everything needed for one call to the Messages API.
///
/// Build with [`CallClaudeOptions::new`], which turns on
/// [`Fallbacks::Default`]; chain the builder methods to override defaults.
#[derive(Debug, Clone)]
pub struct CallClaudeOptions {
    /// System prompt.
    pub system: String,
    /// Conversation so far (must start with a `user` turn).
    pub messages: Vec<MessageParam>,
    /// Model id; [`DEFAULT_MODEL`] when `None`.
    pub model: Option<String>,
    /// Tools the model may call; omitted from the request when `None`.
    pub tools: Option<Vec<Tool>>,
    /// Output token cap; [`DEFAULT_MAX_TOKENS`] when `None`.
    pub max_tokens: Option<u32>,
    /// Explicit API key; falls back to `ANTHROPIC_API_KEY` when `None`.
    pub api_key: Option<String>,
    /// Server-side fallback policy. `Some` also sends the beta header.
    pub fallbacks: Option<Fallbacks>,
}

impl CallClaudeOptions {
    /// Options with `fallbacks` set to [`Fallbacks::Default`] and every other
    /// optional field unset.
    pub fn new(system: impl Into<String>, messages: Vec<MessageParam>) -> Self {
        Self {
            system: system.into(),
            messages,
            model: None,
            tools: None,
            max_tokens: None,
            api_key: None,
            fallbacks: Some(Fallbacks::Default),
        }
    }

    /// Override the model id.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Attach tool definitions.
    #[must_use]
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Override the output token cap.
    #[must_use]
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Supply the API key explicitly instead of reading the environment.
    #[must_use]
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Opt out of server-side fallbacks (drops the beta header and body field).
    #[must_use]
    pub fn without_fallbacks(mut self) -> Self {
        self.fallbacks = None;
        self
    }
}

/// One block of a response's `content` array.
///
/// Unknown block types (e.g. `thinking`) deserialise to
/// [`ContentBlock::Other`] instead of failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text.
    Text {
        /// The text.
        text: String,
    },
    /// A request to invoke a tool.
    ToolUse {
        /// Unique id to echo back in the `tool_result`.
        id: String,
        /// Tool name.
        name: String,
        /// Tool input matching the tool's `input_schema`.
        input: Value,
    },
    /// Any block type this crate does not model.
    #[serde(other)]
    Other,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of the turn.
    EndTurn,
    /// Hit `max_tokens`.
    MaxTokens,
    /// Hit a custom stop sequence.
    StopSequence,
    /// The model wants to call a tool.
    ToolUse,
    /// Long-running turn paused; resume by sending the response back.
    PauseTurn,
    /// The model refused for safety reasons.
    Refusal,
    /// A stop reason this crate does not model.
    #[serde(other)]
    Other,
}

/// Token accounting for one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in the prompt.
    pub input_tokens: u64,
    /// Tokens generated.
    pub output_tokens: u64,
    /// Tokens written to the prompt cache, if caching was used.
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
    /// Tokens served from the prompt cache, if caching was used.
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
}

/// A successful Messages API response (subset of fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Message id (`msg_…`).
    pub id: String,
    /// Always `"message"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Always [`Role::Assistant`].
    pub role: Role,
    /// Model that produced the response.
    pub model: String,
    /// Response content blocks.
    pub content: Vec<ContentBlock>,
    /// Why generation stopped.
    pub stop_reason: Option<StopReason>,
    /// The custom stop sequence hit, if any.
    pub stop_sequence: Option<String>,
    /// Token accounting.
    pub usage: Usage,
}

impl Message {
    /// Concatenation of every [`ContentBlock::Text`] block, in order.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallbacks_serialises_as_default_string() {
        assert_eq!(
            serde_json::to_string(&Fallbacks::Default).unwrap(),
            "\"default\""
        );
        assert_eq!(
            serde_json::from_str::<Fallbacks>("\"default\"").unwrap(),
            Fallbacks::Default
        );
    }

    #[test]
    fn role_serialises_lowercase() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn new_enables_fallbacks_and_builders_override() {
        let opts = CallClaudeOptions::new("sys", vec![MessageParam::user("hi")]);
        assert_eq!(opts.fallbacks, Some(Fallbacks::Default));
        assert!(opts.model.is_none() && opts.tools.is_none());
        assert!(opts.max_tokens.is_none() && opts.api_key.is_none());

        let opts = opts
            .model("m")
            .max_tokens(7)
            .api_key("k")
            .tools(vec![])
            .without_fallbacks();
        assert_eq!(opts.model.as_deref(), Some("m"));
        assert_eq!(opts.max_tokens, Some(7));
        assert_eq!(opts.api_key.as_deref(), Some("k"));
        assert_eq!(opts.tools.as_deref(), Some(&[][..]));
        assert_eq!(opts.fallbacks, None);
    }

    #[test]
    fn unknown_content_block_and_stop_reason_do_not_error() {
        let json = r#"{
            "id": "msg_1", "type": "message", "role": "assistant", "model": "m",
            "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "abc"},
                {"type": "text", "text": "Hello"},
                {"type": "tool_use", "id": "toolu_1", "name": "do", "input": {"x": 1}},
                {"type": "text", "text": " world"}
            ],
            "stop_reason": "something_new",
            "stop_sequence": null,
            "usage": {"input_tokens": 3, "output_tokens": 4}
        }"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(message.content.len(), 4);
        assert_eq!(message.content[0], ContentBlock::Other);
        assert_eq!(
            message.content[2],
            ContentBlock::ToolUse {
                id: "toolu_1".into(),
                name: "do".into(),
                input: serde_json::json!({"x": 1}),
            }
        );
        assert_eq!(message.stop_reason, Some(StopReason::Other));
        assert_eq!(message.text(), "Hello world");
        assert_eq!(message.usage.cache_read_input_tokens, None);
    }

    #[test]
    fn tool_omits_description_when_none() {
        let tool = Tool {
            name: "t".into(),
            description: None,
            input_schema: serde_json::json!({}),
        };
        assert_eq!(
            serde_json::to_string(&tool).unwrap(),
            r#"{"name":"t","input_schema":{}}"#
        );
    }
}
