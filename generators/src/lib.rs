//! Project generators for scaffolding common project patterns.
//!
//! The library exposes the pure, testable pieces (content builders and the
//! `render.yaml` append logic); the `generators` binary in `main.rs` wires
//! them to a clap CLI.

pub mod commands;
