use crate::loadables::loadable::{Loadable, LoadablePrototype};
use crate::loadables::sources::LoadableSource;

// The problem, why we still need to specify a source here is that any Prototype that's parsable
// from CLI also has to be parsable from other sources
pub trait CliLoadable<Prototype, S>: Loadable<Prototype, S>
    where Prototype: CliLoadablePrototype<S>, S: LoadableSource {
    // TODO: Add a function here that adds loading from CLI
}


// In any way we want a prototype to also be loadable from files/file-like structures
pub trait CliLoadablePrototype<S>: LoadablePrototype<S>
    where S: LoadableSource {
    // TODO:
    //       1. Let it supply a clap::App (or similar?) to pass on to the main program to include
    //          there
    //       2. Let it use clap::ArgMatches to parse the requested arguments
}