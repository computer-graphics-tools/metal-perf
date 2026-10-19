use std::str::FromStr;

use super::Error;

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub command: String,
}

impl FromStr for Variant {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let syntax = || Error::VariantSyntax(text.to_owned());
        let (name, command) = text.split_once('=').ok_or_else(syntax)?;
        let valid_name = !name.is_empty()
            && name.chars().all(|character| character.is_ascii_alphanumeric() || "._-".contains(character));
        if !valid_name || command.trim().is_empty() {
            return Err(syntax());
        }
        Ok(Self {
            name: name.to_owned(),
            command: command.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Variant;

    #[test]
    fn parses_name_and_command() {
        let variant: Variant = "main=GPU_TIMES=1 ./bench --mode a=b".parse().expect("valid variant");
        assert_eq!(variant.name, "main");
        assert_eq!(variant.command, "GPU_TIMES=1 ./bench --mode a=b");
        assert!("a b=cmd".parse::<Variant>().is_err());
        assert!("name=".parse::<Variant>().is_err());
        assert!("=cmd".parse::<Variant>().is_err());
    }
}
