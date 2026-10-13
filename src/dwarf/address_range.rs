use std::str::FromStr;

use super::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddressRange {
    pub begin: u64,
    pub end: u64,
}

impl AddressRange {
    pub fn contains(
        &self,
        address: u64,
    ) -> bool {
        (self.begin..self.end).contains(&address)
    }
}

impl FromStr for AddressRange {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let syntax = || Error::RangeSyntax(text.to_owned());
        let parse = |part: &str| {
            u64::from_str_radix(part.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).map_err(|_| syntax())
        };
        let (begin, end) = text.split_once(':').ok_or_else(syntax)?;
        let range = Self {
            begin: parse(begin)?,
            end: parse(end)?,
        };
        if range.begin >= range.end {
            return Err(syntax());
        }
        Ok(range)
    }
}

#[cfg(test)]
mod tests {
    use super::AddressRange;

    #[test]
    fn parses_hex_ranges() {
        let range: AddressRange = "0x558:27b6".parse().expect("valid range");
        assert_eq!(
            range,
            AddressRange {
                begin: 0x558,
                end: 0x27b6,
            }
        );
        assert!(range.contains(0x558) && !range.contains(0x27b6));
        assert!("0x10:0x10".parse::<AddressRange>().is_err());
        assert!("10".parse::<AddressRange>().is_err());
    }
}
