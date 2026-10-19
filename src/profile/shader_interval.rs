use std::rc::Rc;

#[derive(Clone)]
pub struct ShaderInterval {
    pub begin_ns: i64,
    pub end_ns: i64,
    pub shader: Rc<str>,
}
