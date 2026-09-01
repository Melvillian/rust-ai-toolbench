/// Ranges shorter than this are summed linearly instead of being split.
const SPLIT_SUM_MINIMUM_RANGE: usize = 5;

fn split_sum(values: &[f64]) -> f64 {
    // `values.len() - 1 < SPLIT_SUM_MINIMUM_RANGE` in the original (inclusive
    // end index); written without subtraction to stay safe for empty slices.
    if values.len() <= SPLIT_SUM_MINIMUM_RANGE {
        return values.iter().sum();
    }
    let middle = (values.len() - 1) / 2;
    let (left, right) = values.split_at(middle + 1);
    split_sum(left) + split_sum(right)
}

/// Sums `values` using pairwise (split) summation, which limits floating point
/// error growth compared to a naive left-to-right sum. Returns `0.0` for an
/// empty slice.
pub fn sum(values: &[f64]) -> f64 {
    split_sum(values)
}

#[cfg(test)]
mod tests {
    // Exact comparisons are intentional: these inputs sum exactly.
    #![allow(clippy::float_cmp)]

    use super::sum;

    #[test]
    fn empty_is_zero() {
        assert_eq!(sum(&[]), 0.0);
    }

    #[test]
    fn single_element() {
        assert_eq!(sum(&[4.5]), 4.5);
    }

    #[test]
    fn fewer_than_split_threshold() {
        assert_eq!(sum(&[1.0, 2.0, 3.0, 4.0]), 10.0);
    }

    #[test]
    fn exactly_split_threshold() {
        assert_eq!(sum(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 21.0);
    }

    #[test]
    fn more_than_split_threshold() {
        let values: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(sum(&values), 5050.0);
    }

    #[test]
    fn negative_values() {
        assert_eq!(sum(&[1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0]), 4.0);
    }
}
