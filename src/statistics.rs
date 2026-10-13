pub fn median(sorted: &[f64]) -> f64 {
    match sorted.len() {
        0 => f64::NAN,
        length if length % 2 == 1 => sorted[length / 2],
        length => (sorted[length / 2 - 1] + sorted[length / 2]) / 2.0,
    }
}

pub fn quantile(
    sorted: &[f64],
    fraction: f64,
) -> f64 {
    match sorted.len() {
        0 => f64::NAN,
        length => sorted[((length as f64 * fraction) as usize).min(length - 1)],
    }
}

pub fn geometric_mean(values: &[f64]) -> f64 {
    (values.iter().map(|value| value.ln()).sum::<f64>() / values.len() as f64).exp()
}

#[cfg(test)]
mod tests {
    use super::{geometric_mean, median, quantile};

    #[test]
    fn summarises_samples() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 10.0];
        assert_eq!(median(&sorted), 3.0);
        assert_eq!(median(&sorted[..4]), 2.5);
        assert_eq!(quantile(&sorted, 0.1), 1.0);
        assert_eq!(quantile(&sorted, 0.9), 10.0);
        assert!((geometric_mean(&[2.0, 8.0]) - 4.0).abs() < 1e-12);
        assert!(median(&[]).is_nan());
    }
}
