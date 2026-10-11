//! Turning untrusted, user-supplied argument values into an argv.
//!
//! Defence in depth, outermost first:
//!
//! 1. **No shell.** The argv is handed straight to `execve` (see `exec`), so
//!    `;`, `|`, `$( )`, backticks, globs and quotes have no meaning: shell
//!    injection is structurally impossible rather than filtered.
//! 2. **Whole-element substitution.** A value fills exactly one argv slot; it
//!    can never split into two arguments or merge into a literal.
//! 3. **Typed allowlists.** Every value must satisfy its param's kind. None of
//!    the free-form kinds can produce a value starting with `-` (option
//!    injection, e.g. `--output=/etc/passwd` or `-exec`; integer params must
//!    have `min >= 0`), and `path` values are canonicalised and confined to
//!    their root.
//!
//! What this cannot protect against is a program that itself interprets its
//! arguments as code (`bash -c {x}`, `python -c`, `find … -exec`, `git -c
//! core.sshCommand=…`). Allowlist programs, not interpreters.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use crate::config::{ArgTemplate, CommandSpec, ParamKind};

/// Hard ceiling on any single submitted value, before kind-specific limits.
const MAX_RAW_LEN: usize = 4096;
const MAX_PATH_DEPTH: usize = 16;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArgError {
    #[error("unknown argument {0:?}")]
    Unknown(String),
    #[error("missing argument {0:?}")]
    Missing(String),
    #[error("argument {name:?}: {reason}")]
    Invalid { name: String, reason: String },
}

/// Validate `provided` against `spec` and build the final argv (excluding the
/// program itself).
pub fn render_argv(
    spec: &CommandSpec,
    provided: &BTreeMap<String, String>,
) -> Result<Vec<String>, ArgError> {
    if let Some(extra) = provided
        .keys()
        .find(|k| !spec.params.iter().any(|p| &p.name == *k))
    {
        return Err(ArgError::Unknown(extra.clone()));
    }
    let mut values = BTreeMap::new();
    for param in &spec.params {
        let raw = provided
            .get(&param.name)
            .ok_or_else(|| ArgError::Missing(param.name.clone()))?;
        let value = validate(&param.kind, raw).map_err(|reason| ArgError::Invalid {
            name: param.name.clone(),
            reason,
        })?;
        values.insert(param.name.as_str(), value);
    }
    Ok(spec
        .args
        .iter()
        .map(|a| match a {
            ArgTemplate::Literal(s) => s.clone(),
            ArgTemplate::Param(name) => values[name.as_str()].clone(),
        })
        .collect())
}

/// Validate one value against its kind, returning the canonical form to pass.
pub fn validate(kind: &ParamKind, raw: &str) -> Result<String, String> {
    if raw.len() > MAX_RAW_LEN {
        return Err("value too long".into());
    }
    if raw.chars().any(char::is_control) {
        return Err("control characters are not allowed".into());
    }
    match kind {
        ParamKind::Enum { values } => values
            .iter()
            .find(|v| v.as_str() == raw)
            .cloned()
            .ok_or_else(|| "not one of the allowed values".into()),
        ParamKind::Integer { min, max } => {
            let digits = raw.strip_prefix('-').unwrap_or(raw);
            // Strict decimal only: no '+', whitespace, '0x', '1e3', '٣' etc.
            if digits.is_empty() || digits.len() > 19 || !digits.bytes().all(|b| b.is_ascii_digit())
            {
                return Err("must be a base-10 integer".into());
            }
            let n: i64 = raw.parse().map_err(|_| "integer out of range".to_owned())?;
            if n < *min || n > *max {
                return Err(format!("must be between {min} and {max}"));
            }
            // Canonical form: "007" -> "7", "-0" -> "0".
            Ok(n.to_string())
        }
        ParamKind::Word { max_len } => {
            if raw.len() > *max_len {
                return Err(format!("must be at most {max_len} characters"));
            }
            check_word(raw)?;
            Ok(raw.to_owned())
        }
        ParamKind::Path { root } => resolve_under(root, raw),
    }
}

/// `[A-Za-z0-9_][A-Za-z0-9._-]*` — ASCII only, so no Unicode lookalikes,
/// bidi overrides or normalisation games; never begins with `-` or `.`.
fn check_word(s: &str) -> Result<(), String> {
    let mut bytes = s.bytes();
    match bytes.next() {
        None => return Err("must not be empty".into()),
        Some(b) if b.is_ascii_alphanumeric() || b == b'_' => {}
        Some(_) => return Err("must start with a letter, digit or underscore".into()),
    }
    if bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-')) {
        Ok(())
    } else {
        Err("only letters, digits, '_', '.', '-' are allowed".into())
    }
}

