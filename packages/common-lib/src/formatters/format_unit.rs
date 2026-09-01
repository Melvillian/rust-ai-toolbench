use crate::js_compat;
use crate::math::round_to;

/// A unit in a formatting table: the scale (`value`) and the suffix printed
/// after the scaled number (`notation`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    pub value: f64,
    pub notation: &'static str,
}

const FALLBACK_UNIT: Unit = Unit {
    value: 1.0,
    notation: "",
};

/// Formats `value` using the first unit in `units` (ordered largest to
/// smallest) whose scale it reaches, falling back to the last unit (or a
/// unit-less scale of 1 when `units` is empty). The scaled number is rounded to
/// `significant_digits` significant digits and printed JS-style
/// (`1.5KB`, `12.3MB`, `1KB`).
pub fn format_unit(value: f64, significant_digits: u32, units: &[Unit]) -> String {
    let unit = units
        .iter()
        .find(|unit| value >= unit.value)
        .or_else(|| units.last())
        .copied()
        .unwrap_or(FALLBACK_UNIT);
    let scaled = round_to(value / unit.value, significant_digits);
    format!("{}{}", js_compat::number_to_string(scaled), unit.notation)
}

#[cfg(test)]
mod tests {
    use super::{Unit, format_unit};

    const UNITS: &[Unit] = &[
        Unit {
            value: 1000.0,
            notation: "K",
        },
        Unit {
            value: 1.0,
            notation: "",
        },
    ];

    #[test]
    fn picks_largest_matching_unit() {
        assert_eq!(format_unit(1500.0, 3, UNITS), "1.5K");
        assert_eq!(format_unit(1000.0, 3, UNITS), "1K");
        assert_eq!(format_unit(999.0, 3, UNITS), "999");
    }

    #[test]
    fn rounds_to_significant_digits() {
        assert_eq!(format_unit(1234.0, 3, UNITS), "1.23K");
        assert_eq!(format_unit(1234.0, 2, UNITS), "1.2K");
        assert_eq!(format_unit(1234.0, 1, UNITS), "1K");
    }

    #[test]
    fn falls_back_to_last_unit_below_smallest_scale() {
        assert_eq!(format_unit(0.5, 3, UNITS), "0.5");
        assert_eq!(format_unit(0.0, 3, UNITS), "0");
    }

    #[test]
    fn empty_unit_table_uses_unitless_scale() {
        assert_eq!(format_unit(1500.0, 3, &[]), "1500");
    }
}
