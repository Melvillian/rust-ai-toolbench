# openai-summarizer

## Overview

OpenAI integration package for text summarization. `summarize_text(text, &SummarizeOptions)` posts to the chat completions API with `reqwest`; options are validated (empty text, temperature `0..=2`) before any network call. API key from `SummarizeOptions::api_key` or `OPENAI_API_KEY` (`.env` loaded via `dotenvy`); `OPENAI_BASE_URL` overrides the origin.

## Defaults

- Model `gpt-4o-mini`, temperature `0.7`, no `max_tokens` cap.

## Commands

- `cargo build -p openai-summarizer` - Compile
- `cargo test -p openai-summarizer` - Run tests (wiremock, no network)
- `cargo clippy -p openai-summarizer --all-targets -- -D warnings` - Lint

## Dependencies

- **reqwest** (rustls) - HTTP client
- **serde / serde_json** - (de)serialisation
- **dotenvy** - Load `OPENAI_API_KEY` from `.env`
- **common-lib** (workspace) - `get_env`

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
