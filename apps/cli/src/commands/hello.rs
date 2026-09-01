//! `cli hello` — prints a greeting.

use std::io::Write;

/// The greeting printed by `cli hello` (without trailing newline).
pub fn hello() -> &'static str {
    "Hello world!"
}

/// Write the greeting followed by a newline to `out`.
pub fn run(out: &mut impl Write) -> std::io::Result<()> {
    writeln!(out, "{}", hello())
}

#[cfg(test)]
mod tests {
    use super::{hello, run};

    #[test]
    fn hello_returns_greeting() {
        assert_eq!(hello(), "Hello world!");
    }

    #[test]
    fn run_writes_greeting_with_newline() {
        let mut buf = Vec::new();
        run(&mut buf).expect("write to Vec cannot fail");
        assert_eq!(String::from_utf8(buf).unwrap(), "Hello world!\n");
    }
}
