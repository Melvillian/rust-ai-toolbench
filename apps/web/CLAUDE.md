# web

## Overview

Dioxus 0.7 web SPA (client-side rendered WASM) with `dioxus-router` and Tailwind CSS v4.
Bundled to `dist/public` via `dx bundle --release --out-dir dist` and deployed as a
Render static site (SPA-fallback rewrite; `/api/*` rewrite proxies to the `apps/api`
service — see the `render-deploys` skill). In dev, `dx serve` proxies `/api` and
`/health` to axum on :8080 (`[[web.proxy]]` in `Dioxus.toml`).

## Commands (run from repo root)

- `cargo dev` - api (:8080) + `dx serve --port 5173` with hot reload
- `cargo dev-web` - Dioxus dev server alone
- `cargo build-all` - `cargo build --workspace` + `dx bundle --release --out-dir dist`
- `cargo start` - build everything, then api (:8080) + `cargo preview` (:4173)
- `cargo test -p web` - SSR tests (`dioxus_ssr`, no browser needed)
- `cargo clippy -p web --all-targets -- -D warnings` and
  `cargo clippy -p web --target wasm32-unknown-unknown -- -D warnings` - lint both targets

`dx` must be the Dioxus CLI (`cargo install dioxus-cli --locked`); other tools also
install a `dx` binary, so `xtask` looks in `$DX`, then `~/.cargo/bin/dx`, then PATH.

## Structure

- `src/main.rs` - entrypoint: `dioxus::launch(Root)`; `Root` mounts the Tailwind stylesheet + `Router`
- `src/app.rs` - layout shell (nav + `Outlet`) and the router-level SSR test
- `src/routes.rs` - route table: **add new pages here**
- `src/pages/` - one component per route
- `tailwind.css` - Tailwind v4 input (`@import "tailwindcss"; @source "./src/**/*.rs"`)
- `assets/tailwind.css` - **generated** by the dx CLI (`tailwind_input`/`tailwind_output` in `Dioxus.toml`); committed because `asset!()` needs the file at compile time, regenerated on every `dx serve`/`dx bundle`
- `Dioxus.toml` - app name, title, Tailwind paths, watcher (`index_on_404` for client routing), dev proxy

## Conventions

- Add pages in `src/pages/`, register them in `src/routes.rs`; `app.rs` stays a pure layout shell.
- Fetch APIs by relative path (`/api/...`) so the dev proxy and the static site's prod `/api/*` rewrite both work. Browser-only code (`gloo-net`) is `#[cfg(target_arch = "wasm32")]`; native builds compile a stub so SSR tests run on the host.
- Test components with `dioxus_ssr::render_element(rsx! { Page {} })`; for routed pages wrap `Router` in a `HistoryProvider` with `MemoryHistory::with_initial_path` (see `app.rs`).
- Features: `default = ["web"]`, `web = ["dioxus/web"]` — the standard Dioxus platform-feature layout, so `dx` can select the platform.

## Auto-Update Instructions

After changes to files in this directory, run `/update-claude-md`.
