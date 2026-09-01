//! Shared utility library: formatters (bytes, counts, seconds), math helpers
//! (rounding, distributions, percentages) and general utilities (env access,
//! error formatting, object helpers, sleep, `to_camel_case`).
//!
//! Everything is re-exported at the crate root for convenience; the modules
//! remain public for callers that prefer qualified paths.

pub mod formatters;
mod js_compat;
pub mod math;
pub mod utilities;

pub use formatters::{Unit, format_bytes, format_count, format_seconds, format_unit};
pub use math::{
    cumulative_distribution, format, format_percentage, normalize_distribution, round_to, sum,
};
#[cfg(feature = "tokio")]
pub use utilities::sleep;
pub use utilities::{
    EnvError, error_to_string, filter, get_env, is_empty, pick, require_env, to_camel_case,
};
