use serde_json::Value;

pub struct Encoder {
    pub name: String,
    pub begin_ns: i64,
    pub end_ns: i64,
}

impl Encoder {
    pub fn from_row(row: &Value) -> Option<Self> {
        let values = row["values"].as_array()?;
        let name = values.first()?["value"].as_str()?.trim_matches('"').to_owned();
        Some(Self {
            name,
            begin_ns: values.get(1)?["value"].as_i64()?,
            end_ns: values.get(2)?["value"].as_i64()?,
        })
    }
}
