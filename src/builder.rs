/*
 * Copyright 2019 Constantin A. <emoji.builder@c1710.de>
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
//! This is the main module for the actual emoji processing.

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;

use clap::{App, ArgMatches};

use crate::builder::ResetError::IoError;
use crate::emojis::emoji::Emoji;

/// Represents (if [core::result::Result::Ok]) a prepared emoji and possibly derived, prepared emojis
/// (The latter one isn't used yet)
pub type PreparationResult<Prepared, Err> = Result<(Prepared, Option<Vec<(Emoji, Prepared)>>), Err>;

/// A trait that allows custom build routines for emoji sets.
///
/// Usually an `EmojiBuilder` will build an emoji font in one (or more) specific format(s), but
/// it might be used in other contexts as well.
pub trait EmojiBuilder: Send + Sync {
    /// An error type for anything that can go wrong with this `EmojiBuilder`
    type Err: Debug + Send + Sync;
    /// An emoji that has been prepared and can then be used in the building process
    type PreparedEmoji: Send + Sync;

    /// Initializes a new `EmojiBuilder` before using it.
    /// This can set up different settings and specify the working directory for the builder.
    ///
    /// The command line arguments from `clap` that have been specified by `sub_command` are
    /// passed here.
    fn new(
        build_dir: PathBuf,
        arguments: Option<ArgMatches>,
    ) -> Result<Box<Self>, Self::Err>;

    /// Called when the builder is supposed to stop its work early.
    ///
    /// The difference to the `Drop` trait is that the builder has a chance to
    /// store already prepared emojis for easier caching.
    fn finish(&mut self,
              _emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>) -> Result<(), Self::Err> {
        Ok(())
    }

    /// Lets the builder reset a build directory so that it can be reused by that builder just like
    /// it would if an empty directory was used.
    ///
    /// This reset might be done in a way that retains default files.
    /// The default implementation deletes all files and directories in the build directory
    /// without following symbolic links.
    fn reset(&self, build_dir: PathBuf) -> Result<(), ResetError<Self::Err>> {
        let errors: Vec<std::io::Error> = std::fs::read_dir(build_dir)?
            // Go over all files/dirs in the build directory
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            // ...and delete them according to their type
            .map(|entry| if entry.is_file() {
                std::fs::remove_file(entry)
            } else if entry.is_dir() {
                std::fs::remove_dir_all(entry)
            } else {
                Ok(())
            })
            // Collect all errors we got (maybe some File not found errors under Windows 10)
            .filter_map(|result| result.err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ResetError::IoErrors(errors))
        }
    }

    /// Preprocess a single emoji which will be later used to create the emoji set.
    ///
    /// This function needs to be thread-safe as the preparation might be done in parallel/concurrently.
    /// It may assume that either `prepare` hasn't been called yet for this Emoji or that either
    /// `undo` or `reset` have been called.
    fn prepare(&self, emoji: &Emoji) -> PreparationResult<Self::PreparedEmoji, Self::Err>;

    /// Builds the emoji set with the given emojis and sends the output to the specified file.
    ///
    /// Calling this function has to be performed _after_ calling `prepare` for all `Emoji`s in
    /// `emojis`.
    fn build(
        &mut self,
        emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>,
        output_file: PathBuf,
    ) -> Result<(), Self::Err>;

    /// Does the exact opposite to `prepare`, i.e. it assumes that the emoji
    /// has already been prepared and it undoes that operation (e.g. by deleting the file).
    /// It is the responsibility of the controlling code to ensure that the emoji has already been
    /// prepared, however it is possible that the builder failed in preparing that emojis, so errors
    /// need to be handled.
    ///
    /// This function can be used to do for example speculative rendering, i.e. the emojis get
    /// prepared before the user has initiated the build and "approved" them.
    ///
    /// One option would be to define an Error that marks a prepared emoji as invalidated
    fn undo(
        &self,
        _emoji: &Emoji,
        prepared: Result<Self::PreparedEmoji, Self::Err>,
    ) -> Result<Result<Self::PreparedEmoji, Self::Err>, Self::Err> {
        Ok(prepared)
    }

    /// Lets the builder define its own set of command line arguments.
    /// It is required to be able to at least call the builder from the CLI
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

/// An error wrapper that can additionally output IO errors
pub enum ResetError<T> {
    /// Wrapper for a single [std::io::Error]
    IoError(std::io::Error),
    /// Wrapper for a "normal" [crate::builder::EmojiBuilder::Err]
    BuilderError(T),
    /// Wrapper for multiple [std::io::Error]s
    IoErrors(Vec<std::io::Error>)
}

impl<T> From<std::io::Error> for ResetError<T> {
    fn from(e: std::io::Error) -> Self {
        IoError(e)
    }
}
