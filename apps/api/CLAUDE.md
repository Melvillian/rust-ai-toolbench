# API

## Overview

axum HTTP API server. Routes live in `src/app.rs` (a runtime-agnostic `create_app() -> Router` factory, tested in-process via `tower::ServiceExt::oneshot`); `src/main.rs` is the tokio entrypoint that binds `0.0.0.0:$PORT`. API-only: it does NOT serve `apps/web` — the web app deploys as a Render static site whose `/api/*` rewrite proxies to this service (see the `render-deploys` skill). In dev, the Dioxus dev server (`dx serve`) proxies `/api` and `/health` here. Part of the Cargo workspace.

## Endpoints

- `GET /health` - Health check (`{ status, timestamp }`, RFC 3339 UTC)
- `GET /api/hello` - Example API endpoint (fetched by apps/web's Home page)
- `GET /api/info` - Service info (`version` is `CARGO_PKG_VERSION`)
- Unknown `/api` / `/api/*` paths, and wrong methods on known `/api` routes, return `404` with `{ "error": "not found" }` (`any` routes + `method_not_allowed_fallback`)
- All other paths (including `/`) 404 — this service does not serve the web app
- Request bodies are capped at 10 MiB (`DefaultBodyLimit`)

## Configuration

- `PORT` - Listen port (dev: 8080 via `.env`; Docker sets 80)
- `RUST_LOG` - `tracing` env filter (default `info`)
- `.env` is loaded by `dotenvy` at startup; real environment variables win.

## Commands

- `cargo run -p api` - Run the server
- `cargo test -p api` - Run the route tests in `src/app.rs`
- `cargo xtask dev-api` - Run the server with auto-reload (root workspace alias)
- `docker build -t rust-ai-toolbench-api -f apps/api/Dockerfile .` - Build the production image (from the repo root)

## Dependencies

- **axum** (0.8) - HTTP router/server (`DefaultBodyLimit`, `method_not_allowed_fallback`)
- **tokio** - Async runtime and TCP listener
- **tower-http** - `TraceLayer` request logging
- **serde** / **serde_json** - JSON response bodies
- **chrono** - RFC 3339 timestamp for `/health`
- **tracing** / **tracing-subscriber** - Structured logging
- **dotenvy** - `.env` loading
- **common-lib** (workspace) - `get_env` helper
- dev: **tower** (`oneshot`), **http-body-util** (collecting response bodies)

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
