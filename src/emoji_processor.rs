/*
 * Copyright 2020 Constantin A. <emoji.builder@c1710.de>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::emoji::Emoji;
use std::fmt::Debug;
use clap::{ArgMatches, App};

/// A trait that is capable of doing postprocessing for emojis.
/// This might be e.g. a PNG compressor or a masking system for flags (which might then work on the SVGs).
/// This trait is supposed to modularize the building process as certain processes might be useful
/// for different builders.
/// NOTICE: This trait is anything but ready! Anything might change at any time
pub trait EmojiProcessor<T>: Send + Sync {
    type Err: Debug + Send + Sync;

    /// Initializes a new `PostProcessor` before using it.
    /// This can set up different settings.
    ///
    /// The command line arguments from `clap` that have been specified by `sub_command` are
    /// passed here.
    fn new(
        arguments: Option<ArgMatches>,
    ) -> Result<Box<Self>, Self::Err>;

    /// Process one particular emoji. Must be thread-safe.
    /// Does nothing by default.
    /// # Arguments
    /// * `_emoji` is the current `Emoji` it's processing. Might be used to get metadata
    /// * `prepared` is the emoji that the builder prepared and that's supposed to be processed now.
    fn process(&self, _emoji: &Emoji, prepared: T) -> Result<T, Self::Err> {
        Ok(prepared)
    }

    /// Lets the postprocessor define its own set of command line arguments.
    /// It will be used as a subcommand for all appropriate builders
    ///
    /// The resulting argument match is returned in the `new` function.
    fn sub_command<'a, 'b>() -> App<'a, 'b>;

    /// The names of additional modules to enable logging for.
    /// It might be necessary to include the module itself by adding `String::from(module_path!())`
    /// to the `Vec`
    fn log_modules() -> Vec<String> {
        vec![String::from(module_path!())]
    }
}

