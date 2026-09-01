use crate::js_compat;

/// Rounds `value` to `significant_digits` significant digits, taking the
/// magnitude of the input into account (`round_to(1234.5, 3) == 1230.0`,
/// `round_to(0.012345, 2) == 0.012`).
///
/// Zero and non-finite values are returned unchanged; negative values are
/// rounded by magnitude and keep their sign.
pub fn round_to(value: f64, significant_digits: u32) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    if value < 0.0 {
        return -round_to(-value, significant_digits);
    }
    let magnitude_digits = value.log10().floor();
    let scale = 10_f64.powf(f64::from(significant_digits.saturating_sub(1)) - magnitude_digits);
    js_compat::round(value * scale) / scale
}

#[cfg(test)]
mod tests {
    // Exact comparisons are intentional: rounding must hit these values exactly.
    #![allow(clippy::float_cmp)]

    use super::round_to;

    #[test]
    fn rounds_large_values_down_to_significant_digits() {
        assert_eq!(round_to(1234.5, 3), 1230.0);
        assert_eq!(round_to(1234.5, 2), 1200.0);
        assert_eq!(round_to(1234.5, 1), 1000.0);
        assert_eq!(round_to(1234.5, 5), 1234.5);
    }

    #[test]
    fn rounds_small_values() {
        assert_eq!(round_to(0.012_345, 2), 0.012);
        assert_eq!(round_to(0.012_345, 3), 0.0123);
        assert_eq!(round_to(1.5, 3), 1.5);
    }

    #[test]
    fn zero_significant_digits_behaves_like_one() {
        assert_eq!(round_to(1234.5, 0), 1000.0);
    }

    #[test]
    fn zero_and_non_finite_pass_through() {
        assert_eq!(round_to(0.0, 3), 0.0);
        assert!(round_to(f64::NAN, 3).is_nan());
        assert_eq!(round_to(f64::INFINITY, 3), f64::INFINITY);
    }

    #[test]
    fn negative_values_keep_sign() {
        assert_eq!(round_to(-1234.5, 3), -1230.0);
        assert_eq!(round_to(-0.012_345, 2), -0.012);
    }
}
