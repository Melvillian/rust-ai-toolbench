//! `cli summarize [TEXT] [--model M] [--temperature T] [--max-tokens N]`
//!
//! Sends the text to OpenAI via `openai_summarizer` and prints the summary.
//! When `TEXT` is omitted the whole of stdin is read instead, so it composes
//! with pipes: `cat notes.md | cli summarize`.

use std::io::Read;

use anyhow::Context;
use clap::Args;
use openai_summarizer::{SummarizeOptions, summarize_text};

#[derive(Debug, Args)]
pub struct SummarizeArgs {
    /// Text to summarize. Reads stdin when omitted.
    pub text: Option<String>,

    /// OpenAI model to use
    #[arg(long, default_value_t = SummarizeOptions::default().model)]
    pub model: String,

    /// Sampling temperature (0.0 - 2.0)
    #[arg(long, default_value_t = SummarizeOptions::default().temperature)]
    pub temperature: f32,

    /// Maximum tokens in the generated summary
    #[arg(long)]
    pub max_tokens: Option<u32>,
}

impl SummarizeArgs {
    fn options(&self) -> SummarizeOptions {
        SummarizeOptions {
            api_key: None, // resolved from OPENAI_API_KEY by the library
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

/// Resolve the input text: the positional argument, else all of stdin.
fn read_input(text: Option<String>) -> anyhow::Result<String> {
    if let Some(t) = text {
        return Ok(t);
    }
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read text from stdin")?;
    Ok(buf)
}

pub async fn run(args: SummarizeArgs) -> anyhow::Result<()> {
    let options = args.options();
    let text = read_input(args.text)?;
    let summary = summarize_text(&text, &options)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SummarizeArgs, read_input};
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: SummarizeArgs,
    }

    #[test]
    fn positional_text_is_used_verbatim() {
        assert_eq!(read_input(Some("hi".into())).unwrap(), "hi");
    }

    #[test]
    fn defaults_match_the_library() {
        let h = Harness::parse_from(["x", "some text"]);
        let opts = h.args.options();
        let lib = openai_summarizer::SummarizeOptions::default();
        assert_eq!(opts.model, lib.model);
        assert!((opts.temperature - lib.temperature).abs() < f32::EPSILON);
        assert_eq!(opts.max_tokens, None);
        assert_eq!(opts.api_key, None);
    }

    #[test]
    fn flags_override_defaults() {
        let h = Harness::parse_from([
            "x",
            "--model",
            "gpt-4o",
            "--temperature",
            "0.2",
            "--max-tokens",
            "50",
            "text",
        ]);
        let opts = h.args.options();
        assert_eq!(opts.model, "gpt-4o");
        assert!((opts.temperature - 0.2).abs() < f32::EPSILON);
        assert_eq!(opts.max_tokens, Some(50));
        assert_eq!(h.args.text.as_deref(), Some("text"));
    }
}
