//! Running an allowlisted command: no shell, cleared environment, bounded
//! time and bounded output.

use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::config::CommandSpec;

/// Fixed PATH for children. The program itself is an absolute path; this only
/// matters for anything *it* spawns.
const SAFE_PATH: &str = "/usr/local/bin:/usr/bin:/bin";
/// Per-stream capture cap. Output beyond this is drained and discarded so a
/// chatty child cannot block on a full pipe or exhaust our memory.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// How long to wait for pipes to close after the main process exits before
/// assuming a backgrounded grandchild is holding them and killing the group.
const PIPE_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("failed to start {program}: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    #[error("failed waiting for child: {0}")]
    Wait(std::io::Error),
    #[error("child did not exit after SIGKILL (uninterruptible sleep?)")]
    Unkillable,
}

#[derive(Debug, Serialize)]
pub struct ExecOutcome {
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Execute `spec.program` with `argv`. `inherited_env` is the subset of the
/// server's environment the operator chose to pass through.
pub async fn run(
    spec: &CommandSpec,
    argv: &[String],
    inherited_env: &[(String, String)],
) -> Result<ExecOutcome, ExecError> {
    let started = Instant::now();
    let mut cmd = Command::new(&spec.program);
    cmd.args(argv)
        .env_clear()
        .env("PATH", SAFE_PATH)
        .envs(inherited_env.iter().map(|(k, v)| (k, v)))
        .envs(&spec.env)
        .current_dir(
            spec.cwd
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new("/")),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group, so a timeout can kill everything the command
        // started, not just the direct child.
        .process_group(0)
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|source| ExecError::Spawn {
        program: spec.program.display().to_string(),
        source,
    })?;
    let pgid = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .map(Pid::from_raw);
    let kill_group = || {
        if let Some(pgid) = pgid {
            // ESRCH (group already gone) is fine.
            let _ = killpg(pgid, Signal::SIGKILL);
        }
    };

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    // One task joins both streams, so the handle completes exactly once and
    // can safely be re-awaited after a timeout.
    let mut readers =
        tokio::spawn(async move { tokio::join!(read_capped(stdout), read_capped(stderr)) });

    let mut timed_out = false;
    let status = if let Ok(status) =
        tokio::time::timeout(Duration::from_secs(spec.timeout_secs), child.wait()).await
    {
        status.map_err(ExecError::Wait)?
    } else {
        timed_out = true;
        kill_group();
        // The direct child may have left our group (setsid/setpgid), so kill
        // it explicitly too, and never wait unboundedly while holding the slot.
        let _ = child.start_kill();
        match tokio::time::timeout(PIPE_GRACE, child.wait()).await {
            Ok(status) => status.map_err(ExecError::Wait)?,
            Err(_) => return Err(ExecError::Unkillable),
        }
    };

    let joined = if let Ok(r) = tokio::time::timeout(PIPE_GRACE, &mut readers).await {
        r.ok()
    } else {
        // The main process exited but something it backgrounded still holds
        // the pipes open.
        kill_group();
        if let Ok(r) = tokio::time::timeout(PIPE_GRACE, &mut readers).await {
            r.ok()
        } else {
            readers.abort();
            None
        }
    };
    let ((stdout, stdout_truncated), (stderr, stderr_truncated)) =
        joined.unwrap_or_else(|| ((Vec::new(), true), (Vec::new(), true)));

    Ok(ExecOutcome {
        exit_code: status.code(),
        signal: status.signal(),
        timed_out,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        stdout_truncated,
        stderr_truncated,
    })
}

async fn read_capped<R: AsyncRead + Unpin>(mut r: R) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match r.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = MAX_OUTPUT_BYTES.saturating_sub(buf.len());
                buf.extend_from_slice(&chunk[..n.min(room)]);
                truncated |= n > room;
            }
        }
    }
    (buf, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ArgTemplate;
    use std::collections::BTreeMap;

    fn spec(program: &str, timeout_secs: u64) -> CommandSpec {
        CommandSpec {
            id: "t".into(),
            description: String::new(),
            program: program.into(),
            args: Vec::<ArgTemplate>::new(),
            params: Vec::new(),
            timeout_secs,
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn shell_metacharacters_are_inert() {
        // If a shell were involved, `$(id)` would expand and `;` would chain.
        let argv = vec!["$(id);".to_owned(), "`whoami`|cat".to_owned()];
        let out = run(&spec("/bin/echo", 5), &argv, &[]).await.unwrap();
        assert_eq!(out.stdout, "$(id); `whoami`|cat\n");
        assert_eq!(out.exit_code, Some(0));
    }

    #[tokio::test]
    async fn environment_is_cleared() {
        // SAFETY of test assumption: the test process has *some* env var set.
        let out = run(&spec("/usr/bin/env", 5), &[], &[]).await.unwrap();
        assert_eq!(out.stdout.trim(), format!("PATH={SAFE_PATH}"));
    }

    #[tokio::test]
    async fn timeout_kills_the_process_group() {
        let out = run(&spec("/bin/sleep", 1), &["30".to_owned()], &[])
            .await
            .unwrap();
        assert!(out.timed_out);
        assert_eq!(out.signal, Some(9));
        assert!(out.duration_ms < 10_000);
    }

    #[tokio::test]
    async fn output_is_capped() {
        let out = run(
            &spec("/usr/bin/head", 5),
            &["-c".into(), "200000".into(), "/dev/zero".into()],
            &[],
        )
        .await
        .unwrap();
        assert_eq!(out.stdout.len(), MAX_OUTPUT_BYTES);
        assert!(out.stdout_truncated);
    }
}
