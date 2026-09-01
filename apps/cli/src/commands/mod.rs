//! Subcommand definitions. Each submodule owns its clap args and a `run` fn.

pub mod hello;
pub mod summarize;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Hello world command
    Hello,
    /// Summarize text with OpenAI (reads stdin when TEXT is omitted)
    Summarize(summarize::SummarizeArgs),
}
