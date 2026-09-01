//! One module per generator command. To add a generator: create a module here,
//! give it a clap `Args` struct plus a `run` function, and add a variant to
//! the `Command` enum in `main.rs`.

pub mod render_deploy;
