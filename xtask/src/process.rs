//! Spawning child processes: echo the command line, run to completion or run
//! several concurrently with Ctrl-C forwarding and fail-fast semantics.

use std::path::Path;
use std::process::ExitStatus;

use anyhow::{Context, bail};
use tokio::process::{Child, Command};

/// A command line to run, with an optional working directory and env vars.
#[derive(Debug, Clone)]
pub struct Spec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<std::path::PathBuf>,
    pub env: Vec<(String, String)>,
}

impl Spec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            env: Vec::new(),
        }
    }

    #[must_use]
    pub fn cwd(mut self, dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(dir.as_ref().to_path_buf());
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Shell-ish rendering for the `$ …` echo line.
    pub fn display(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.iter().map(|arg| {
            if arg.contains(' ') {
                format!("'{arg}'")
            } else {
                arg.clone()
            }
        }));
        parts.join(" ")
    }

    fn spawn(&self) -> anyhow::Result<Child> {
        eprintln!("$ {}", self.display());
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        if let Some(dir) = &self.cwd {
            cmd.current_dir(dir);
        }
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        cmd.kill_on_drop(true);
        cmd.spawn()
            .with_context(|| format!("failed to spawn `{}`", self.display()))
    }
}

fn check(spec: &Spec, status: ExitStatus) -> anyhow::Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("`{}` exited with {status}", spec.display())
    }
}

/// Run one command to completion; error on non-zero exit.
pub async fn run(spec: &Spec) -> anyhow::Result<()> {
    let mut child = spec.spawn()?;
    let status = child.wait().await?;
    check(spec, status)
}

/// Run each command in order, stopping at the first failure.
pub async fn run_all(specs: &[Spec]) -> anyhow::Result<()> {
    for spec in specs {
        run(spec).await?;
    }
    Ok(())
}

/// Run several long-lived commands concurrently. Ctrl-C stops all of them;
/// if any exits on its own, the rest are killed and its status is reported.
pub async fn run_concurrently(specs: &[Spec]) -> anyhow::Result<()> {
    let mut children: Vec<(usize, Child)> = specs
        .iter()
        .enumerate()
        .map(|(i, spec)| spec.spawn().map(|child| (i, child)))
        .collect::<Result<_, _>>()?;

    let outcome = {
        let mut waits = children
            .iter_mut()
            .map(|(i, child)| {
                let i = *i;
                Box::pin(async move { (i, child.wait().await) })
            })
            .collect::<Vec<_>>();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => None,
            finished = select_first(&mut waits) => Some(finished),
        }
    };

    for (_, child) in &mut children {
        let _ = child.start_kill();
    }
    for (_, child) in &mut children {
        let _ = child.wait().await;
    }

    match outcome {
        None => {
            eprintln!("interrupted; stopped all processes");
            Ok(())
        }
        Some((i, status)) => {
            let status = status?;
            check(&specs[i], status)
                .map(|()| eprintln!("`{}` exited; stopped the others", specs[i].display()))
        }
    }
}

/// Resolve with the output of whichever future completes first (a minimal
/// `select_all`, so the `futures` crate is not needed).
async fn select_first<F, T>(futs: &mut [std::pin::Pin<Box<F>>]) -> T
where
    F: std::future::Future<Output = T> + ?Sized,
{
    std::future::poll_fn(|cx| {
        for fut in futs.iter_mut() {
            if let std::task::Poll::Ready(out) = fut.as_mut().poll(cx) {
                return std::task::Poll::Ready(out);
            }
        }
        std::task::Poll::Pending
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_quotes_args_with_spaces() {
        let spec = Spec::new("cargo", ["watch", "-x", "run -p api"]);
        assert_eq!(spec.display(), "cargo watch -x 'run -p api'");
    }

    #[tokio::test]
    async fn run_reports_non_zero_exit() {
        let ok = Spec::new("true", Vec::<String>::new());
        run(&ok).await.unwrap();
        let bad = Spec::new("false", Vec::<String>::new());
        let err = run(&bad).await.unwrap_err();
        assert!(err.to_string().contains("exited with"), "{err}");
    }

    #[tokio::test]
    async fn concurrent_stops_when_one_child_exits() {
        let specs = [
            Spec::new("sleep", ["30"]),
            Spec::new("false", Vec::<String>::new()),
        ];
        let started = std::time::Instant::now();
        let err = run_concurrently(&specs).await.unwrap_err();
        assert!(err.to_string().contains("false"), "{err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}
