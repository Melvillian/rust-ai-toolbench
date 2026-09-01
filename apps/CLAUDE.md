# apps/

Thin executable wrappers. Each app's entrypoint parses CLI args, sets up a server, or mounts a UI, then delegates to a package in `packages/`. Apps must not contain business logic.

## Required shape for every app

Every crate under `apps/` MUST:

1. Be listed in the root `Cargo.toml` `[workspace] members` — cargo refuses to build a workspace with a missing member, so unlike `bun --filter` nothing can be silently skipped. `cargo build --workspace`, `cargo typecheck`, `cargo test --workspace`, and `cargo lint-check` therefore cover every app automatically.
2. Use `[lints] workspace = true` so clippy pedantic + `-D warnings` apply uniformly.
3. Inherit `version`, `edition`, `license`, `rust-version`, `repository` from `[workspace.package]` and take dependencies via `dep.workspace = true`.
4. Have a `CLAUDE.md` describing its purpose, commands, and dependencies.

Binary crates that need testable logic should split it into a library target (see `generators/`, `xtask/`) or a module with unit tests (see `apps/api/src/app.rs`).

## Verification before considering a new app done

From the repo root, all of these must pass:

```bash
cargo build --workspace && cargo typecheck && cargo test -p <app-name> && cargo clippy -p <app-name> --all-targets -- -D warnings
```

For browser apps also: `cargo clippy -p <app-name> --target wasm32-unknown-unknown -- -D warnings`.

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
