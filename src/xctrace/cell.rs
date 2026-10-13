#[derive(Default)]
pub struct Cell {
    pub text: String,
    pub format: Option<String>,
}

impl Cell {
    pub fn display(&self) -> &str {
        self.format.as_deref().unwrap_or(&self.text)
    }

    pub fn number(&self) -> Option<f64> {
        self.text.trim().parse().ok()
    }
}
