#[derive(Default)]
pub struct IrFunction {
    pub name: String,
    pub instructions: usize,
    pub calls: Vec<(String, usize)>,
}

impl IrFunction {
    pub fn add_call(
        &mut self,
        callee: &str,
    ) {
        match self.calls.iter_mut().find(|(name, _)| name == callee) {
            Some((_, count)) => *count += 1,
            None => self.calls.push((callee.to_owned(), 1)),
        }
    }
}
