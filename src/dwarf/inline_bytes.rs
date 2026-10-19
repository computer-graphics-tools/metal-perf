use std::collections::HashMap;

#[derive(Default)]
pub struct InlineBytes {
    pub kernels: Vec<(String, u64)>,
    pub exclusive: HashMap<String, u64>,
}
