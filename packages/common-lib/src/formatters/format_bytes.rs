use super::format_unit::{Unit, format_unit};

const BYTE_UNITS: &[Unit] = &[
    Unit {
        value: 1000.0 * 1000.0 * 1000.0 * 1000.0 * 1000.0,
        notation: "PB",
    },
    Unit {
        value: 1000.0 * 1000.0 * 1000.0 * 1000.0,
        notation: "TB",
    },
    Unit {
        value: 1000.0 * 1000.0 * 1000.0,
        notation: "GB",
    },
    Unit {
        value: 1000.0 * 1000.0,
        notation: "MB",
    },
    Unit {
        value: 1000.0,
        notation: "KB",
    },
    Unit {
        value: 1.0,
        notation: "B",
    },
];

/// Formats a byte count with decimal (SI, base-1000) units and 3 significant
/// digits, e.g. `1.5KB`, `12.3MB`. Returns an empty string for `None`.
pub fn format_bytes(value: Option<f64>) -> String {
    value.map_or_else(String::new, |value| format_unit(value, 3, BYTE_UNITS))
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn none_is_empty() {
        assert_eq!(format_bytes(None), "");
    }

    #[test]
    fn formats_each_scale() {
        assert_eq!(format_bytes(Some(0.0)), "0B");
        assert_eq!(format_bytes(Some(999.0)), "999B");
        assert_eq!(format_bytes(Some(1000.0)), "1KB");
        assert_eq!(format_bytes(Some(1500.0)), "1.5KB");
        assert_eq!(format_bytes(Some(1_000_000.0)), "1MB");
        assert_eq!(format_bytes(Some(12_345_678.0)), "12.3MB");
        assert_eq!(format_bytes(Some(1_000_000_000.0)), "1GB");
        assert_eq!(format_bytes(Some(1_000_000_000_000.0)), "1TB");
        assert_eq!(format_bytes(Some(1_000_000_000_000_000.0)), "1PB");
        assert_eq!(format_bytes(Some(2_500_000_000_000_000.0)), "2.5PB");
    }
}
