use super::sum::sum;

/// Scales `values` so they sum to 1. An all-zero (or empty) input yields
/// non-finite entries, mirroring the original library's `1 / 0` behaviour.
pub fn normalize_distribution(values: &[f64]) -> Vec<f64> {
    let inverse_sum = 1.0 / sum(values);
    values.iter().map(|value| value * inverse_sum).collect()
}

#[cfg(test)]
mod tests {
    // Exact comparisons are intentional: these inputs normalise exactly.
    #![allow(clippy::float_cmp)]

    use super::normalize_distribution;

    #[test]
    fn scales_to_unit_sum() {
        assert_eq!(
            normalize_distribution(&[1.0, 1.0, 2.0]),
            vec![0.25, 0.25, 0.5]
        );
        assert_eq!(normalize_distribution(&[5.0]), vec![1.0]);
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(normalize_distribution(&[]).is_empty());
    }

    #[test]
    fn zero_sum_yields_non_finite() {
        let out = normalize_distribution(&[0.0, 0.0]);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|v| !v.is_finite()));
    }
}
