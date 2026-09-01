//! Small helpers that reproduce JavaScript number semantics so the formatters
//! produce byte-for-byte the same output as the original TypeScript library.

/// Formats an `f64` the way JavaScript's `String(n)` does for ordinary values:
/// integers print without a fractional part (`1000`, not `1000.0`), non-integers
/// print the shortest round-trip representation with no trailing zeros (`1.5`,
/// `0.012`), and `-0` prints as `0`.
///
/// Exponent notation (used by JS for very large/small magnitudes) is not
/// reproduced; such values print in plain decimal form.
pub(crate) fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_owned();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if n == 0.0 {
        return "0".to_owned();
    }
    // `Display` for f64 already prints the shortest round-trip form without a
    // trailing ".0" for integral values.
    format!("{n}")
}

/// JavaScript's `Math.round`: rounds half toward positive infinity
/// (`round(-2.5) == -2`), unlike Rust's `f64::round` which rounds half away
/// from zero.
pub(crate) fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

#[cfg(test)]
mod tests {
    // Exact comparisons are intentional: these helpers must be bit-faithful.
    #![allow(clippy::float_cmp)]

    use super::{number_to_string, round};

    #[test]
    fn integers_print_without_fraction() {
        assert_eq!(number_to_string(1000.0), "1000");
        assert_eq!(number_to_string(1.0), "1");
        assert_eq!(number_to_string(-42.0), "-42");
    }

    #[test]
    fn fractions_print_without_trailing_zeros() {
        assert_eq!(number_to_string(1.5), "1.5");
        assert_eq!(number_to_string(12.3), "12.3");
        assert_eq!(number_to_string(0.012), "0.012");
        assert_eq!(number_to_string(-0.25), "-0.25");
    }

    #[test]
    fn zero_and_special_values() {
        assert_eq!(number_to_string(0.0), "0");
        assert_eq!(number_to_string(-0.0), "0");
        assert_eq!(number_to_string(f64::NAN), "NaN");
        assert_eq!(number_to_string(f64::INFINITY), "Infinity");
        assert_eq!(number_to_string(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn round_matches_math_round() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(-2.6), -3.0);
        assert_eq!(round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(round(12.345), 12.0);
        assert!(round(f64::NAN).is_nan());
    }
}
