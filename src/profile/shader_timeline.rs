use serde::Deserialize;

use super::interval::Interval;

const SPLIT_TOLERANCE_NS: i64 = 50;

#[derive(Deserialize)]
pub struct ShaderTimeline {
    pub name: String,
    pub intervals: Vec<Interval>,
}

impl ShaderTimeline {
    pub fn merged_intervals(&self) -> Vec<Interval> {
        let mut ordered = self.intervals.clone();
        ordered.sort_by_key(|interval| interval.begin_ns);
        let mut merged: Vec<Interval> = Vec::with_capacity(ordered.len());
        for interval in ordered {
            match merged.last_mut() {
                Some(last) if (interval.begin_ns - last.end_ns).abs() < SPLIT_TOLERANCE_NS => {
                    last.end_ns = interval.end_ns
                },
                _ => merged.push(interval),
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::{Interval, ShaderTimeline};

    #[test]
    fn merges_the_profiler_split_of_a_first_dispatch() {
        let timeline = ShaderTimeline {
            name: "kernel".to_owned(),
            intervals: vec![
                Interval {
                    begin_ns: 1000,
                    end_ns: 1500,
                },
                Interval {
                    begin_ns: 0,
                    end_ns: 400,
                },
                Interval {
                    begin_ns: 420,
                    end_ns: 900,
                },
            ],
        };
        let merged = timeline.merged_intervals();
        assert_eq!(
            merged,
            vec![
                Interval {
                    begin_ns: 0,
                    end_ns: 900,
                },
                Interval {
                    begin_ns: 1000,
                    end_ns: 1500,
                },
            ]
        );
    }
}
