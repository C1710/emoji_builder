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

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{App, ArgMatches};

use crate::builder::EmojiBuilder;
use crate::changes::{CheckError, FileHashes};
use crate::emoji::Emoji;
use crate::tests::integration::builder::DummyError::{CsvError, IoError};

/// A very simple implementation of the `EmojiBuilder` trait
/// that simply computes the hashes for the emojis
/// and saves them
pub struct DummyBuilder();

#[derive(Debug)]
pub enum DummyError {
    IoError(std::io::Error),
    CheckError(CheckError),
    CsvError(csv::Error),
}

impl From<std::io::Error> for DummyError {
    fn from(err: std::io::Error) -> Self {
        IoError(err)
    }
}

impl From<CheckError> for DummyError {
    fn from(err: CheckError) -> Self {
        DummyError::CheckError(err)
    }
}

impl From<csv::Error> for DummyError {
    fn from(err: csv::Error) -> Self {
        CsvError(err)
    }
}

impl EmojiBuilder for DummyBuilder {
    type Err = DummyError;
    type PreparedEmoji = Vec<u8>;

    fn new(
        _build_dir: PathBuf,
        _arguments: Option<ArgMatches>,
    ) -> Result<Box<Self>, Self::Err> {
        Ok(Box::new(DummyBuilder()))
    }

    fn prepare(&self, emoji: &Emoji) -> Result<(Self::PreparedEmoji, Option<Vec<(Emoji, Self::PreparedEmoji)>>), Self::Err> {
        info!("Loading emoji {}.", emoji);
        let hash = FileHashes::hash(emoji);
        match hash {
            Ok(hash) => Ok((Vec::from(hash.as_slice()), None)),
            Err(err) => Err(err.into())
        }
    }

    fn build(
        &mut self,
        emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>,
        output_file: PathBuf,
    ) -> Result<(), Self::Err> {
        let mut hashes = FileHashes::new();
        for emoji in emojis {
            let hash = emoji.1;
            let emoji = emoji.0;
            info!("Saving hash for emoji {}.", emoji);
            match hash {
                Ok(hash) => hashes.update(emoji, &hash),
                // Yes, it will crash on even one error
                Err(err) => return Err(err)
            };
        }
        let wrote = hashes.write_to_path(output_file);
        match wrote {
            Ok(()) => Ok(()),
            Err(err) => Err(err.into())
        }
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        App::new("dummy")
    }
}
