use serde::Deserialize;

#[derive(Deserialize)]
pub struct CounterSample {
    pub t: f64,
    pub v: f64,
}
