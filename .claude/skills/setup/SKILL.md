---
name: setup
description: Check prerequisites, build the repo, and verify everything works. Use when first cloning the repo or when setup seems broken.
---

# Repository Setup

Run through each section below in order. For each check, run the command, inspect the output, and report the result to the user. If a check fails, attempt the fix before moving on. If the fix also fails, stop and tell the user what went wrong.

## 1. Check Rust

Run `cargo --version` and `rustc --version`. `rust-toolchain.toml` pins the stable channel and adds the `wasm32-unknown-unknown` target + clippy + rustfmt; rustup installs them on first use.

- **If missing:** Tell the user to install rustup from https://rustup.rs. Do NOT install it automatically — just inform the user and stop.
- Run `rustup target list --installed` and confirm `wasm32-unknown-unknown` is present; if not, run `rustup target add wasm32-unknown-unknown`.

## 2. Check the Dioxus CLI

Run `~/.cargo/bin/dx --version` (fall back to `dx --version`). The output must contain `dioxus` and the version should be **0.7.x**, matching the `dioxus` crate in the root `Cargo.toml`.

- **Beware of shadowing:** other tools (e.g. Deno) install a different `dx` binary that may come first on PATH. `cargo xtask` handles this (it checks `$DX`, then `~/.cargo/bin/dx`, then PATH and verifies the version output), but tell the user if the `dx` on their PATH is not Dioxus.
- **If missing:** Ask the user if they'd like you to install it with `cargo install dioxus-cli --locked` (several minutes) or `cargo binstall dioxus-cli` (seconds, needs cargo-binstall). If they decline, stop and tell them it is required for the web app.

## 3. Check Docker (optional)

Run `docker --version`. Only needed for `cargo docker-build-api` / deploy parity; warn if missing, don't block.

## 4. Update the project name

Find all instances of `rust-ai-toolbench` in the repository (root `Cargo.toml` `repository`, `xtask/src/main.rs` Docker image name, `CLAUDE.md`, README), and replace them with the name of the repository. Choose the name of the project's root directory, even if it differs from the repository name according to git.

## 5. Rewrite the README for this project

The README.md still describes the template this repo was created from ("This is a template for a Cargo-workspace monorepo..."). Rewrite it to describe **this** project.

1. **Check whether this is still needed:** If the README's title and intro already describe this project (not the template), report "already done" and skip to the next step.
2. **Determine the project name:** Use the project's root directory name (same convention as step 4).
3. **Determine the project's purpose:** Ask the user for a one-to-two sentence explanation of what this project is for. You should almost always ask — only skip asking when the root directory name makes the purpose unambiguous.
4. **Rewrite README.md:** Replace the title and intro with the project name and purpose; remove template-specific language; keep the Setup, Commands, and Environment variables sections — they describe this repo's mechanics and remain accurate. Keep the Features section only if it still reflects what the project uses.

## 6. Build all packages and apps

Run `cargo build-all` from the repo root. This runs `cargo build --workspace` (every member in dependency order) and then `dx bundle --release --out-dir dist` for `apps/web` (dx downloads wasm-bindgen, wasm-opt, and Tailwind on first run — this can take a few minutes).

- **If it fails:** Show the error output. Common causes: missing wasm target (step 1), wrong `dx` (step 2), compile errors in source code. Help the user diagnose.

## 7. Run type checking

Run `cargo typecheck` (and `cargo check -p web --target wasm32-unknown-unknown` for the browser code path).

- **If it fails:** Show which crate(s) failed and the errors. This usually indicates a code issue, not a setup issue — inform the user.

## 8. Run tests

Run `cargo test --workspace`. No network, API keys, or browser are needed.

- **If it fails:** Show the failing tests. Distinguish between test failures (code issue) and a broken toolchain (setup issue).

## 9. Run linter

Run `cargo lint-check`.

- **If it fails due to lint errors:** That's fine — the linter works. Tell the user there are lint issues they can fix with `cargo lint`.
- **If it fails because clippy/rustfmt are missing:** Run `rustup component add clippy rustfmt`.

## 10. Check environment variables

Check for environment variables that packages in this repo may need at runtime. Do NOT create any files — just warn about what's missing.

**Required env vars to check:**

- None (for now)

**Optional env vars:**

- `RENDER_API_KEY` — Only needed if deploying to Render. Note if missing, but don't treat it as blocking.
- `OPENAI_API_KEY` — Used by `openai-summarizer` (`cli summarize`). Warn if unset.
- `ANTHROPIC_API_KEY` — Used by `claude-api`. Warn if unset.

No `.env` is needed to run the API server locally — it defaults to port 8080, which is what the Dioxus dev proxy targets. `apps/api/.env.example` exists only as a template for overriding `PORT`; mention it only if the user wants a non-default port (and note the `[[web.proxy]]` backends in `apps/web/Dioxus.toml` must be updated to match).

## 11. Summary

Print a summary table of all checks:

| Step                   | Status       |
| ---------------------- | ------------ |
| Rust toolchain         | pass/fail    |
| Dioxus CLI (dx 0.7)    | pass/fail    |
| Docker                 | pass/warn    |
| Update template name   | pass/fail    |
| README rewritten       | pass/skipped |
| Build                  | pass/fail    |
| Type check             | pass/fail    |
| Tests                  | pass/fail    |
| Linter                 | pass/fail    |
| Environment variables  | pass/warn/ok |

If everything passed, tell the user they're good to go and remind them of the key commands:

- `cargo dev` — Start api and web dev servers together
- `cargo start` — Build everything, then run the full stack: api (:8080) + web preview (:4173)
- `cargo dev-cli -- hello` — Run the CLI
- `cargo docker-build-api` / `cargo docker-start-api` — build and run the API in Docker
- `cargo test --workspace` — Run tests
- `cargo lint` — Lint and fix

---

## Maintaining This Skill

Update this skill when the repo's setup requirements change:

- **New runtime dependency added** (e.g., a package starts requiring Redis or Postgres): add a check section before the summary.
- **New environment variable required**: add it to section 10 with the package that uses it and whether it's required.
- **Rust or dx version requirement changes**: update sections 1–2 (and `rust-toolchain.toml` / the `dioxus-cli` version in `.github/workflows/ci.yml`).
- **New workspace crate added**: no build-script change needed — `cargo build --workspace` covers all members. If the new crate has unique prerequisites (native libraries, external services), add a check here.
- **New cargo alias / xtask command added**: only update this skill if it represents a setup-critical step; otherwise add it to the "key commands" reminder if users should know about it.

When in doubt, ask: "Would a fresh clone fail or behave unexpectedly without this change in /setup?" If yes, update the skill.
