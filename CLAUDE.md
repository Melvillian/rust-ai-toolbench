# CLAUDE.md

## Overview

Rust monorepo (Cargo workspace, edition 2024). Rust stable via `rust-toolchain.toml`; the Dioxus CLI (`dx` 0.7) is required for the web app.

- `apps/*` — Thin executable wrappers. Each app's entrypoint should do minimal work: parse CLI args, set up a server, or mount the UI, then delegate to a package. Apps should not contain business logic.
- `packages/*` — Importable library crates containing the actual logic. Each package exposes an interface that the corresponding app depends on. There is no standard interface shape; it depends on what the module does.
- `generators/` — Code generation CLI for scaffolding project files (e.g. Render deploy configs). A workspace member like the others, so `cargo build --workspace`/`typecheck`/`lint-check`/`test` all cover it.
- `xtask/` — Dev orchestration (`cargo xtask …`); the home of every root "script" that runs more than one process.

Cargo builds every workspace member in dependency order and refuses to build with a missing member, so there is no per-package script to forget — the four correctness gates below cover everything by construction.

## Commands

Aliases are defined in `.cargo/config.toml`; see `xtask/CLAUDE.md` for what each spawns.

- `cargo build-all` - Build all packages and apps (`cargo build --workspace` + `dx bundle --release --out-dir dist` for the web app)
- `cargo dev` - Start api (:8080) and the Dioxus dev server with hot reload (:5173) together
- `cargo dev-api` - Run the API server alone (auto-reload if `cargo-watch` is installed)
- `cargo dev-cli -- <args>` - Run the CLI app
- `cargo dev-web` - Run the Dioxus dev server alone
- `cargo docker-build-api` - Build the API Docker image (from the working tree)
- `cargo docker-start-api` - Run the API Docker image on port 8080
- `cargo start` - Build everything, then run the full stack from prod builds: api (:8080) + web via `cargo preview` (:4173, serves `apps/web/dist/public` and proxies `/api` to the api — the local stand-in for the static site's prod rewrite)
- `cargo test --workspace` - Run tests
- `cargo lint` - Lint and fix (clippy `--fix` + rustfmt)
- `cargo lint-check` - Lint check only (clippy `-D warnings` + `rustfmt --check`)
- `cargo typecheck` - Typecheck all packages (`cargo check --workspace --all-targets`)
- `cargo fmt --all` - Format code
- `cargo clean` - Remove target dir

## Render

- When using the Render MCP, always select this workspace first using `select_workspace` with id `tea-cspvkb8gph6c73ft0hd0`
- **Service naming**: when adding a new service to `render.yaml`, name it `<repository-name>-<app-name>` (e.g. `rust-ai-toolbench-api` for `apps/api` in this repo). Render appends its own random suffix to the `.onrender.com` subdomain — do not try to include or control the suffix in `render.yaml`. Generic names like `api` produce unrecognizable domains like `api-mvqh.onrender.com`.

- **Not on Render:** `apps/remote-exec` (+ `packages/command-gate`) runs on the owner's machine, loopback-bound behind `tailscale serve`. Never add it to `render.yaml`. Its security invariants are listed in `packages/command-gate/CLAUDE.md`; read them before changing auth, args or exec.

### Deploy concepts

- Deploy concepts (static site vs web service decision, blueprint patterns, Rust/dx toolchain requirements, build order) live in the `render-deploys` skill.
- **Static-site-first**: frontend apps deploy as Render static sites by default (free, CDN-served). `apps/api` is API-only — it does NOT serve `apps/web`; in production the static site's `/api/*` rewrite proxies to the api service (hybrid pattern).

## Claude API

`packages/claude-api` defaults to `claude-opus-5` with server-side fallbacks enabled (see its CLAUDE.md). Rust has no official Anthropic SDK, so it calls the REST API directly.
