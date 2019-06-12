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

use std::fs::create_dir;
use std::path::PathBuf;

use clap::{App, ArgMatches, SubCommand};

use crate::builder::ResetError::IoError;
use crate::emoji::Emoji;

/// A trait that allows custom build routines for emoji sets.
///
/// Usually an `EmojiBuilder` will build an emoji font in one (or more) specific format(s), but
/// it might be used in other contexts as well.
pub trait EmojiBuilder {
    type Err;

    /// Instantiates a new `EmojiBuilder` before using it.
    /// This will set up different settings and specify the working directory for the builder.
    fn new(
        build_dir: PathBuf,
        verbose: bool,
        arguments: &ArgMatches,
    ) -> Result<Box<Self>, Self::Err>;

    /// Called when the builder is supposed to stop its work.
    ///
    /// The difference to the `Drop` trait is that
    /// this might be called before the builder is dropped and an error may be returned.
    fn finish(&self) -> Result<(), Self::Err> {
        Ok(())
    }

    /// Lets the builder reset a build directory so that it can be reused by that builder just like
    /// it would if an empty directory was used.
    ///
    /// This reset might be done in a way that retains default files.
    /// The default implementation deletes the whole directory and recreates it by a new one
    fn reset(&self, build_dir: PathBuf) -> Result<(), ResetError<Self::Err>> {
        let parent = build_dir.parent();
        match parent {
            None => Err(ResetError::NoParentError),
            Some(_) => match remove_dir_all::remove_dir_all(&build_dir) {
                Err(err) => Err(err.into()),
                Ok(_) => match create_dir(build_dir) {
                    Err(err) => Err(err.into()),
                    Ok(_) => Ok(()),
                },
            },
        }
    }

    /// Preprocess a single emoji which will be later used to create the emoji set.
    ///
    /// This function needs to be thread-safe as the preparation might be done in parallel/concurrency.
    fn prepare(&mut self, emoji: &Emoji) -> Result<(), Self::Err>;

    /// Builds the emoji set with the given emojis and sends the output to the specified file.
    ///
    /// Calling this function has to be performed _after_ calling `prepare` for all `Emoji`s in
    /// `emojis`.
    fn build<I>(&mut self, emojis: I, output_file: PathBuf) -> Result<(), Self::Err>
        where
            I: IntoIterator<Item=Emoji>;

    /// Lets the builder define its own set of command line arguments.
    /// It is required to be able to at least call the builder from the CLI
    ///
    /// The resulting argument match is returned in the `new` function.
    fn sub_command<'a, 'b>() -> App<'a, 'b>;
}

pub enum ResetError<T> {
    IoError(std::io::Error),
    BuilderError(T),
    NoParentError,
}

impl<T> From<std::io::Error> for ResetError<T> {
    fn from(e: std::io::Error) -> Self {
        IoError(e)
    }
}
