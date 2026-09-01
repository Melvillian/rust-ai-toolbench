# A Toolbench of Rust Apps & Packages To Start Off Your Web Services

This is a template for a Cargo-workspace monorepo that uses best practices for building Rust web services, with a [Dioxus](https://dioxuslabs.com/) web frontend. It is the Rust counterpart of [typescript-ai-toolbench](https://github.com/Melvillian/typescript-ai-toolbench) and keeps the same shape: thin `apps/*`, logic in `packages/*`, a `generators/` scaffolder, one-command dev and prod flows, and Render deploy conventions.

## Setup

> **Using Claude Code?** Just run `/setup` — it checks all prerequisites, builds everything, and verifies your environment. No manual steps needed.

### Manual setup

Requires **Rust stable** (installed via [rustup](https://rustup.rs); `rust-toolchain.toml` adds the `wasm32-unknown-unknown` target automatically) and the **Dioxus CLI**:

```bash
cargo install dioxus-cli --locked   # or: cargo binstall dioxus-cli
cargo build-all
```

`dx` downloads wasm-bindgen, wasm-opt, and the Tailwind CSS binary itself — no Node/npm needed. See the [Environment variables](#environment-variables) section below for optional configuration.

## Features

- Cargo workspace with shared dependency versions and shared lints (clippy pedantic, `-D warnings`)
- Rust 2024 edition
- Dioxus 0.7 web SPA (`apps/web`) with `dioxus-router` and Tailwind CSS v4 (run by `dx`), deployed as a Render static site
- Single-command dev (`cargo dev`) and prod-build (`cargo start`) full-stack flows
- axum for the API server, tested without sockets via `tower::ServiceExt::oneshot`
- clap CLI
- Thin `reqwest` clients for the Anthropic Messages API and OpenAI chat completions, tested with `wiremock`
- rustfmt for formatting, clippy for linting, VSCode format-on-save
- `cargo test --workspace` runs every test (no network, no browser: Dioxus components are tested via `dioxus_ssr`)
- GitHub Actions CI (build, typecheck, test, lint)
- `cargo xtask` for multi-process tasks; cargo aliases for everything else

## Commands

| Command                    | Description                                                                 |
| -------------------------- | --------------------------------------------------------------------------- |
| `cargo build-all`          | Build all packages and apps (+ `dx bundle` for the web app)                 |
| `cargo dev`                | Start api (:8080) and the Dioxus dev server (:5173) together                |
| `cargo dev-api`            | Run the API server (auto-reload with `cargo-watch` if installed)            |
| `cargo dev-cli -- <args>`  | Run the CLI app                                                             |
| `cargo dev-web`            | Run the Dioxus dev server                                                   |
| `cargo docker-build-api`   | Build the API Docker image                                                  |
| `cargo docker-start-api`   | Run the API Docker image on port 8080                                       |
| `cargo start`              | Build everything, then run the full stack: api (:8080) + web preview (:4173) |
| `cargo test --workspace`   | Run tests                                                                   |
| `cargo lint`               | Lint and fix (clippy + rustfmt)                                             |
| `cargo lint-check`         | Lint check only                                                             |
| `cargo typecheck`          | Type check all packages                                                     |
| `cargo fmt --all`          | Format code                                                                 |
| `cargo clean`              | Remove the target directory                                                 |

`cargo start` is the go-to full-stack command: it builds everything, then runs
the api (:8080) and the web app via `cargo preview` (:4173). The preview server
serves the production bundle from `apps/web/dist/public` and proxies `/api` to
the api — the same shape as production, where the Render static site's `/api/*`
rewrite proxies to the api service.

To run the full stack with the api in Docker instead (deploy parity), use two
terminals: `cargo docker-build-api && cargo docker-start-api` in one, then
`cargo preview` in the other — the preview proxy targets :8080, which the
Docker container publishes.

## Layout

```
apps/api               axum JSON API           packages/common-lib        formatters, math, env utils
apps/cli               clap CLI                packages/claude-api        Anthropic Messages API client
apps/web               Dioxus web SPA          packages/openai-summarizer OpenAI summarizer
generators/            render-deploy scaffold  xtask/                     dev orchestration
```

## Environment variables

- `PORT` — API listen port; defaults to 8080 locally (no `.env` needed), and the Docker image sets 80 for production. Optionally override via `apps/api/.env` (see `apps/api/.env.example`)
- `OPENAI_API_KEY` — Required for OpenAI-powered features (used by `openai-summarizer` / `cli summarize`)
- `ANTHROPIC_API_KEY` — Required for `claude-api` calls
- `RENDER_API_KEY` — Optional, only needed for Render deployments
