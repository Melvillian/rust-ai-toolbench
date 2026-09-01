use crate::js_compat;

/// Formats a percentage value rounded to one decimal place, e.g.
/// `format_percentage(12.34) == "12.3%"`. The input is already a percentage
/// (not a ratio): `0.1234` becomes `"0.1%"`.
pub fn format_percentage(value: f64) -> String {
    let rounded = js_compat::round(value * 10.0) / 10.0;
    format!("{}%", js_compat::number_to_string(rounded))
}

#[cfg(test)]
mod tests {
    use super::format_percentage;

    #[test]
    fn rounds_to_one_decimal() {
        assert_eq!(format_percentage(0.1234), "0.1%");
        assert_eq!(format_percentage(12.34), "12.3%");
        assert_eq!(format_percentage(12.35), "12.4%");
        assert_eq!(format_percentage(99.96), "100%");
    }

    #[test]
    fn integers_have_no_fraction() {
        assert_eq!(format_percentage(50.0), "50%");
        assert_eq!(format_percentage(0.0), "0%");
    }

    #[test]
    fn negative_values() {
        assert_eq!(format_percentage(-12.34), "-12.3%");
    }
}
