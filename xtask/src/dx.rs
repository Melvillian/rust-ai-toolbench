//! Locating the Dioxus CLI. Other tools also install a `dx` binary (Deno's
//! `deno x` shim, for one), so the lookup verifies `--version` before trusting
//! whatever is first on `PATH`.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// Install hint printed when no usable `dx` is found.
pub const INSTALL_HINT: &str = "install the Dioxus CLI with `cargo install dioxus-cli --locked` (or `cargo binstall dioxus-cli`), \
     or point the DX environment variable at the binary";

/// Candidate locations, in priority order: `$DX`, `~/.cargo/bin/dx`, then
/// plain `dx` on `PATH`.
pub fn candidates(env_dx: Option<&str>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(explicit) = env_dx.map(str::trim).filter(|s| !s.is_empty()) {
        out.push(PathBuf::from(explicit));
    }
    if let Some(home) = home {
        out.push(home.join(".cargo").join("bin").join("dx"));
    }
    out.push(PathBuf::from("dx"));
    out
}

/// True when `--version` output identifies the Dioxus CLI.
pub fn is_dioxus_version_output(output: &str) -> bool {
    output.to_ascii_lowercase().contains("dioxus")
}

/// Pick the first candidate for which `probe` reports Dioxus version output.
pub fn select(
    candidates: &[PathBuf],
    mut probe: impl FnMut(&Path) -> Option<String>,
) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| probe(candidate).is_some_and(|out| is_dioxus_version_output(&out)))
        .cloned()
}

/// Locate the Dioxus CLI on this machine.
pub fn locate() -> anyhow::Result<PathBuf> {
    let env_dx = std::env::var("DX").ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let candidates = candidates(env_dx.as_deref(), home.as_deref());
    let found = select(&candidates, |path| {
        std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    });
    match found {
        Some(path) => Ok(path),
        None => bail!("could not find the Dioxus CLI (`dx`); {INSTALL_HINT}"),
    }
}

/// Convenience: `locate()` with a friendlier error context.
pub fn require() -> anyhow::Result<PathBuf> {
    locate().context("the Dioxus CLI is required for this task")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_prefer_env_then_cargo_home_then_path() {
        let c = candidates(Some("/opt/dx"), Some(Path::new("/home/me")));
        assert_eq!(
            c,
            vec![
                PathBuf::from("/opt/dx"),
                PathBuf::from("/home/me/.cargo/bin/dx"),
                PathBuf::from("dx"),
            ]
        );
        assert_eq!(candidates(Some("  "), None), vec![PathBuf::from("dx")]);
    }

    #[test]
    fn select_skips_non_dioxus_binaries() {
        let c = candidates(None, Some(Path::new("/h")));
        let picked = select(&c, |path| {
            if path == Path::new("/h/.cargo/bin/dx") {
                Some("dioxus 0.7.10 (abc)".into())
            } else {
                Some("deno x ...".into())
            }
        });
        assert_eq!(picked, Some(PathBuf::from("/h/.cargo/bin/dx")));

        let none = select(&c, |_| None);
        assert_eq!(none, None);
    }

    #[test]
    fn version_detection_is_case_insensitive() {
        assert!(is_dioxus_version_output("Dioxus 0.7.10"));
        assert!(!is_dioxus_version_output("deno 2.0"));
    }
}
