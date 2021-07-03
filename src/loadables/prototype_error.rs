use std::fmt::{Display, Formatter};

use crate::loadables::loadable::LoadablePrototype;
use crate::loadables::sources::LoadableSource;

#[derive(Debug)]
pub enum PrototypeLoadingError<Prototype, Source>
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    Source(Source::Error),
    Prototype(Prototype::Error)
}

impl<Prototype, Source> Display for PrototypeLoadingError<Prototype, Source>
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PrototypeLoadingError::Prototype(error) => error.fmt(f),
            PrototypeLoadingError::Source(error) => error.fmt(f)
        }
    }
}



impl<Prototype, Source> std::error::Error for PrototypeLoadingError<Prototype, Source>
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    // FIXME: Implement source
    /*
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrototypeLoadingError::Source(error) => Some(&error),
            PrototypeLoadingError::Prototype(error) => Some(&error)
        }
    }
     */
}

/*impl<Prototype, Source> From<Source::Error> for PrototypeLoadingError<Prototype, Source>
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    fn from(error: <Source as LoadableSource>::Error) -> Self {
        Self::Source(error)
    }
}

impl<Prototype, Source> From<Prototype::Error> for PrototypeLoadingError<Prototype, Source>
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    fn from(error: <Prototype as LoadablePrototype<Source>>::Error) -> Self {
        Self::Prototype(error)
    }
}*/
