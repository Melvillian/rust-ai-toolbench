use super::format_unit::{Unit, format_unit};

const COUNT_UNITS: &[Unit] = &[
    Unit {
        value: 1000.0 * 1000.0 * 1000.0 * 1000.0 * 1000.0,
        notation: "P",
    },
    Unit {
        value: 1000.0 * 1000.0 * 1000.0 * 1000.0,
        notation: "T",
    },
    Unit {
        value: 1000.0 * 1000.0 * 1000.0,
        notation: "B",
    },
    Unit {
        value: 1000.0 * 1000.0,
        notation: "M",
    },
    Unit {
        value: 1000.0,
        notation: "K",
    },
    Unit {
        value: 1.0,
        notation: "",
    },
];

/// Formats a count with K/M/B/T/P suffixes and 3 significant digits,
/// e.g. `1.5K`, `12.3M`. Returns an empty string for `None`.
pub fn format_count(value: Option<f64>) -> String {
    value.map_or_else(String::new, |value| format_unit(value, 3, COUNT_UNITS))
}

#[cfg(test)]
mod tests {
    use super::format_count;

    #[test]
    fn none_is_empty() {
        assert_eq!(format_count(None), "");
    }

    #[test]
    fn formats_each_scale() {
        assert_eq!(format_count(Some(0.0)), "0");
        assert_eq!(format_count(Some(500.0)), "500");
        assert_eq!(format_count(Some(1500.0)), "1.5K");
        assert_eq!(format_count(Some(1_000_000.0)), "1M");
        assert_eq!(format_count(Some(12_345_678.0)), "12.3M");
        assert_eq!(format_count(Some(1_000_000_000.0)), "1B");
        assert_eq!(format_count(Some(1_000_000_000_000.0)), "1T");
        assert_eq!(format_count(Some(1_000_000_000_000_000.0)), "1P");
    }
}
