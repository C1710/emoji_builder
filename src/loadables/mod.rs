use std::fmt::{Display, Formatter};

pub mod loadable;
pub mod prototypes;
pub mod sources;
pub mod cli_loadable;
pub mod prototype_error;
pub mod loading_error;

#[derive(Debug, Clone, Copy)]
pub struct NoError;

impl Display for NoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        "".fmt(f)
    }
}

impl std::error::Error for NoError {}

impl From<()> for NoError {
    fn from(_: ()) -> Self {
        Self {}
    }
}