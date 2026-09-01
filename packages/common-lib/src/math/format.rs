use crate::js_compat;

/// Formats a number like `Intl.NumberFormat("en-US")` with default options:
/// thousands separated by `,` and at most 3 fraction digits with trailing
/// zeros dropped (`1234567.891` → `"1,234,567.891"`, `1000` → `"1,000"`).
/// Returns an empty string for `None`.
pub fn format(value: Option<f64>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if !value.is_finite() {
        return js_compat::number_to_string(value);
    }

    let fixed = format!("{:.3}", value.abs());
    let (int_part, frac_part) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let frac_part = frac_part.trim_end_matches('0');

    let mut out = String::with_capacity(fixed.len() + int_part.len() / 3 + 1);
    if value.is_sign_negative() {
        out.push('-');
    }
    let digit_count = int_part.len();
    for (i, digit) in int_part.chars().enumerate() {
        if i > 0 && (digit_count - i) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    if !frac_part.is_empty() {
        out.push('.');
        out.push_str(frac_part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn none_is_empty() {
        assert_eq!(format(None), "");
    }

    #[test]
    fn inserts_thousands_separators() {
        assert_eq!(format(Some(0.0)), "0");
        assert_eq!(format(Some(999.0)), "999");
        assert_eq!(format(Some(1000.0)), "1,000");
        assert_eq!(format(Some(123_456.0)), "123,456");
        assert_eq!(format(Some(1_234_567.0)), "1,234,567");
        assert_eq!(format(Some(1_000_000_000.0)), "1,000,000,000");
    }

    #[test]
    fn keeps_up_to_three_fraction_digits() {
        assert_eq!(format(Some(1_234_567.891)), "1,234,567.891");
        assert_eq!(format(Some(1.5)), "1.5");
        assert_eq!(format(Some(0.25)), "0.25");
        assert_eq!(format(Some(1.234_56)), "1.235");
        assert_eq!(format(Some(2.000_1)), "2");
    }

    #[test]
    fn negative_values() {
        assert_eq!(format(Some(-1234.5)), "-1,234.5");
        assert_eq!(format(Some(-0.5)), "-0.5");
    }

    #[test]
    fn non_finite_values() {
        assert_eq!(format(Some(f64::NAN)), "NaN");
        assert_eq!(format(Some(f64::INFINITY)), "Infinity");
    }
}
