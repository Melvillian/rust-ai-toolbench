//! Append-only JSONL record of every execution attempt that passed auth.

use std::path::PathBuf;

use serde::Serialize;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct AuditLog {
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AuditEntry<'a> {
    pub at: String,
    pub login: Option<&'a str>,
    pub command: &'a str,
    pub argv: &'a [String],
    pub outcome: &'a str,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

impl AuditLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Best effort: a failed audit write is logged loudly but does not fail
    /// the request (the command has already run by then).
    pub async fn append(&self, entry: &AuditEntry<'_>) {
        let Ok(mut line) = serde_json::to_vec(entry) else {
            return;
        };
        line.push(b'\n');
        let res = async {
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(&self.path)
                .await?;
            f.write_all(&line).await?;
            f.flush().await
        }
        .await;
        if let Err(e) = res {
            tracing::error!("audit log write to {} failed: {e}", self.path.display());
        }
    }
}
