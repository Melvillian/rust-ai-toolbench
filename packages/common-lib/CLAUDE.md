# common-lib

## Overview

Shared utility library providing formatters (bytes, counts, seconds), math helpers (distributions, rounding, percentages), and general utilities (env access, error handling, object helpers, sleep, `to_camel_case`). Pure library with no async runtime by default: the `tokio` cargo feature (on by default) gates only `sleep`, so `default-features = false` builds for `wasm32-unknown-unknown`.

## Commands

- `cargo build -p common-lib` - Compile the crate
- `cargo test -p common-lib` - Run the colocated unit tests
- `cargo clippy -p common-lib --all-targets -- -D warnings` - Lint (workspace lints: clippy all + pedantic)
- `cargo check -p common-lib --no-default-features --target wasm32-unknown-unknown` - Verify the wasm build

## Key Files

- `src/lib.rs` - Module declarations and crate-root re-exports of every public item
- `src/formatters/` - Unit formatting (`format_unit`, `format_bytes`, `format_count`, `format_seconds`)
- `src/math/` - Math utilities (`round_to`, `sum`, `format_percentage`, `normalize_distribution`, `cumulative_distribution`, `format`)
- `src/utilities/` - General helpers (`get_env`/`require_env`, `error_to_string`, `pick`/`filter`/`is_empty`, `sleep`, `to_camel_case`)
- `src/js_compat.rs` - Private helpers reproducing JS `String(n)` and `Math.round` semantics so output matches the original TypeScript library

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
