use std::rc::Rc;

pub struct LineSpan {
    pub address: u64,
    pub size: u64,
    pub file: Rc<str>,
    pub line: u64,
}
