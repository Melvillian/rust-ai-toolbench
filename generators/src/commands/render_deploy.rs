//! `generators render-deploy <app-name> [--static]`
//!
//! Adds Render deployment config for an app in `apps/`: either a static-site
//! entry in `render.yaml` (`--static`), or a Dockerfile plus a Docker web
//! service entry (default).

use std::path::Path;

use anyhow::Context;
use clap::Args;

/// Path of the blueprint file, relative to the repo root.
pub const RENDER_YAML_PATH: &str = "render.yaml";

#[derive(Debug, Args)]
pub struct RenderDeployArgs {
    /// Name of the app directory under apps/ (e.g. my-service)
    pub app_name: String,

    /// Deploy as a Render static site (no server, no Dockerfile; the app must
    /// bundle to apps/<app-name>/dist/public via `dx bundle --out-dir dist`)
    #[arg(long = "static")]
    pub is_static: bool,
}

/// Result of [`append_to_render_yaml`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendOutcome {
    /// The entry was appended.
    Written,
    /// An entry with the same service name already existed; nothing changed.
    AlreadyPresent,
}

/// Run the command in the current directory (expected to be the repo root).
pub fn run(args: &RenderDeployArgs) -> anyhow::Result<()> {
    let target = Path::new("apps").join(&args.app_name);
    println!("Adding Render deploy files to: {}", target.display());

    if !args.is_static {
        let dockerfile = target.join("Dockerfile");
        std::fs::write(&dockerfile, dockerfile_content(&args.app_name))
            .with_context(|| format!("failed to write {}", dockerfile.display()))?;
        println!("✓ Created Dockerfile");
    }

    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let repo_name = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .context("current directory has no usable name")?;
    let service_name = service_name_for(repo_name, &args.app_name);

    match append_to_render_yaml(
        &args.app_name,
        repo_name,
        Path::new(RENDER_YAML_PATH),
        args.is_static,
    )? {
        AppendOutcome::Written => println!("✓ Updated render.yaml"),
        AppendOutcome::AlreadyPresent => {
            println!("⚠ render.yaml already contains a service named '{service_name}', skipping");
        }
    }

    println!("\n✓ Render deploy files generated successfully!");
    println!("\nNext steps:");
    println!("1. Create a Blueprint in the Render dashboard pointing to render.yaml");
    println!("   (one-time setup: https://dashboard.render.com/select-repo?type=blueprint)");
    if args.is_static {
        println!("2. If this app fetches same-origin /api/* from a sibling web service,");
        println!("   fill in the commented rewrite in render.yaml with that service’s");
        println!("   real .onrender.com URL (known after its first deploy)");
    } else {
        println!("2. Commit your changes:");
        println!("   git add -A && git commit");
    }
    println!("3. Push to main to deploy:");
    println!("   git push origin main");
    Ok(())
}

/// Multi-stage Dockerfile that builds one workspace package and ships only
/// its release binary.
pub fn dockerfile_content(app_name: &str) -> String {
    format!(
        r#"# Stage 1: Build with the official Rust image (ships a C toolchain for any
# crates with build scripts). reqwest uses rustls, so no OpenSSL headers are
# required.
FROM rust:1-bookworm AS builder

WORKDIR /app

# Build context is the repo root (see root .dockerignore), so the whole
# workspace — including the path-dependency crates under packages/ — is here.
COPY . .

# --locked: fail instead of silently resolving new versions if Cargo.lock is
# out of date. Only the `{app_name}` package (and its dependency graph) is built.
RUN cargo build --release --package {app_name} --locked

# Stage 2: Slim Debian runtime — same glibc family as the builder so the
# dynamically linked binary runs unchanged. ca-certificates is needed for any
# outbound HTTPS calls (rustls reads the system root store).
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Only the release binary is copied: no source, no target/, no toolchain.
COPY --from=builder /app/target/release/{app_name} /usr/local/bin/{app_name}

# Listen on 80 inside the container; Render terminates TLS in front of it.
ENV PORT=80

EXPOSE 80

CMD ["{app_name}"]
"#
    )
}

/// Render appends its own random suffix to the `.onrender.com` subdomain, so a
/// bare app name like `api` produces an unrecognisable domain (`api-mvqh`).
/// Prefixing the repo name keeps domains identifiable across template clones.
pub fn service_name_for(repo_name: &str, app_name: &str) -> String {
    format!("{repo_name}-{app_name}")
}

