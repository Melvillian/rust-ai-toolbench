# remote-exec

## Overview

Thin binary over `packages/command-gate`: parses flags, sets up tracing and signal handling (Ctrl-C + SIGTERM), then calls `command_gate::serve`. Runs on the owner's own machine, bound to loopback and exposed only through `tailscale serve`. Not deployed to Render (it executes commands on the host it runs on). See `README.md` for setup.

## Commands

- `cargo run -p remote-exec -- --config <path> [--enroll] [--dev] [--check]`
- `cargo install --path apps/remote-exec` — installs to `~/.cargo/bin/remote-exec` (what `remote-exec.service` expects)
- `cargo test -p remote-exec`, `cargo clippy -p remote-exec --all-targets -- -D warnings`

## Files

- `src/main.rs` — clap CLI (`--config`/`REMOTE_EXEC_CONFIG`, `--enroll`, `--dev`, `--check`)
- `commands.example.toml` — annotated config template
- `remote-exec.service` — hardened systemd user unit
- `README.md` — setup walkthrough

## Dependencies

- **command-gate** (workspace) — all logic
- **clap**, **tokio**, **tracing**, **tracing-subscriber**, **anyhow**

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
