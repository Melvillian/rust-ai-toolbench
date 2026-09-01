//! `cargo xtask <command>` — the Rust analogue of the TS repo's root
//! `package.json` scripts. Pure cargo tasks are aliases in
//! `.cargo/config.toml`; anything that spawns several processes lives here.

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use xtask::dx;
use xtask::preview;
use xtask::process::{Spec, run, run_all, run_concurrently};

const API_IMAGE: &str = "rust-ai-toolbench-api";
const WEB_DEV_PORT: &str = "5173";

#[derive(Debug, Parser)]
#[command(name = "cargo xtask", version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Build every workspace member, then bundle the web app into apps/web/dist (`dx bundle --release`)
    Build,
    /// Run the api (auto-reloading if cargo-watch is installed) and `dx serve` together
    Dev,
    /// Run only the api server (:8080)
    DevApi,
    /// Run only the Dioxus dev server (:5173, proxies /api to :8080)
    DevWeb {
        /// Extra arguments forwarded to `dx serve`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run the CLI app
    DevCli {
        /// Arguments forwarded to the cli binary
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build everything, then run the full stack: api (:8080) + web preview (:4173)
    Start,
    /// Serve the production web build with an /api proxy (the `vite preview` stand-in)
    Preview {
        #[arg(long, default_value_t = preview::DEFAULT_PORT)]
        port: u16,
        /// Directory containing the `dx bundle` output (the static site root)
        #[arg(long, default_value = "apps/web/dist/public")]
        dir: PathBuf,
        /// Origin to proxy /api and /health to
        #[arg(long, default_value = preview::DEFAULT_API)]
        api: String,
    },
    /// Build the api Docker image
    DockerBuildApi,
    /// Run the api Docker image on port 8080
    DockerStartApi,
    /// clippy (-D warnings) + rustfmt --check, without modifying files
    LintCheck,
    /// clippy --fix + rustfmt
    Lint,
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask has no parent directory")?
        .canonicalize()
        .context("failed to canonicalise workspace root")
}

fn cargo(args: &[&str]) -> Spec {
    Spec::new("cargo", args.iter().copied())
}

fn dx_spec(root: &Path, args: &[&str]) -> anyhow::Result<Spec> {
    let dx = dx::require()?;
    Ok(Spec::new(dx.to_string_lossy(), args.iter().copied()).cwd(root.join("apps/web")))
}

fn cargo_watch_available() -> bool {
    std::process::Command::new("cargo")
        .args(["watch", "--version"])
        .output()
        .is_ok_and(|out| out.status.success())
}

fn dev_api_spec() -> Spec {
    if cargo_watch_available() {
        cargo(&["watch", "-x", "run -p api"])
    } else {
        eprintln!("hint: `cargo install cargo-watch` to auto-reload the api on save");
        cargo(&["run", "-p", "api"])
    }
}

fn dev_web_spec(root: &Path, extra: &[String]) -> anyhow::Result<Spec> {
    let mut args = vec!["serve", "--port", WEB_DEV_PORT];
    args.extend(extra.iter().map(String::as_str));
    dx_spec(root, &args)
}

fn build_specs(root: &Path) -> anyhow::Result<Vec<Spec>> {
    Ok(vec![
        cargo(&["build", "--workspace"]),
        dx_spec(root, &["bundle", "--release", "--out-dir", "dist"])?,
    ])
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    std::env::set_current_dir(&root)?;

    match cli.command {
        Task::Build => run_all(&build_specs(&root)?).await,
        Task::Dev => run_concurrently(&[dev_api_spec(), dev_web_spec(&root, &[])?]).await,
        Task::DevApi => run(&dev_api_spec()).await,
        Task::DevWeb { args } => run(&dev_web_spec(&root, &args)?).await,
        Task::DevCli { args } => {
            let mut all = vec!["run", "-p", "cli", "--"];
            all.extend(args.iter().map(String::as_str));
            run(&cargo(&all)).await
        }
        Task::Start => {
            run_all(&build_specs(&root)?).await?;
            let api = cargo(&["run", "--release", "-p", "api"]).env("PORT", "8080");
            let preview = Spec::new(std::env::current_exe()?.to_string_lossy(), ["preview"]);
            run_concurrently(&[api, preview]).await
        }
        Task::Preview { port, dir, api } => preview::serve(root.join(dir), port, &api).await,
        Task::DockerBuildApi => {
            run(&Spec::new(
                "docker",
                ["build", "-t", API_IMAGE, "-f", "apps/api/Dockerfile", "."],
            ))
            .await
        }
        Task::DockerStartApi => {
            run(&Spec::new(
                "docker",
                ["run", "--rm", "--init", "-p", "8080:80", API_IMAGE],
            ))
            .await
        }
        Task::LintCheck => {
            run_all(&[
                cargo(&[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ]),
                cargo(&["fmt", "--all", "--", "--check"]),
            ])
            .await
        }
        Task::Lint => {
            run_all(&[
                cargo(&[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--fix",
                    "--allow-dirty",
                    "--allow-staged",
                    "--",
                    "-D",
                    "warnings",
                ]),
                cargo(&["fmt", "--all"]),
            ])
            .await
        }
    }
}
