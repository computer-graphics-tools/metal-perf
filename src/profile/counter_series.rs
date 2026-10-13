use serde::Deserialize;

use super::counter_sample::CounterSample;

#[derive(Deserialize)]
pub struct CounterSeries {
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
}
