# CLI

## Overview

Command-line tool built with clap 4 (derive API). Uses the workspace package `openai-summarizer`. `src/main.rs` defines the `Cli` parser; each subcommand is a module in `src/commands/` with its own clap `Args` and a `run` function. Version comes from `CARGO_PKG_VERSION` (clap's `version` attribute).

## Commands

- `cargo run -p cli -- hello` - Print `Hello world!`
- `cargo run -p cli -- summarize "text"` - Summarize text with OpenAI (`--model`, `--temperature`, `--max-tokens`; reads stdin when no text is given; needs `OPENAI_API_KEY`)
- `cargo run -p cli -- --version` - Print the crate version
- `cargo test -p cli` - Unit tests (`src/commands/*`) plus integration tests in `tests/cli.rs` that execute the built binary
- `cargo build --release -p cli` - Produce a single self-contained binary at `target/release/cli` (the analogue of the TS `build:single`)
- `cargo xtask dev-cli` - Run via the root workspace alias

## Dependencies

- **clap** (4, `derive`) - CLI argument parsing
- **tokio** - Async runtime for the OpenAI call
- **anyhow** - Error propagation from `main` (prints `Error: ...` and exits 1)
- **dotenvy** - Optional `.env` loading
- **openai-summarizer** (workspace) - OpenAI integration (`summarize_text`, `SummarizeOptions`)

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
