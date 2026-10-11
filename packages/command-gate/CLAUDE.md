# command-gate

## Overview

Library behind `apps/remote-exec`: passkey-gated execution of a TOML allowlist of commands, reached from a phone over Tailscale. It owns the axum router, the embedded UI (`src/ui/`, served via `include_str!`), WebAuthn, sessions, argument validation and process execution. Native-only (uses `nix`, `tokio::process`, OpenSSL via `webauthn-rs`), so unlike most packages it is not wasm-compatible and depends on tokio directly.

## Security model (do not weaken without a reason written here)

| Layer | Where | Invariant |
| --- | --- | --- |
| Network | `config.rs`, `server.rs::guard` | `bind` must be loopback (config error otherwise). In `Mode::Tailscale` every request needs `Tailscale-User-Login` ∈ `allowed_tailscale_logins`. Loopback keeps *other tailnet peers* from forging the header; it does **not** stop other local users/processes, who can connect and set any header — passkeys are the real gate against them, and pending ceremonies evict oldest-first so they can't lock the owner out. Run on a machine where you trust local users. |
| CSRF | `server.rs::guard` | Every non-GET needs `Origin` == configured origin and, if present, `Sec-Fetch-Site: same-origin`. Cookie is `__Host-`, `HttpOnly`, `Secure`, `SameSite=Strict`. |
| AuthN | `auth.rs` | WebAuthn passkeys (user verification required). Enrollment needs a one-time 256-bit token printed on the host terminal, carried in the URL fragment. Sessions: random 256-bit token, stored server-side as SHA-256, sliding idle TTL + 12 h absolute cap (cookie `Max-Age` = the cap; idle is enforced server-side), bound to the Tailscale login that created them. Under systemd, enrollment never auto-opens (token would land in the journal). |
| AuthZ | `config.rs` | Only `[[command]]` entries run. Config file must be owned by root/us and not group/world-writable. `program` is absolute, executable, owned by root/us, not group/world-writable, with no world-writable ancestor dir (checked after symlink resolution). Strict TOML (`deny_unknown_fields`). |
| Args | `args.rs` | Placeholders fill whole argv elements only. Kinds: `enum` (exact), `integer` (strict decimal, bounded, canonicalised), `word` (`[A-Za-z0-9_][A-Za-z0-9._-]*`), `path` (word components, canonicalised, must stay under root, not root itself, resolved components re-checked so symlinks can't reach root or dot-dirs). Integer `min >= 0`, so no value can start with `-`. |
| Exec | `exec.rs`, `server.rs::execute` | Direct `execve`, never a shell. `env_clear` + fixed `PATH` + `pass_env` allowlist. stdin `/dev/null`, own process group (group and direct child killed on timeout or if pipes stay open; waits are bounded), 64 KiB output cap per stream, one command at a time. Runs in a detached task so a dropped connection can't orphan it or skip the audit. |
| Audit | `audit.rs` | JSONL (`data_dir/audit.jsonl`, 0600): `started` before exec, then `finished`/`timeout`/`exec-failed`; also `rejected-args` and `unknown-command`. |
| Process | `lib.rs::serve` | Refuses to run as root. `data_dir` 0700, `passkeys.json` 0600 (rejected if group/world-readable). |
| Browser | `server.rs`, `ui/` | Strict CSP (`script-src 'self'`, no inline), no `innerHTML`, `no-store`, `DENY` framing. |

Known non-goals: the allowlist cannot make an *interpreter* safe (`bash -c`, `python`, `find -exec`, `git -c`) — don't allowlist them. There is no per-command step-up re-auth; a live session can run any allowlisted command.

## Layout

- `src/lib.rs` — `serve(opts, shutdown)` + startup checks
- `src/config.rs` — TOML schema + validation (`Config::from_toml`, `Mode`)
- `src/args.rs` — `render_argv`, `validate`
- `src/exec.rs` — `run` (shell-free, bounded)
- `src/auth.rs` — `Auth` (passkey store, ceremonies, sessions, enrollment)
- `src/server.rs` — `router`, `AppState`, `guard` middleware, handlers
- `src/audit.rs` — `AuditLog`
- `src/ui/` — `index.html`, `app.js`, `app.css` (no build step)
- `tests/e2e.rs` — full enroll → login → exec → injection attempts → logout, using `webauthn-authenticator-rs`'s software passkey

## Commands

- `cargo test -p command-gate` — unit tests + the e2e passkey test (no network, no browser)
- `cargo clippy -p command-gate --all-targets -- -D warnings`

## Dependencies

- **axum**, **tower-http** — router, tracing layer
- **tokio** — process spawning, timeouts, signals
- **webauthn-rs** (0.5) — passkey registration/authentication (links OpenSSL; needs `libssl-dev` + `pkg-config` to build)
- **nix** — `killpg`, `geteuid`
- **toml**, **serde**, **serde_json** — config and wire format
- **sha2**, **subtle**, **getrandom** — session hashing, constant-time token compare, CSPRNG
- **chrono**, **tracing**, **thiserror**
- dev: **webauthn-authenticator-rs** (`softpasskey`), **tempfile**, **tower**, **http-body-util**

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
