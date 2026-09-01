---
name: render-deploys
description: Render deploy concepts — deciding static site vs web service, the static-site and Docker blueprint patterns, Rust/dx toolchain requirements, and workspace build order. Use when adding or editing render.yaml, deploying a new app to Render, running the render-deploy generator, debugging a Render build, or reproducing a Render build locally.
---

# Render deploy concepts

## Decision: static site vs web service

**Default every frontend app to a static site** (`runtime: static`). It has no
fixed compute cost (a Docker web service costs $7+/mo on starter) and serves
from Render's CDN. Only use a **web service** for apps that genuinely need a
server at request time.

An app requires a web service if ANY of these hold — check the frontend and
every sibling app, not the framework:

1. **SSR / server-rendered framework.** A Dioxus *fullstack* app with server
   functions or SSR. (A client-only `dioxus/web` build is fine.)
2. **Secrets needed at request time.** Static builds may use build-time env
   vars, but their values are baked into the public WASM bundle — anything
   secret disqualifies pure-static.
3. **Server-only behavior.** Websockets, background jobs, request-time auth,
   file uploads, redirects computed per-request.
4. **It IS the server.** An app like `apps/api` that implements `/api/*`
   routes is a web service by definition.

**A frontend that calls a same-repo API is still a static site** — that's the
**hybrid pattern**, and it is this template's default shape: `apps/web`
fetches `/api/hello`, so a derived repo deploys `apps/web` as a static site
whose `routes` rewrite proxies `/api/*` to the `apps/api` web service. You pay
only for the API. Static XOR web service is a false choice.

**Do not judge by the mere existence of a server app.** A server whose only
routes are static-file serving, an SPA fallback, and health/info endpoints
exists solely to host the SPA — delete it and go pure-static. Enumerate the
routes; ignore the framework. Conversely, if a derived repo's frontend stops
calling `/api/*`, delete `apps/api` and the rewrite, leaving a pure static
site.

## Generating the config

Both paths go through the generator (see `/generate-render-deploy`):

- **Static site:** `cargo run -p generators -- render-deploy web --static`
  — appends a `runtime: static` entry to `render.yaml` (no Dockerfile). The
  entry contains an SPA fallback rewrite and a commented hybrid `/api/*`
  rewrite; fill in the api service's real `.onrender.com` URL (with Render's
  random suffix, known only after that service's first deploy) to activate it.
- **Web service:** `cargo run -p generators -- render-deploy api` —
  writes `apps/api/Dockerfile` and appends an `env: docker` entry.

Static-site notes:

- The build runs in Render's **native environment**, which has no Rust
  toolchain, so the generated `buildCommand` installs `rustup` (minimal
  profile + `wasm32-unknown-unknown`), `cargo-binstall`, and `dioxus-cli`,
  then runs `dx bundle --release --out-dir dist` inside `apps/<app>`. The
  bundle lands in `apps/<app>/dist/public`, which is the `staticPublishPath`
  (dx 0.7 ignores `out_dir` in `Dioxus.toml` for `dx build`; `dx bundle
  --out-dir` is the reliable way to choose the output location).
- Local parity: `cargo build-all` runs the same `dx bundle --release --out-dir dist`.
- Static sites cannot use `startCommand`/`dockerCommand`; static-only fields
  are `staticPublishPath`, `routes`, and `headers`. Routes match in order —
  the `/api/*` rewrite must precede the `/*` SPA fallback.
- Render prefixes subdomain slugs that start with a digit (e.g. `2026-...` →
  `two026-...`); use a custom domain if the slug matters.

## Web service (Docker) concepts

- **Docker vs native runtime:** web services use `env: docker`, so build and
  run come from the **Dockerfile** (`RUN`/`CMD`) — NOT `render.yaml`
  `buildCommand`/`startCommand` (those are native-runtime only). The
  Dockerfile is the single source of truth, giving local == Render parity.
  Reproduce a Render build locally with `cargo docker-build-api` (directory
  context `.`, so the root `.dockerignore` applies). The build uses your
  working tree as-is — commit and push first if you want it to mirror exactly
  what Render builds from your branch.
- **The Dockerfile builds one workspace package** with
  `cargo build --release --package <app> --locked`, which needs the committed
  `Cargo.lock`. The runtime image is `debian:bookworm-slim` + `ca-certificates`
  (outbound TLS uses rustls, so no libssl). The binary listens on `PORT`
  (80 in the image).

## Shared concepts

- **Build order:** Cargo orders workspace members by dependency automatically;
  `cargo build --workspace` and `cargo xtask build` need no manual step when a
  package or app is added.
- **Blueprint sync:** pushing `render.yaml` changes to main creates/updates
  services automatically (first-time repos need a one-time Blueprint
  connection in the Render dashboard).
- **Migrating a service between types:** changing `type`/`runtime`/`name` in
  the blueprint creates a NEW service; it never converts the old one.
  Sequence: add the new service entry, sync, verify, move custom domains, then
  delete the old service (removing its entry from `render.yaml` does not
  destroy the running service — confirm deletion in the dashboard or via the
  API).