/// Static sites have no fixed compute cost (a Docker web service is $7+/mo) and
/// serve from Render's CDN. Render's native build environment has no Rust
/// toolchain, so the build command installs rustup + `dx` before building.
pub fn static_site_entry(app_name: &str, service_name: &str) -> String {
    format!(
        r#"
  - type: web
    name: {service_name}
    runtime: static
    autoDeploy: true
    # Render's native environment has no Rust toolchain: install rustup (minimal
    # profile + wasm target), cargo-binstall, and the Dioxus CLI, then build.
    buildCommand: >-
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs
      | sh -s -- -y --profile minimal --target wasm32-unknown-unknown
      && . "$HOME/.cargo/env"
      && curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
      && cargo binstall -y dioxus-cli
      && cd apps/{app_name}
      && dx bundle --release --out-dir dist
    staticPublishPath: apps/{app_name}/dist/public
    routes:
      # Hybrid pattern: if this app fetches same-origin /api/* implemented by a
      # sibling web service, uncomment and set the destination to that
      # service's real .onrender.com URL (known only after its first deploy).
      # Must come before the /* fallback — routes match in order.
      # - type: rewrite
      #   source: /api/*
      #   destination: https://<repo>-api-<suffix>.onrender.com/api/*
      # SPA fallback: existing files are served directly; everything else
      # rewrites to index.html. Remove for non-SPA (multi-page) sites.
      - type: rewrite
        source: /*
        destination: /index.html
"#
    )
}

/// Docker web service entry; build and run come from the app's Dockerfile.
pub fn docker_service_entry(app_name: &str, service_name: &str) -> String {
    format!(
        r"
  - type: web
    name: {service_name}
    env: docker
    autoDeploy: true
    dockerfilePath: apps/{app_name}/Dockerfile
    healthCheckPath: /health
    envVars:
      # Image-constant env vars (also baked into the Dockerfile; safe to repeat):
      - key: PORT
        value: 80
      # Secrets: declare with `sync: false` and enter the value in the Render
      # dashboard (encrypted, never committed). Example:
      # - key: SOME_API_KEY
      #   sync: false
"
    )
}

/// Append a service entry for `app_name` to the blueprint at `render_yaml_path`,
/// creating the file (with a `services:` header) if it does not exist. Skips
/// when an entry with the same service name is already present.
pub fn append_to_render_yaml(
    app_name: &str,
    repo_name: &str,
    render_yaml_path: &Path,
    is_static: bool,
) -> anyhow::Result<AppendOutcome> {
    let service_name = service_name_for(repo_name, app_name);
    let entry = if is_static {
        static_site_entry(app_name, &service_name)
    } else {
        docker_service_entry(app_name, &service_name)
    };

    let existing = match std::fs::read_to_string(render_yaml_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => "services:\n".to_owned(),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read {}", render_yaml_path.display()));
        }
    };

    if existing.contains(&format!("name: {service_name}")) {
        return Ok(AppendOutcome::AlreadyPresent);
    }

    let updated = format!("{}\n{entry}", existing.trim_end());
    std::fs::write(render_yaml_path, updated)
        .with_context(|| format!("failed to write {}", render_yaml_path.display()))?;
    Ok(AppendOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_name_prefixes_repo_name() {
        assert_eq!(service_name_for("my-repo", "api"), "my-repo-api");
    }

    #[test]
    fn dockerfile_builds_and_runs_the_named_package() {
        let content = dockerfile_content("svc");
        assert!(content.contains("cargo build --release --package svc --locked"));
        assert!(content.contains("/app/target/release/svc /usr/local/bin/svc"));
        assert!(content.contains(r#"CMD ["svc"]"#));
        assert!(content.contains("ENV PORT=80"));
    }

    #[test]
    fn static_entry_has_static_runtime_publish_path_and_spa_fallback() {
        let entry = static_site_entry("web", "repo-web");
        assert!(entry.contains("name: repo-web"));
        assert!(entry.contains("runtime: static"));
        assert!(entry.contains("staticPublishPath: apps/web/dist/public"));
        assert!(entry.contains("dx bundle --release --out-dir dist"));
        assert!(entry.contains("cd apps/web"));
        assert!(entry.contains("destination: /index.html"));
        assert!(entry.contains("# - type: rewrite"));
    }

    #[test]
    fn docker_entry_points_at_dockerfile_and_health_check() {
        let entry = docker_service_entry("api", "repo-api");
        assert!(entry.contains("name: repo-api"));
        assert!(entry.contains("env: docker"));
        assert!(entry.contains("dockerfilePath: apps/api/Dockerfile"));
        assert!(entry.contains("healthCheckPath: /health"));
        assert!(entry.contains("value: 80"));
    }

    #[test]
    fn append_creates_file_then_skips_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("render.yaml");

        let outcome = append_to_render_yaml("api", "repo", &yaml, false).unwrap();
        assert_eq!(outcome, AppendOutcome::Written);
        let first = std::fs::read_to_string(&yaml).unwrap();
        assert!(first.starts_with("services:\n"));
        assert!(first.contains("name: repo-api"));
        assert_eq!(first.matches("name: repo-api").count(), 1);

        let outcome = append_to_render_yaml("api", "repo", &yaml, false).unwrap();
        assert_eq!(outcome, AppendOutcome::AlreadyPresent);
        assert_eq!(std::fs::read_to_string(&yaml).unwrap(), first);

        let outcome = append_to_render_yaml("web", "repo", &yaml, true).unwrap();
        assert_eq!(outcome, AppendOutcome::Written);
        let second = std::fs::read_to_string(&yaml).unwrap();
        assert!(second.starts_with(first.trim_end()));
        assert!(second.contains("name: repo-web"));
        assert!(second.contains("runtime: static"));
    }
}
