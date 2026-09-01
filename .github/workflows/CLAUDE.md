# GitHub Workflows

## Overview

CI/CD pipeline configuration for the monorepo.

## Key Files

- `ci.yml` - Continuous integration workflow: `build` (cargo + `dx bundle` via `cargo xtask build`), `typecheck` (`cargo check`, native + wasm32), `test` (`cargo test --workspace`), `lint` (`cargo lint-check` = clippy `-D warnings` + `rustfmt --check`, plus wasm32 clippy for `apps/web`). The Dioxus CLI version installed in CI (`dioxus-cli@0.7.10`) should match the `dioxus` crate minor version in the root `Cargo.toml`.

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
