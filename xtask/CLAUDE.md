# xtask

## Overview

Dev orchestration binary invoked as `cargo xtask <task>` (alias in `.cargo/config.toml`). It is the Rust analogue of the TS repo's root `package.json` scripts: pure cargo tasks are cargo aliases, and anything that spawns several processes (dev servers, docker, the preview proxy) lives here. Library modules (`dx`, `preview`, `process`) hold the testable pieces; `main.rs` is the clap CLI.

## Aliases → what they run

| Alias                     | Runs                                                                 |
| ------------------------- | -------------------------------------------------------------------- |
| `cargo build-all`         | `cargo build --workspace` then `dx bundle --release --out-dir dist` in `apps/web` (static site lands in `apps/web/dist/public`)    |
| `cargo dev`               | api (`cargo watch -x 'run -p api'` if installed, else `cargo run`) + `dx serve --port 5173` |
| `cargo dev-api`           | api only                                                             |
| `cargo dev-web [-- args]` | `dx serve --port 5173` only                                          |
| `cargo dev-cli [-- args]` | `cargo run -p cli -- args`                                           |
| `cargo start`             | `build-all`, then release api (:8080) + `preview` (:4173) together   |
| `cargo preview`           | serves `apps/web/dist/public` with SPA fallback, proxies `/api` + `/health` to :8080 |
| `cargo docker-build-api`  | `docker build -t rust-ai-toolbench-api -f apps/api/Dockerfile .`     |
| `cargo docker-start-api`  | `docker run --rm --init -p 8080:80 rust-ai-toolbench-api`            |
| `cargo lint-check`        | `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all -- --check` |
| `cargo lint`              | `cargo clippy … --fix` + `cargo fmt --all`                            |
| `cargo typecheck`         | `cargo check --workspace --all-targets` (plain alias, not xtask)     |

Concurrent tasks forward Ctrl-C to every child and stop all of them if one exits.

## Locating `dx`

Other tools also ship a `dx` binary (Deno's `deno x` shim is first on many PATHs). `xtask::dx::locate` tries `$DX`, then `~/.cargo/bin/dx`, then `dx` on PATH, and only accepts a binary whose `--version` output mentions "dioxus".

## Commands

- `cargo test -p xtask` - unit tests + the preview-server integration test
- `cargo clippy -p xtask --all-targets -- -D warnings` - Lint

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
