# packages/

Importable Rust library crates containing the actual logic. Each package exposes an interface that a corresponding app (or another package) depends on. There is no standard interface shape; it depends on what the module does.

## Required shape for every package

Every crate under `packages/` MUST:

1. Be listed in the root `Cargo.toml` `[workspace] members` and, for path use by siblings, in `[workspace.dependencies]` (`name = { path = "packages/<name>" }`). Cargo builds workspace members in dependency order and refuses to build with a missing member, so every package is covered by `cargo build --workspace`, `cargo typecheck`, `cargo test --workspace`, and `cargo lint-check` with no per-package script to forget.
2. Use `[lints] workspace = true` and inherit package metadata from `[workspace.package]`.
3. Use `thiserror` enums for errors (no `anyhow` in libraries).
4. Keep external I/O behind an injectable base URL / client so tests run without network (see `claude-api`, `openai-summarizer` + `wiremock`).
5. Prefer no default async-runtime dependency so the crate can also be used from `apps/web` (wasm32) — gate runtime-specific helpers behind a feature (see `common-lib`'s `tokio` feature).
6. Have a `CLAUDE.md` describing its purpose, commands, and dependencies.

## Verification before considering a new package done

```bash
cargo build --workspace && cargo typecheck && cargo test -p <package-name> && cargo clippy -p <package-name> --all-targets -- -D warnings
```

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
