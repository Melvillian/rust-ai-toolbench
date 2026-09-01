# claude-api

## Overview

Thin, generic client for the Anthropic Messages API (`POST /v1/messages`). There is no official Rust SDK, so this crate talks to the HTTP API directly with `reqwest` + `serde`. Callers pass their own system prompt, messages, and tools. API key resolved from `CallClaudeOptions::api_key` or the `ANTHROPIC_API_KEY` env var (`.env` loaded via `dotenvy`).

## Defaults

- Model: `claude-opus-5` (`DEFAULT_MODEL`)
- `max_tokens`: 16000 (`DEFAULT_MAX_TOKENS`)
- Server-side fallbacks: **on** — `CallClaudeOptions::new` sets `fallbacks: Some(Fallbacks::Default)`, which sends `"fallbacks": "default"` plus the `anthropic-beta: server-side-fallback-2026-07-01` header so refusals are re-routed server-side. Call `.without_fallbacks()` to opt out.
- `ANTHROPIC_BASE_URL` overrides the origin (useful for proxies/tests).

## Commands

- `cargo build -p claude-api` - Compile
- `cargo test -p claude-api` - Run tests (wiremock, no network)
- `cargo clippy -p claude-api --all-targets -- -D warnings` - Lint

## Key Files

- `src/client.rs` - `ClaudeClient`, `call_claude`, request body, key resolution
- `src/types.rs` - request/response types (`CallClaudeOptions`, `MessageParam`, `Tool`, `Message`, `ContentBlock`, …)
- `src/errors.rs` - `ClaudeApiError`

## Dependencies

- **reqwest** (rustls) - HTTP client
- **serde / serde_json** - (de)serialisation
- **dotenvy** - Load `ANTHROPIC_API_KEY` from `.env`
- **common-lib** (workspace) - `get_env`

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
