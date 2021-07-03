use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Debug;

use crate::loadables::prototype_error::PrototypeLoadingError;
use crate::loadables::sources::LoadableSource;
use crate::loadables::loading_error::LoadingError;

// TODO: Add a custom derive-macro
// As of now, we have to distinguish between Loadable and Prototype, as otherwise we would
// end up with a cyclic implementation because of TryFrom...
// imagine impl<T,P> Loadable for T where T: TryFrom<P>, P: Loadable
// In that case we would end up with a conflicting implementation for our prototype-type
// (let's call it Prototype), as Prototype: TryFrom<Prototype>, which means, we already have an
// Implementation for it.
// Plus it would get complicated if we introduced another TryFrom for a type that happens to be
// TryFrom for another type... that implements Loadable
// Therefore, this structure limits us to these (more reasonable) usecases.

// TODO: Maybe it would make sense to have Loadable just implementing a TryFrom-chain?
//       e.g. EmojiPack: TryFrom<EmojiPackFile> and EmojiPackFile: TryFrom<PathBuf>
//       and therefore EmojiPack: Loadable<PathBuf>?

// TODO: Use this to pass information to the Loadable to e.g. normalize paths.
//       It should be the same information the Prototype gets, just without the actual data it is
//       built from. E.g. file paths, etc.
// TODO: Maybe make this an enum for the different backends for loading prototypes?
// TODO: Maybe make it generic?
// type PrototypeMetadata = ();

pub trait Loadable<Prototype, Source>: Sized + Debug + Send + Sync
    where Prototype: LoadablePrototype<Source>, Source: LoadableSource {
    type Error: Error;
    // TODO: What do we want to do here?
    //       1. Provide functions that can load the struct from the same files as its prototype,
    //          but this time with a transformation into a usable format
    // TODO: Ideas:
    //       1. Maybe use a common supertrait for both Prototype and Loadable such that they can
    //          implement the same from-function(s)?

    fn load(source: Source) -> Result<Self, LoadingError<Self, Prototype, Source>>
        where Source: LoadableSource;
}


impl<Prototype, T, E, Source> Loadable<Prototype, Source> for T
where Prototype: LoadablePrototype<Source>,
          T: TryFrom<(Prototype, Source), Error=E> + Debug + Send + Sync,
          E: std::error::Error,
          Source: LoadableSource {
    type Error = T::Error;

    fn load(source: Source) -> Result<Self, LoadingError<Self, Prototype, Source>>
        where Source: LoadableSource {
        let prototype = Prototype::load_prototype(&source)?;
        Self::try_from((prototype, source))
            .map_err(|error| LoadingError::Loadable(error))
    }
}


pub trait LoadablePrototype<Source>: Debug + Send + Sync
    where Source: LoadableSource {
    type Error: Error;
    // TODO: What should be the capabilities?
    //       1. We want to create these structs from a reader or a file (later possibly in a ZIP file)
    //            This should work using serde or a csv reader or anything!
    //       2. We want (in some cases) to create these structs from CLI-args
    //       3. These structs are _not_ supposed to already contain all the data; they are in the
    //          end only a common data structure from which the actual Loadable can be built
    //       4. We might however want to do some preprocessing, like making paths relative to the
    //          file's path (even in a ZIP?)
    //       5. We want to use additional information to e.g. determine the content type (e.g. for
    //          Deserialize, where we need to figure out the format)
    // TODO: Ideas:
    //       Use Base Path (where the file is located or e.g. for a ZIP-file just
    //       / or something similar) + Reader

    // TODO: Do we need to incorporate the source error here?
    // TODO: Do we need to use the source here?!
    fn load_prototype(source: &Source) -> Result<Self, PrototypeLoadingError<Self, Source>>
        where Source: LoadableSource, Self: Sized;
}