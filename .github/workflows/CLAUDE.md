# GitHub Workflows

## Overview

CI/CD pipeline configuration for the monorepo.

## Key Files

- `ci.yml` - Continuous integration workflow: `build` (cargo + `dx bundle` via `cargo xtask build`), `typecheck` (`cargo check`, native + wasm32), `test` (`cargo test --workspace`), `lint` (`cargo lint-check` = clippy `-D warnings` + `rustfmt --check`, plus wasm32 clippy for `apps/web`). The Dioxus CLI version installed in CI (`dioxus-cli@0.7.10`) should match the `dioxus` crate minor version in the root `Cargo.toml`.
- `audit.yml` - Nightly (07:00 UTC, plus manual `workflow_dispatch`) `cargo audit` of `Cargo.lock` against RustSec. On failure it opens a `cargo-audit`-labelled issue assigned to the repo owner (or comments on the existing open one), which is how the report reaches the maintainer's email. Close the issue once the advisories are fixed so the next failure opens a fresh one.

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
