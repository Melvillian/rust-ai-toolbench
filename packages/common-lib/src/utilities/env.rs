use thiserror::Error;

/// Error returned by [`require_env`] when a variable is unset or empty.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnvError {
    /// The named environment variable is missing or empty after trimming.
    #[error("Required environment variable {0} is missing or empty")]
    Missing(String),
}

/// Trims a raw environment value; empty (after trimming) or unset → `None`.
fn normalize(raw: Option<String>) -> Option<String> {
    let trimmed = raw?.trim().to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Reads an environment variable, trimmed. Returns `None` when the variable is
/// unset, not valid Unicode, or empty after trimming. Trimming once here
/// prevents a trailing newline pasted into a dashboard from leaking into an
/// API header (e.g. a 401 "invalid key").
pub fn get_env(name: &str) -> Option<String> {
    normalize(std::env::var(name).ok())
}

/// Like [`get_env`], but fails with [`EnvError::Missing`] (naming the variable)
/// when it is missing or empty.
pub fn require_env(name: &str) -> Result<String, EnvError> {
    get_env(name).ok_or_else(|| EnvError::Missing(name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{EnvError, get_env, normalize, require_env};

    // Setting process environment variables is `unsafe` in edition 2024 and the
    // workspace forbids unsafe code, so the trimming logic is exercised through
    // `normalize` and the real functions are checked against an unset variable.
    const UNSET: &str = "COMMON_LIB_ENV_TEST_UNSET_VARIABLE";

    #[test]
    fn trims_surrounding_whitespace_and_newlines() {
        assert_eq!(
            normalize(Some("  secret-value\n".to_owned())),
            Some("secret-value".to_owned())
        );
        assert_eq!(normalize(Some(" abc ".to_owned())), Some("abc".to_owned()));
    }

    #[test]
    fn unset_is_none() {
        assert_eq!(normalize(None), None);
        assert_eq!(get_env(UNSET), None);
    }

    #[test]
    fn empty_after_trim_is_none() {
        assert_eq!(normalize(Some("   ".to_owned())), None);
        assert_eq!(normalize(Some(String::new())), None);
    }

    #[test]
    fn require_env_error_names_the_variable() {
        let err = require_env(UNSET).unwrap_err();
        assert_eq!(err, EnvError::Missing(UNSET.to_owned()));
        assert!(err.to_string().contains(UNSET));
    }

    #[test]
    fn require_env_returns_present_value() {
        // PATH is set in every reasonable test environment.
        assert!(require_env("PATH").is_ok());
    }
}
