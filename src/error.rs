#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Ab(#[from] crate::ab::Error),
    #[error(transparent)]
    Air(#[from] crate::air::Error),
    #[error(transparent)]
    Cache(#[from] crate::cache::Error),
    #[error(transparent)]
    Capture(#[from] crate::capture::Error),
    #[error(transparent)]
    Dwarf(#[from] crate::dwarf::Error),
    #[error(transparent)]
    Gpudebug(#[from] crate::gpudebug::Error),
    #[error(transparent)]
    Probe(#[from] crate::probe::Error),
    #[error(transparent)]
    Profile(#[from] crate::profile::Error),
    #[error(transparent)]
    Xctrace(#[from] crate::xctrace::Error),
}
