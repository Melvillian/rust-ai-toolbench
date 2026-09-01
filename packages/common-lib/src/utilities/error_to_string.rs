use std::error::Error;
use std::fmt::Write as _;

/// Renders an error as its `Display` text followed by every error in its
/// `source()` chain, separated by `": "` (e.g. `"outer: inner: root"`).
pub fn error_to_string(error: &dyn Error) -> String {
    let mut out = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        // Writing to a String cannot fail.
        let _ = write!(out, ": {cause}");
        source = cause.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fmt;

    use super::error_to_string;

    #[derive(Debug)]
    struct Chained {
        message: &'static str,
        source: Option<Box<dyn Error + Send + Sync>>,
    }

    impl fmt::Display for Chained {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.message)
        }
    }

    impl Error for Chained {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.source.as_deref().map(|e| e as &(dyn Error + 'static))
        }
    }

    #[test]
    fn single_error_is_its_display() {
        let err = Chained {
            message: "root",
            source: None,
        };
        assert_eq!(error_to_string(&err), "root");
    }

    #[test]
    fn walks_the_source_chain() {
        let err = Chained {
            message: "outer",
            source: Some(Box::new(Chained {
                message: "inner",
                source: Some(Box::new(Chained {
                    message: "root",
                    source: None,
                })),
            })),
        };
        assert_eq!(error_to_string(&err), "outer: inner: root");
    }

    #[test]
    fn works_with_std_errors() {
        let err = "not a number".parse::<i32>().unwrap_err();
        assert_eq!(error_to_string(&err), "invalid digit found in string");
    }
}
