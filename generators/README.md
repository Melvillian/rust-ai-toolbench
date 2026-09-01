# Project Generators

This directory contains project generators for quickly scaffolding common project patterns.

## Usage

`generators` is a workspace member, so it builds on demand with `cargo run`:

```bash
cargo run -p generators -- <generator-name> [options]
```

The expected way to use the `render-deploy` generator is via Claude Code:

```claude
/generate-render-deploy <app-name>
```

## Available Generators

### render-deploy

Adds Render deployment files (Dockerfile) and updates `render.yaml` for an app in `apps/`.

```bash
cargo run -p generators -- render-deploy <app-name>            # Docker web service
cargo run -p generators -- render-deploy <app-name> --static   # static site
```

This creates:

- A Dockerfile for the app under `apps/<app-name>` (web service only)
- A service entry in the root `render.yaml`, named `<repository-name>-<app-name>`

After generating, create a Blueprint in the Render dashboard pointing to `render.yaml` (one-time setup), then commit and push to main to deploy.

## Adding New Generators

1. Create a new module in `src/commands/` with a clap `Args` struct and a `run` function
2. Register it in `src/commands/mod.rs`
3. Add a variant to the `Command` enum in `src/main.rs`
