# rust-ai-toolbench — design

**Date:** 2026-08-31
**Status:** implemented 2026-09-01 (autonomous session; see "Assumptions")

A Rust port of [typescript-ai-toolbench](https://github.com/Melvillian/typescript-ai-toolbench):
a template monorepo with thin executable apps (api, cli, web) over importable
library packages, plus a code generator for Render deploy config, CI, and
Claude Code skills. The web app uses [Dioxus](https://dioxuslabs.com/) as
requested.

## Goals

- Same shape as the TS repo so someone who knows one can navigate the other:
  `apps/*` are thin wrappers, `packages/*` hold logic, `generators/` scaffolds
  deploy config, root-level "scripts" fan out to every workspace member.
- Every workspace member is covered by the same four correctness gates:
  **build**, **typecheck**, **test**, **lint** — and, unlike `bun --filter`,
  Cargo cannot silently skip a member, so the gates are uniform by
  construction.
- Idiomatic Rust: Cargo workspace, `thiserror` errors, `clippy -D warnings`,
  `rustfmt`, no shell-only glue.

## Non-goals

- Feature parity with every TS dependency (no `wellcrafted`, `date-fns`, `zod`
  analogues). Rust's type system covers what those provided.
- SSR / Dioxus fullstack. The web app is a client-side WASM SPA that calls a
  separate axum API — the "hybrid" static-site + API deploy pattern the TS
  repo uses.

## Workspace layout

```
Cargo.toml                 # [workspace] members, shared deps, shared lints
.cargo/config.toml         # cargo aliases: `cargo xtask`, `cargo dev`, …
rust-toolchain.toml        # stable + wasm32-unknown-unknown + clippy/rustfmt
apps/
  api/                     # axum JSON API (bin)  — port 8080, Docker sets 80
  cli/                     # clap CLI (bin)       — `hello`, `summarize`
  web/                     # Dioxus web SPA (bin) — dx serve :5173, proxies /api
packages/
  common-lib/              # formatters, math, env/string utilities (lib)
  claude-api/              # thin Anthropic Messages API client (lib)
  openai-summarizer/       # summarize_text() over OpenAI chat completions (lib)
generators/                # `render-deploy <app> [--static]` scaffolder (bin)
xtask/                     # dev orchestration: dev, start, preview, docker… (bin)
docs/superpowers/specs/    # this file
.github/workflows/ci.yml   # build / check / test / lint jobs
.claude/                   # setup + render-deploys skills, generate-render-deploy command
scripts/                   # export/import Claude Code setup (verbatim from TS repo)
```

Naming follows the TS repo (`api`, `web`, `cli`, `common-lib`, …). Library
crates are referenced as `common_lib`, `claude_api`, `openai_summarizer`.

## Root "scripts" → cargo aliases + xtask

The TS root `package.json` scripts map to cargo aliases in
`.cargo/config.toml` (pure cargo where possible, `xtask` for multi-process
orchestration):

| TS                          | Rust                                        |
| --------------------------- | ------------------------------------------- |
| `bun run build`             | `cargo build-all` (cargo build --workspace + `dx bundle --release --out-dir dist` for web) |
| `bun run typecheck`         | `cargo check --workspace --all-targets`     |
| `bun run test`              | `cargo test --workspace`                    |
| `bun run lint:check`        | `cargo lint-check` = clippy `-D warnings` + `fmt --check` |
| `bun run lint`              | `cargo lint` = `clippy --fix` + `fmt`        |
| `bun run format`            | `cargo fmt --all`                           |
| `bun run dev`               | `cargo xtask dev` (api watch + `dx serve`)  |
| `bun run dev:api/web/cli`   | `cargo xtask dev-api` / `dev-web` / `dev-cli` |
| `bun run start`             | `cargo xtask start` (build all, api :8080 + preview :4173) |
| `bun run docker:build:api`  | `cargo xtask docker-build-api`              |
| `bun run docker:start:api`  | `cargo xtask docker-start-api`              |
| `bun run clean`             | `cargo clean`                               |

`cargo preview` is the stand-in for `vite preview`: it serves the bundle
directory (`apps/web/dist/public`) with an SPA fallback and proxies `/api/*`
and `/health` to the api on :8080 — the same shape as the Render static-site
rewrite in production.

**Build output contract:** `dx build` in 0.7 ignores `out_dir` in `Dioxus.toml`
(output stays under `target/dx/web/release/web/public`), so the repo uses
`dx bundle --release --out-dir dist`, which reliably produces
`apps/web/dist/public/{index.html,assets/*}`. That path is what `cargo
preview`, the Render static-site `staticPublishPath`, and CI all use.

## Components

### packages/common-lib

Pure library (no async runtime by default) so it compiles for
`wasm32-unknown-unknown` too.

- `formatters`: `format_bytes`, `format_count`, `format_seconds`, `format_unit`
  (`Option<f64>` → `String`, 3 significant digits, unit tables identical to TS).
- `math`: `round_to`, `sum` (pairwise split-sum like the TS one), `format_percentage`,
  `normalize_distribution`, `cumulative_distribution`, `format` (thousands separators).
- `utilities`: `get_env`/`require_env` (trimmed; empty → `None`),
  `error_to_string` (Display + source chain), `to_camel_case` (capitalise first char,
  preserving the TS name/behaviour), `pick`/`filter`/`is_empty` over
  `serde_json::Map`, and `sleep` behind the `tokio` feature (on by default).

### packages/claude-api

Thin client for `POST https://api.anthropic.com/v1/messages`. Rust has no
official Anthropic SDK, so this uses `reqwest` + `serde` directly.

- `CallClaudeOptions { system, messages, model, tools, max_tokens, api_key, fallbacks }`.
  Defaults: model `claude-opus-5`, `max_tokens` 16000, `fallbacks: "default"`
  (sends the `server-side-fallback-2026-07-01` beta header so refusals are
  re-routed server-side; set to `None` to opt out).
- `call_claude(&options) -> Result<Message, ClaudeApiError>`; also a
  `ClaudeClient::new(api_key).with_base_url(..)` for tests.
- API key: `options.api_key` → `ANTHROPIC_API_KEY` (via `dotenvy` + `get_env`)
  → `ClaudeApiError::MissingApiKey`.
- Response types are minimal `serde` structs (`Message`, `ContentBlock`, `Usage`,
  `stop_reason`) with `#[serde(other)]`/`serde_json::Value` fallbacks so new
  block types don't break deserialisation.
- Tests: `wiremock` server asserting default model, header set, and pass-through
  of system/messages/tools/max_tokens; missing-key error.

### packages/openai-summarizer

`summarize_text(text, &SummarizeOptions) -> Result<String, SummarizeError>` over
`POST /v1/chat/completions`. Defaults `gpt-4o-mini`, temperature 0.7. Empty
text → `SummarizeError::EmptyText`; missing key → `MissingApiKey`; empty
choices → `EmptySummary`. `base_url` override + `wiremock` tests.

### apps/api

axum 0.8. `app.rs` exposes `create_app() -> Router` (runtime-agnostic, tested
with `tower::ServiceExt::oneshot`); `main.rs` binds `0.0.0.0:$PORT` (default
8080). Routes:

- `GET /health` → `{ status: "ok", timestamp: <RFC3339> }`
- `GET /api/hello` → `{ message: "Hello from the api endpoint!" }`
- `GET /api/info` → `{ service, version, endpoints }`
- any other `/api` or `/api/*` → `404 { error: "not found" }`
- everything else → 404 (the web app is a static site, not served here)
- 10 MiB request body limit (`DefaultBodyLimit`).

Dockerfile: `rust:1-bookworm` builder → `debian:bookworm-slim` runtime,
`PORT=80`, `EXPOSE 80`, `CMD ["/usr/local/bin/api"]`. Build context is repo root.

### apps/cli

clap 4 derive. `cli hello` prints `Hello world!`; `cli summarize [--model]
[TEXT]` (reads stdin when no arg) calls `openai_summarizer`. Version from
`CARGO_PKG_VERSION`.

### apps/web

Dioxus web SPA (details in the "Dioxus specifics" section):

- `main.rs` — `dioxus::launch(App)`.
- `app.rs` — layout shell: nav with `Link`s + `Outlet`; Tailwind classes copied
  from the TS `App.tsx`.
- `routes.rs` — `#[derive(Routable)] enum Route { #[layout(App)] Home "/", About "/about" }`.
  **Add new pages here.**
- `pages/home.rs` — `use_resource` fetching `/api/hello` (relative path so the
  dev proxy and the prod rewrite both work), rendering loading / ok / error.
- `pages/about.rs` — static page.
- `tailwind.css` (crate root) — Tailwind v4 input (`@import "tailwindcss";
  @source "./src/**/*.rs"`). The dx CLI downloads the standalone Tailwind
  binary and compiles it to `assets/tailwind.css` (`tailwind_input` /
  `tailwind_output` in `Dioxus.toml`) on every `dx serve`/`dx bundle`. The
  output is committed (marked `linguist-generated`) because `asset!()`
  requires the file to exist at compile time for plain `cargo` builds/tests.
- `Dioxus.toml` — app name, title, Tailwind paths, `index_on_404` (client
  routing), and two `[[web.proxy]]` entries forwarding `/api` and `/health` to
  :8080. There is no port key: xtask passes `dx serve --port 5173`.
- Browser-only fetching uses `gloo-net` under `#[cfg(target_arch = "wasm32")]`
  (reqwest cannot resolve relative URLs); native builds compile a stub so the
  crate builds and tests on the host.
- Tests: render components to HTML with `dioxus_ssr` in plain `cargo test`
  (no browser). The routed layout is tested by wrapping `Router` in a
  `HistoryProvider` with `MemoryHistory::with_initial_path(..)`.
- CI lints/typechecks the crate for both the host and `wasm32-unknown-unknown`.

### generators

clap binary: `cargo run -p generators -- render-deploy <app-name> [--static]`.
Writes `apps/<app>/Dockerfile` (Rust multi-stage) and appends a service entry
to root `render.yaml`, named `<repo-dir-name>-<app-name>`; `--static` emits a
`runtime: static` entry whose `buildCommand` installs the Rust toolchain +
`dx` (via cargo-binstall), runs `dx bundle --release --out-dir dist`, and
publishes `apps/<app>/dist/public`. Unit tests cover `service_name_for`,
Dockerfile content, and yaml append/skip-duplicate.

### xtask

`cargo xtask <cmd>`: `build`, `dev`, `dev-api`, `dev-web`, `dev-cli`, `start`,
`preview`, `docker-build-api`, `docker-start-api`, `lint`, `lint-check`.
Spawns child processes (`cargo watch` if installed, else plain `cargo run`;
`dx serve --port 5173`), forwards Ctrl-C, and exits non-zero if any child
fails. Locates the Dioxus CLI via `$DX` → `~/.cargo/bin/dx` → PATH and
verifies `--version` mentions "dioxus" (Deno also ships a `dx` shim).

## CI

`.github/workflows/ci.yml` on `pull_request`: four jobs mirroring the TS repo —
`build` (cargo build + dx web build), `typecheck` (`cargo check`), `test`,
`lint` (`clippy -D warnings` + `fmt --check`). Uses `dtolnay/rust-toolchain`,
`Swatinem/rust-cache`, and `cargo-binstall` for `dioxus-cli`.

## Error handling

Each package has a `thiserror` enum; apps map errors to exit codes (cli) or
JSON responses (api). No `anyhow` in libraries; `anyhow` allowed in bins/xtask.

## Testing

`cargo test --workspace` runs everything: unit tests colocated in each module,
HTTP clients tested against `wiremock`, axum routes via `oneshot`, Dioxus
components via `dioxus_ssr`. No network or API keys needed.

## Assumptions (made because the session was autonomous)

1. Client-side WASM SPA + separate axum API (matches the TS hybrid pattern)
   rather than Dioxus fullstack.
2. Default Claude model `claude-opus-5` with `fallbacks: "default"` (the TS
   repo defaulted to `claude-haiku-4-5`; the claude-api skill mandates Opus 5
   and server-side fallbacks for new code). Override via `CallClaudeOptions`.
3. Tailwind via the `dx` CLI's built-in integration rather than a separate npm
   process, so no Node/Bun is needed to develop.
4. `xtask` + cargo aliases instead of `just`/`cargo-make`, so a fresh clone
   needs only `rustup` and `dx`.
