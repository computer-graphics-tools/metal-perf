use quick_xml::{XmlVersion, events::BytesStart};

use super::Error;

#[derive(Default)]
pub struct OpenElement {
    pub id: Option<String>,
    pub reference: Option<String>,
    pub format: Option<String>,
    pub sentinel: bool,
    pub text: String,
}

impl OpenElement {
    pub fn new(start: &BytesStart) -> Result<Self, Error> {
        let mut element = Self {
            sentinel: start.name().as_ref() == "sentinel",
            ..Self::default()
        };
        for attribute in start.attributes() {
            let attribute = attribute?;
            let value = attribute.normalized_value(XmlVersion::Explicit1_0)?.into_owned();
            match attribute.key.as_ref() {
                "id" => element.id = Some(value),
                "ref" => element.reference = Some(value),
                "fmt" => element.format = Some(value),
                _ => {},
            }
        }
        Ok(element)
    }
}
