//! Math helpers: significant-digit rounding, pairwise summation,
//! distributions and locale-style number formatting.

mod cumulative_distribution;
mod format;
mod format_percentage;
mod normalize_distribution;
mod round_to;
mod sum;

pub use cumulative_distribution::cumulative_distribution;
pub use format::format;
pub use format_percentage::format_percentage;
pub use normalize_distribution::normalize_distribution;
pub use round_to::round_to;
pub use sum::sum;
