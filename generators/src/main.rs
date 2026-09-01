use clap::{Parser, Subcommand};
use generators::commands::render_deploy::{self, RenderDeployArgs};

/// Project generators for scaffolding common project patterns.
#[derive(Debug, Parser)]
#[command(name = "generators", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Add Render deployment config for an app in apps/: a static site entry in
    /// render.yaml (--static), or a Dockerfile plus a Docker web service entry
    /// (default)
    RenderDeploy(RenderDeployArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::RenderDeploy(args) => render_deploy::run(&args),
    }
}
