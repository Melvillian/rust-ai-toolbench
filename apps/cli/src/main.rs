//! `cli` — command-line tool built with clap.
//!
//! Subcommands live in `src/commands/`; add a new module there and a variant
//! to [`commands::Commands`].

mod commands;

use clap::Parser;

use commands::Commands;

/// Command-line tool for the rust-ai-toolbench workspace.
#[derive(Debug, Parser)]
#[command(name = "cli", version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `.env` is optional; real environment variables win.
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    match cli.command {
        Commands::Hello => commands::hello::run(&mut std::io::stdout())?,
        Commands::Summarize(args) => commands::summarize::run(args).await?,
    }
    Ok(())
}
