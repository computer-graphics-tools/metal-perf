use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LinesMode {
    /// Code bytes per source line, most expensive first
    Lines,
    /// Address-ordered runs of file:line; a loop header reappears near the back edge
    Layout,
}
