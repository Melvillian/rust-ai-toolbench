use super::sum::sum;

/// Returns the running total of the normalised `values`; the last entry is
/// (approximately) 1. An all-zero (or empty) input yields non-finite entries,
/// mirroring the original library's `1 / 0` behaviour.
pub fn cumulative_distribution(values: &[f64]) -> Vec<f64> {
    let inverse_sum = 1.0 / sum(values);
    let mut cumulative_sum = 0.0;
    values
        .iter()
        .map(|value| {
            cumulative_sum += value * inverse_sum;
            cumulative_sum
        })
        .collect()
}

#[cfg(test)]
mod tests {
    // Exact comparisons are intentional: these inputs accumulate exactly.
    #![allow(clippy::float_cmp)]

    use super::cumulative_distribution;

    #[test]
    fn accumulates_normalised_values() {
        assert_eq!(
            cumulative_distribution(&[1.0, 1.0, 2.0]),
            vec![0.25, 0.5, 1.0]
        );
        assert_eq!(cumulative_distribution(&[5.0]), vec![1.0]);
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(cumulative_distribution(&[]), Vec::<f64>::new());
    }

    #[test]
    fn is_monotonic() {
        let out = cumulative_distribution(&[3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0]);
        assert!(out.windows(2).all(|w| w[0] <= w[1]));
        assert!((out[out.len() - 1] - 1.0).abs() < 1e-12);
    }
}
