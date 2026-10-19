use std::str::FromStr;

use super::Error;

#[derive(Clone, Debug)]
pub struct Minimum {
    pub column: String,
    pub value: f64,
}

impl FromStr for Minimum {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let syntax = || Error::MinimumSyntax(text.to_owned());
        let (column, value) = text.split_once('=').ok_or_else(syntax)?;
        if column.is_empty() {
            return Err(syntax());
        }
        Ok(Self {
            column: column.to_owned(),
            value: value.trim().parse().map_err(|_| syntax())?,
        })
    }
}
