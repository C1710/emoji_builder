use std::fmt::Debug;
use crate::loadables::loadable::{Loadable, LoadablePrototype};
use crate::loadables::cli_loadable::{CliLoadable, CliLoadablePrototype};
use crate::loadables::sources::LoadableSource;

pub trait BuilderConfig<'de, Prototype, Source>:
    serde::Deserialize<'de>
    + CliLoadable<Prototype, Source>
    + Loadable<Prototype, Source>
    + Debug
    + Clone
    where Prototype: CliLoadablePrototype<Source>, Source: LoadableSource {}