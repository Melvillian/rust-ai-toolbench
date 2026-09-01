# Generators

## Overview

Code generation tool for scaffolding project files (e.g., Render deploy configs). Built with clap. The pure pieces (content builders, `render.yaml` append logic) live in the library (`src/lib.rs` → `src/commands/`) so they are unit-testable; `src/main.rs` is the thin CLI.

## Commands

- `cargo run -p generators -- render-deploy <app> [--static]` - Run the generator (builds on demand)
- `cargo test -p generators` - Unit + end-to-end tests (temp dirs, no side effects)
- `cargo clippy -p generators --all-targets -- -D warnings` - Lint

## Conventions

- **Render service naming**: the `render-deploy` command names services `<repository-name>-<app-name>` (e.g. `caroline-nanny-website-api`), not the bare app name. The repository name is derived from the basename of the directory the generator is run from (the repo root). Render appends its own random suffix to the `.onrender.com` subdomain, so bare names like `api` yield unrecognizable domains like `api-mvqh.onrender.com`. See `service_name_for` in `src/commands/render_deploy.rs`.
- Static-site entries install the Rust toolchain + `dx` in Render's native build environment and publish `apps/<app>/dist/public` (`dx bundle --release --out-dir dist`; see the `render-deploys` skill).

## Dependencies

- **clap** - CLI argument parsing
- **anyhow** - error context in the binary

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
