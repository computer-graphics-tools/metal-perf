use serde::Deserialize;

use super::counter_sample::CounterSample;

#[derive(Deserialize)]
pub struct CounterSeries {
    pub unit: Option<String>,
    pub samples: Vec<CounterSample>,
}

impl CounterSeries {
    pub fn window_mean(
        &self,
        spans: &[(f64, f64)],
    ) -> (f64, usize) {
        let mut sum = 0.0;
        let mut count = 0;
        for &(begin, end) in spans {
            let first = self.samples.partition_point(|sample| sample.t < begin);
            let last = self.samples.partition_point(|sample| sample.t <= end);
            for sample in &self.samples[first..last.max(first)] {
                sum += sample.v;
                count += 1;
            }
        }
        (
            if count == 0 {
                f64::NAN
            } else {
                sum / count as f64
            },
            count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CounterSample, CounterSeries};

    #[test]
    fn averages_samples_inside_the_spans() {
        let series = CounterSeries {
            unit: Some("Count".to_owned()),
            samples: (0..10)
                .map(|index| CounterSample {
                    t: index as f64 * 10.0,
                    v: index as f64,
                })
                .collect(),
        };
        assert_eq!(series.window_mean(&[(10.0, 30.0), (85.0, 95.0)]), (3.75, 4));
        let (mean, count) = series.window_mean(&[(1.0, 2.0)]);
        assert!(mean.is_nan());
        assert_eq!(count, 0);
    }

    #[test]
    fn preserves_capture_units_and_missing_status() {
        let series: CounterSeries =
            serde_json::from_str(r#"{"unit":"GiB / Second","samples":[{"t":10,"v":2},{"t":20,"v":4}]}"#).unwrap();
        assert_eq!(series.unit.as_deref(), Some("GiB / Second"));
        assert_eq!(series.window_mean(&[(10.0, 20.0)]), (3.0, 2));
        let missing: CounterSeries = serde_json::from_str(r#"{"samples":[]}"#).unwrap();
        assert_eq!(missing.unit, None);
    }
}
