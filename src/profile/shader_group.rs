use std::str::FromStr;

use regex::Regex;

use super::Error;

#[derive(Clone, Debug)]
pub struct ShaderGroup {
    pub label: String,
    pub pattern: Regex,
}

impl FromStr for ShaderGroup {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (label, pattern) = text.split_once('=').ok_or_else(|| Error::GroupSyntax(text.to_owned()))?;
        if label.is_empty() {
            return Err(Error::GroupSyntax(text.to_owned()));
        }
        Ok(Self {
            label: label.to_owned(),
            pattern: Regex::new(pattern)?,
        })
    }
}
