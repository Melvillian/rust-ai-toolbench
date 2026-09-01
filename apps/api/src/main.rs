//! Binary entrypoint: loads `.env`, initialises tracing, and serves the router
//! from [`app::create_app`] on `0.0.0.0:$PORT` (default 8080).

mod app;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_PORT: u16 = 8080;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Real environment variables win over `.env` (dotenvy never overrides).
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let port = match common_lib::get_env("PORT") {
        Some(raw) => raw
            .parse::<u16>()
            .map_err(|e| anyhow::anyhow!("invalid PORT {raw:?}: {e}"))?,
        None => DEFAULT_PORT,
    };

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("Server listening on port {port}");

    axum::serve(listener, app::create_app())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::error!("failed to listen for ctrl-c: {e}");
    }
    tracing::info!("shutdown signal received");
}
