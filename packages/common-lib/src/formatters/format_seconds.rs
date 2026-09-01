use super::format_unit::{Unit, format_unit};

// NOTE: the "M" (month) scale is `30 * 25 * 60 * 60`, exactly as in the original
// TypeScript library. It is kept verbatim for output parity.
const SECOND_UNITS: &[Unit] = &[
    Unit {
        value: 30.0 * 25.0 * 60.0 * 60.0,
        notation: "M",
    },
    Unit {
        value: 7.0 * 24.0 * 60.0 * 60.0,
        notation: "w",
    },
    Unit {
        value: 24.0 * 60.0 * 60.0,
        notation: "d",
    },
    Unit {
        value: 60.0 * 60.0,
        notation: "h",
    },
    Unit {
        value: 60.0,
        notation: "m",
    },
    Unit {
        value: 1.0,
        notation: "s",
    },
    Unit {
        value: 0.001,
        notation: "ms",
    },
];

/// Formats a duration in seconds with ms/s/m/h/d/w/M suffixes and 3
/// significant digits, e.g. `500ms`, `1.5m`, `2h`. Returns an empty string
/// for `None`.
pub fn format_seconds(value: Option<f64>) -> String {
    value.map_or_else(String::new, |value| format_unit(value, 3, SECOND_UNITS))
}

#[cfg(test)]
mod tests {
    use super::format_seconds;

    #[test]
    fn none_is_empty() {
        assert_eq!(format_seconds(None), "");
    }

    #[test]
    fn formats_each_scale() {
        assert_eq!(format_seconds(Some(0.0)), "0ms");
        assert_eq!(format_seconds(Some(0.5)), "500ms");
        assert_eq!(format_seconds(Some(1.0)), "1s");
        assert_eq!(format_seconds(Some(45.678)), "45.7s");
        assert_eq!(format_seconds(Some(90.0)), "1.5m");
        assert_eq!(format_seconds(Some(3600.0)), "1h");
        assert_eq!(format_seconds(Some(86_400.0)), "1d");
        assert_eq!(format_seconds(Some(604_800.0)), "1w");
        assert_eq!(format_seconds(Some(2_700_000.0)), "1M");
    }

    #[test]
    fn sub_millisecond_uses_ms() {
        assert_eq!(format_seconds(Some(0.000_25)), "0.25ms");
    }
}
