/*
 * Copyright 2021 Constantin A. <emoji.builder@c1710.de>
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
 *
 */

use std::collections::HashMap;
use crate::packs::pack::EmojiPack;
use std::path::{PathBuf, Path};
use clap::ArgMatches;
use crate::builder::EmojiBuilder;
use crate::loadable::{Loadable, LoadingError};
use std::io::Read;
use crate::configs::config_file::PackConfigFile;
use std::convert::{TryFrom, TryInto};

pub struct PackConfig {
    pub output_path: Option<PathBuf>,
    pub output_name: Option<String>,
    pub build_path: Option<PathBuf>,
    pub packs: Vec<EmojiPack>,
    pub config: HashMap<String, String>
}

impl PackConfig {
    pub fn build<B>(&self, matches: Option<ArgMatches>) -> Result<(), B::Err>
        where B: EmojiBuilder {
        let mut builder = B::new(
            self.build_path.clone().unwrap_or_else(|| PathBuf::from("build")),
            matches
        )?;

        let mut pack = EmojiPack::default();
        pack.extend(self.packs.iter());

        // Perform validation
        let (missing, additional) = pack.validate();
        missing.err().unwrap_or_default().iter()
            .for_each(|missing_emoji| warn!("Missing emoji: {} (Codepoint: {:X?}, Emoji: {})",
                                            missing_emoji,
                                            missing_emoji.sequence,
                                            missing_emoji.display_emoji()));
        additional.iter()
            .for_each(|additional_emoji| info!("Additional emoji: {} (Codepoint: {:X?}, Emoji: {})",
                                               additional_emoji,
                                               additional_emoji.sequence,
                                               additional_emoji.display_emoji()));


        let prepared = pack.emojis.iter()
            .map(|emoji| (emoji, builder.prepare(emoji)))
            .map(|(emoji, preparation_result)| (emoji, preparation_result
                .map(|(prepared, _derived)| prepared)))
            .collect();

        let current_dir = std::env::current_dir().unwrap_or_default();

        builder.build(prepared, self.output_path
            .as_ref()
            .unwrap_or(&current_dir)
            .join(self.output_name
                .as_ref()
                .map(|output_name| output_name as &str)
                .unwrap_or("font.ttf"))
            )
    }
}

impl TryFrom<PackConfigFile> for PackConfig {
    type Error = LoadingError;

    fn try_from(config_file: PackConfigFile) -> Result<Self, Self::Error> {
        match config_file.load() {
            Ok(config) => Ok(config),
            Err((_, err)) => Err(err)
        }
    }
}

impl TryFrom<Result<PackConfigFile, LoadingError>> for PackConfig {
    type Error = LoadingError;

    fn try_from(config_file: Result<PackConfigFile, LoadingError>) -> Result<Self, Self::Error> {
        match config_file.map(Self::try_from) {
            Ok(Ok(config)) => Ok(config),
            Ok(Err(err)) => Err(err),
            Err(err) => Err(err)
        }
    }
}

impl Loadable for PackConfig {
    fn from_file(file: &Path) -> Result<Self, LoadingError> {
        PackConfigFile::from_file(file).try_into()
    }

    fn from_reader<R>(reader: R) -> Result<Self, LoadingError>
        where R: Read {
        PackConfigFile::from_reader(reader).try_into()
    }
}