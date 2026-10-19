use crate::statistics::{median, quantile};

pub struct RunSummary {
    pub median: f64,
    pub tail: f64,
    pub samples: usize,
}

impl RunSummary {
    pub fn new(
        mut samples: Vec<f64>,
        tail_fraction: f64,
    ) -> Self {
        samples.sort_by(f64::total_cmp);
        Self {
            median: median(&samples),
            tail: quantile(&samples, tail_fraction),
            samples: samples.len(),
        }
    }
}
