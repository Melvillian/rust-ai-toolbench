# Render Web Service

This project sets up an axum (Rust) server deployed to Render via Docker with Continuous Deployment.

- Runs as a Render web service with a public URL
- Deploys automatically on push to `main`
- Listens on port 80 (HTTP) in production (the Docker image sets `PORT=80`); defaults to 8080 locally
- Render handles SSL termination

## Architecture

```
Internet (HTTPS) → Render SSL Termination → Docker Container (HTTP:80) → axum server (Rust)
```

## Prerequisites

1. A Render account with a Blueprint connected to this repo's `render.yaml`

## Setup Instructions

### 1. One-time Blueprint Setup

Create a Blueprint in the Render dashboard pointing to `render.yaml` at the repo root:
https://dashboard.render.com/select-repo?type=blueprint

### 2. Continuous Deployment

After Blueprint setup, just push to `main`:

```bash
git push origin main
```

Render detects the push, builds the Docker image, and deploys automatically.

### 3. Verify Deployment

Once deployed:

```bash
curl https://api.onrender.com/health
```

## API Endpoints

- `GET /health` - Health check (`{ "status": "ok", "timestamp": "<RFC 3339>" }`)
- `GET /api/hello` - Example API endpoint
- `GET /api/info` - Service name, version and endpoint list
- Any other `/api` or `/api/*` request (including wrong methods on the routes above) - `404 { "error": "not found" }`
- Everything else, including `GET /` - plain `404`. The web app (`apps/web`) is a separate Render static site, not served here.

## Local Development

```bash
# Run the server (reads apps/api/.env if present; defaults to port 8080)
cargo run -p api

# Or, from the repo root, the xtask equivalent
cargo xtask dev-api

# Run the route tests
cargo test -p api
```

## Environment Variables

| Variable   | Description                                  | Required                                 |
| ---------- | -------------------------------------------- | ---------------------------------------- |
| `PORT`     | Port to listen on                            | No (default: 8080; Docker image sets 80) |
| `RUST_LOG` | `tracing` filter, e.g. `debug` or `api=trace` | No (default: `info`)                     |

## How It Works

1. **Dockerfile**: Multi-stage build — compiles a release binary in `rust:1-bookworm`, then copies only that binary into `debian:bookworm-slim`
2. **render.yaml**: Declares the web service with Docker environment and auto-deploy
3. **Continuous Deployment**: Render watches the `main` branch and deploys on every push

## Troubleshooting

### Service won't start

- Check Render logs for errors
- Ensure your server binds to `0.0.0.0` not `localhost` (`src/main.rs` does)
- Check that the `PORT` environment variable matches your server configuration

### Docker build fails with "lock file needs to be updated"

The image builds with `cargo build --locked`. Run `cargo build` locally and commit the updated `Cargo.lock`.

## Customization

1. Edit `src/app.rs` to add your endpoints (and a test for each in the same file)
2. Update `render.yaml` for Render-specific configuration
3. Modify `Dockerfile` if you need additional runtime dependencies

## License

MIT
