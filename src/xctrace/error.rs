use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: quick_xml::Error,
    },
    #[error("malformed XML attribute")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),
    #[error("malformed XML")]
    Xml(#[from] quick_xml::Error),
    #[error("unknown XML entity &{0};")]
    UnknownEntity(String),
    #[error("a cell refers to undefined id {0}")]
    UnknownReference(String),
    #[error("the table has no column {name}; columns: {columns}")]
    UnknownColumn {
        name: String,
        columns: String,
    },
    #[error("a filter must look like column~text: {0}")]
    FilterSyntax(String),
    #[error("a minimum must look like column=number: {0}")]
    MinimumSyntax(String),
    #[error("the export holds no table schema")]
    NoSchema,
}
