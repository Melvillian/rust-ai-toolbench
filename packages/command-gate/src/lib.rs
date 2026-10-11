//! Passkey-gated execution of an allowlist of commands, meant to be reached
//! from a phone over Tailscale.
//!
//! Layers, outermost first:
//! - **Network:** the server binds loopback only; `tailscale serve` terminates
//!   HTTPS on the tailnet and injects `Tailscale-User-Login`, which must name
//!   an allowed login ([`server`]).
//! - **Authentication:** `WebAuthn` passkeys with user verification, enrolled
//!   only via a one-time token printed on the host ([`auth`]).
//! - **Authorisation:** only commands in the TOML allowlist, each with typed
//!   parameters ([`config`], [`args`]).
//! - **Execution:** direct `execve` (no shell), cleared environment, timeout,
//!   output cap, one command at a time, audit log ([`exec`], [`audit`]).

pub mod args;
pub mod audit;
pub mod auth;
pub mod config;
pub mod exec;
pub mod server;

use std::future::Future;
use std::path::PathBuf;

pub use config::{Config, Mode};

#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Auth(#[from] auth::AuthError),
    #[error(
        "refusing to run as root: every allowlisted command would run as root too. Use a dedicated user."
    )]
    Root,
    #[error("server I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub config_path: PathBuf,
    pub mode: Mode,
    /// Open a one-time passkey enrollment window at startup. Also opened
    /// automatically when no passkey is enrolled yet.
    pub enroll: bool,
}

/// Load config, run startup safety checks, and serve until `shutdown`
/// resolves.
pub async fn serve(
    opts: ServeOptions,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), GateError> {
    if nix::unistd::geteuid().is_root() {
        return Err(GateError::Root);
    }
    let config = Config::load(&opts.config_path, opts.mode)?;
    let auth = auth::Auth::open(&config.server)?;

    if opts.mode == Mode::Dev {
        tracing::warn!(
            "DEV MODE: Tailscale identity checks are OFF. Loopback only; never expose this port."
        );
    }
    // Under systemd, stderr goes to the journal; never auto-print a token
    // there. Explicit --enroll is assumed to be an interactive run.
    let under_systemd = std::env::var_os("JOURNAL_STREAM").is_some();
    if auth.passkey_count() == 0 && !opts.enroll && under_systemd {
        tracing::warn!(
            "no passkeys enrolled; stop the service and run interactively with --enroll"
        );
    }
    if opts.enroll || (auth.passkey_count() == 0 && !under_systemd) {
        let token = auth.open_enrollment(server::ENROLLMENT_TTL);
        // Printed to the controlling terminal, not the structured log, and
        // carried in the URL *fragment* so it never reaches server or proxy
        // logs. Single use, expires after ENROLLMENT_TTL.
        eprintln!(
            "\n  Passkey enrollment is open for {} minutes (single use).\n  Open this on the device to enroll:\n\n    {}#enroll={token}\n",
            server::ENROLLMENT_TTL.as_secs() / 60,
            config.server.origin_header
        );
    }

    let listener = tokio::net::TcpListener::bind(config.server.bind).await?;
    tracing::info!(
        bind = %config.server.bind,
        origin = %config.server.origin_header,
        commands = config.commands.len(),
        passkeys = auth.passkey_count(),
        "command-gate listening"
    );
    let app = server::router(server::AppState::new(config, auth));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}