fn resolve_under(root: &Path, raw: &str) -> Result<String, String> {
    let rel = Path::new(raw);
    if rel.is_absolute() {
        return Err("must be relative to the configured root".into());
    }
    let mut depth = 0;
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => {
                let part = part.to_str().ok_or("path must be UTF-8")?;
                check_word(part).map_err(|e| format!("path component {part:?}: {e}"))?;
                depth += 1;
            }
            Component::CurDir => {}
            // ParentDir is the obvious escape; Prefix/RootDir cannot occur for
            // a relative unix path but are rejected rather than assumed away.
            _ => return Err("'..' and absolute components are not allowed".into()),
        }
    }
    if depth == 0 {
        return Err("must name something inside the configured root".into());
    }
    if depth > MAX_PATH_DEPTH {
        return Err("path is too deep".into());
    }
    // canonicalize resolves symlinks, so a symlink inside root pointing at
    // /etc is caught by the containment check below.
    let resolved = root
        .join(rel)
        .canonicalize()
        .map_err(|_| "path does not exist".to_owned())?;
    // Re-check *after* symlink resolution: a link inside root may point at
    // root itself (`self -> .`) or at a dot-directory (`x -> .git`).
    let inner = resolved
        .strip_prefix(root)
        .map_err(|_| "path escapes the configured root".to_owned())?;
    if inner.as_os_str().is_empty() {
        return Err("must name something inside the configured root".into());
    }
    for comp in inner.components() {
        let part = comp.as_os_str().to_str().ok_or("path must be UTF-8")?;
        check_word(part).map_err(|e| format!("resolved path component {part:?}: {e}"))?;
    }
    resolved
        .into_os_string()
        .into_string()
        .map_err(|_| "resolved path is not UTF-8".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParamSpec;
    use std::path::PathBuf;

    fn word() -> ParamKind {
        ParamKind::Word { max_len: 64 }
    }

    #[test]
    fn word_accepts_plain_tokens() {
        for ok in ["nginx", "my-service.timer", "_x", "A1.2-3_b", "v1"] {
            assert_eq!(validate(&word(), ok).unwrap(), ok);
        }
    }

    #[test]
    fn word_rejects_injection_attempts() {
        for bad in [
            "",
            "-rf",
            "--output=/etc/passwd",
            ".",
            "..",
            ".hidden",
            "a b",
            "a;reboot",
            "a|b",
            "$(id)",
            "`id`",
            "a&&b",
            "a\nb",
            "a\0b",
            "*",
            "a/b",
            "~root",
            "caf\u{e9}",
            "\u{202e}txt",
            "x'y",
            "x\"y",
            "{a}",
        ] {
            assert!(validate(&word(), bad).is_err(), "accepted {bad:?}");
        }
        assert!(validate(&ParamKind::Word { max_len: 3 }, "abcd").is_err());
    }

    #[test]
    fn integer_is_strict_and_bounded() {
        let k = ParamKind::Integer { min: 0, max: 100 };
        assert_eq!(validate(&k, "42").unwrap(), "42");
        assert_eq!(validate(&k, "007").unwrap(), "7");
        for bad in [
            "",
            "-1",
            "101",
            "+5",
            " 5",
            "5 ",
            "0x10",
            "1e2",
            "4.2",
            "١",
            "99999999999999999999",
        ] {
            assert!(validate(&k, bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn enum_is_exact_match() {
        let k = ParamKind::Enum {
            values: vec!["nanoclaw".into(), "notion-executor".into()],
        };
        assert_eq!(validate(&k, "nanoclaw").unwrap(), "nanoclaw");
        for bad in ["Nanoclaw", "nanoclaw ", "nano", "nanoclaw;reboot"] {
            assert!(validate(&k, bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn path_is_confined_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("logs")).unwrap();
        std::fs::write(root.join("logs/app.log"), "x").unwrap();
        std::os::unix::fs::symlink("/etc", root.join("escape")).unwrap();
        std::os::unix::fs::symlink(".", root.join("self")).unwrap();
        std::fs::create_dir(root.join(".secret")).unwrap();
        std::os::unix::fs::symlink(".secret", root.join("dotlink")).unwrap();
        let k = ParamKind::Path { root: root.clone() };

        let ok = validate(&k, "logs/app.log").unwrap();
        assert_eq!(PathBuf::from(ok), root.join("logs/app.log"));
        assert!(validate(&k, "./logs").is_ok());

        for bad in [
            "../etc/passwd",
            "logs/../../etc",
            "/etc/passwd",
            "escape/passwd",
            "escape",
            "missing",
            "-x",
            "logs/.git",
            "",
            ".",
            "./",
            "self",
            "dotlink",
        ] {
            assert!(validate(&k, bad).is_err(), "accepted {bad:?}");
        }
    }

    fn spec() -> CommandSpec {
        CommandSpec {
            id: "t".into(),
            description: String::new(),
            program: "/bin/echo".into(),
            args: vec![
                ArgTemplate::Literal("-n".into()),
                ArgTemplate::Param("count".into()),
                ArgTemplate::Param("name".into()),
            ],
            params: vec![
                ParamSpec {
                    name: "count".into(),
                    description: String::new(),
                    kind: ParamKind::Integer { min: 1, max: 10 },
                },
                ParamSpec {
                    name: "name".into(),
                    description: String::new(),
                    kind: word(),
                },
            ],
            timeout_secs: 5,
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn render_substitutes_whole_elements() {
        let args = BTreeMap::from([
            ("count".to_owned(), "3".to_owned()),
            ("name".to_owned(), "svc".to_owned()),
        ]);
        assert_eq!(render_argv(&spec(), &args).unwrap(), vec!["-n", "3", "svc"]);
    }

    #[test]
    fn render_rejects_missing_and_unknown() {
        let missing = BTreeMap::from([("count".to_owned(), "3".to_owned())]);
        assert_eq!(
            render_argv(&spec(), &missing),
            Err(ArgError::Missing("name".into()))
        );
        let extra = BTreeMap::from([
            ("count".to_owned(), "3".to_owned()),
            ("name".to_owned(), "svc".to_owned()),
            ("evil".to_owned(), "x".to_owned()),
        ]);
        assert_eq!(
            render_argv(&spec(), &extra),
            Err(ArgError::Unknown("evil".into()))
        );
    }
}
