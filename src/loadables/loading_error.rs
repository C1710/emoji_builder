use crate::loadables::loadable::{Loadable, LoadablePrototype};
use std::fmt::{Display, Formatter};
use crate::loadables::prototype_error::PrototypeLoadingError;
use crate::loadables::sources::LoadableSource;

#[derive(Debug)]
pub enum LoadingError<L, P, S>
    where L: Loadable<P, S>,
          P: LoadablePrototype<S>,
          S: LoadableSource {
    Loadable(<L as Loadable<P, S>>::Error),
    Prototype(P::Error),
    Source(S::Error)
}

impl<L, Prototype, Source> std::error::Error for LoadingError<L, Prototype, Source>
    where L: Loadable<Prototype, Source>,
          Prototype: LoadablePrototype<Source>,
          Source: LoadableSource {
    // FIXME: Implement this
    /*
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Loadable(error) => Some(&error),
            Self::Source(error) => Some(&error),
            Self::Prototype(error) => Some(&error)
        }
    }*/
}

// FIXME: Maybe try to implement From again?

impl<L, Prototype, Source> Display for LoadingError<L, Prototype, Source>
    where L: Loadable<Prototype, Source>,
          Prototype: LoadablePrototype<Source>,
          Source: LoadableSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingError::Loadable(error) => error.fmt(f),
            LoadingError::Prototype(error) => error.fmt(f),
            LoadingError::Source(error) => error.fmt(f),
        }
    }
}

impl<L, Prototype, Source> From<PrototypeLoadingError<Prototype, Source>> for LoadingError<L, Prototype, Source>
    where L: Loadable<Prototype, Source>, Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    fn from(error: PrototypeLoadingError<Prototype, Source>) -> Self {
        match error {
            PrototypeLoadingError::Source(error) => Self::Source(error),
            PrototypeLoadingError::Prototype(error) => Self::Prototype(error)
        }
    }
}
