use std::str::FromStr;

use super::Error;

#[derive(Clone, Debug)]
pub struct Filter {
    pub column: String,
    pub text: String,
}

impl FromStr for Filter {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.split_once('~') {
            Some((column, needle)) if !column.is_empty() => Ok(Self {
                column: column.to_owned(),
                text: needle.to_owned(),
            }),
            _ => Err(Error::FilterSyntax(text.to_owned())),
        }
    }
}
