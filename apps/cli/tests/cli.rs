//! End-to-end tests that run the compiled `cli` binary.

use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cli"))
}

#[test]
fn hello_prints_greeting() {
    let out = cli().arg("hello").output().expect("run cli");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Hello world!\n");
}

#[test]
fn version_prints_crate_version() {
    let out = cli().arg("--version").output().expect("run cli");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("cli {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn no_args_exits_nonzero_with_usage() {
    let out = cli().output().expect("run cli");
    assert!(!out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("Usage:"),
        "expected usage text, got: {combined}"
    );
    assert!(combined.contains("hello"));
    assert!(combined.contains("summarize"));
}

#[test]
fn summarize_without_api_key_fails_cleanly() {
    // Without a key the library must fail before any network call; the CLI
    // reports `Error: ...` on stderr (anyhow) and exits 1.
    let out = cli()
        .env_remove("OPENAI_API_KEY")
        .args(["summarize", "some text"])
        .output()
        .expect("run cli");
    assert!(!out.status.success());
    assert!(!out.stderr.is_empty());
}
