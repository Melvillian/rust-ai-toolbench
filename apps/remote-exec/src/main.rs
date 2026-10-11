//! Binary entrypoint: parses flags, initialises tracing, and hands off to
//! [`command_gate::serve`]. All logic lives in `packages/command-gate`.

use std::path::PathBuf;

use clap::Parser;
use command_gate::{Mode, ServeOptions};
use tracing_subscriber::EnvFilter;

/// Run allowlisted commands on this machine from a phone, behind Tailscale
/// and passkeys.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to the allowlist/server TOML (see apps/remote-exec/commands.example.toml).
    #[arg(short, long, env = "REMOTE_EXEC_CONFIG")]
    config: PathBuf,

    /// Open a single-use, 15-minute passkey enrollment window and print its link.
    /// Opens automatically when no passkey is enrolled yet.
    #[arg(long)]
    enroll: bool,

    /// Local development on `http://localhost`: disables the Tailscale identity check.
    #[arg(long)]
    dev: bool,

    /// Validate the config and exit.
    #[arg(long)]
    check: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let mode = if cli.dev { Mode::Dev } else { Mode::Tailscale };

    if cli.check {
        let cfg = command_gate::Config::load(&cli.config, mode)?;
        tracing::info!("config OK: {} command(s)", cfg.commands.len());
        return Ok(());
    }

    command_gate::serve(
        ServeOptions {
            config_path: cli.config,
            mode,
            enroll: cli.enroll,
        },
        shutdown_signal(),
    )
    .await?;
    Ok(())
}

/// Ctrl-C interactively, SIGTERM under systemd.
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let term = async {
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::error!("failed to install SIGTERM handler: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = term => {}
    }
    tracing::info!("shutdown signal received");
}
