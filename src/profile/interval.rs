use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Interval {
    pub begin_ns: i64,
    pub end_ns: i64,
}
