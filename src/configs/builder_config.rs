use crate::loadable::{CliArgLoadable, LoadableImpl, Loadable};
use std::fmt::Debug;

pub trait BuilderConfig: serde::Deserialize + CliArgLoadable + Loadable + Debug + Clone {}