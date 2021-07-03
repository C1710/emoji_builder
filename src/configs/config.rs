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
use std::convert::TryFrom;
use serde::Deserialize;
use crate::loadables::sources::LoadableSource;
use crate::loadables::loadable::Loadable;
use itertools::{Itertools, Either};
use crate::loadables::NoError;

pub struct PackConfig {
    pub output_path: Option<PathBuf>,
    pub output_name: Option<String>,
    pub build_path: Option<PathBuf>,
    pub packs: Vec<EmojiPack>,
    pub config: HashMap<String, String>
}

#[derive(Deserialize, Clone, Debug)]
struct PackConfigPrototype {
    output_path: Option<PathBuf>,
    output_name: Option<String>,
    build_path: Option<PathBuf>,
    packs: Vec<PathBuf>,
    config: HashMap<String, String>
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

impl<S> TryFrom<(PackConfigPrototype, S)> for PackConfig
    where S: LoadableSource {
    type Error = NoError;

    fn try_from((prototype, source): (PackConfigPrototype, S)) -> Result<Self, Self::Error> {
        let output_path = prototype.output_path.map(|output_path| relate_path(
                source.root(),
                output_path.as_path())
        );
        let output_name = prototype.output_name;
        let build_path = prototype.build_path.map(|build_path| relate_path(
            source.root(),
            build_path.as_path()
        ));
        let (packs, errors): (Vec<_>, Vec<_>) = prototype.packs
            .iter()
            .filter_map(|pack_path| source.request_source(pack_path).ok())
            .map(|pack_source| EmojiPack::load(pack_source))
            .partition_map(|pack_result| match pack_result {
                Ok(pack) => Either::Left(pack),
                Err(error) => Either::Right(error)
            });
        let config = prototype.config;

        // TODO: Error handling
        errors.iter()
            .for_each(|error| error!("{:?}", error));

        Ok(Self {
            output_path,
            output_name,
            build_path,
            packs,
            config
        })
    }
}

fn relate_path(base_dir: &Path, target_path: &Path) -> PathBuf {
    // We use has_root here instead of is_absolute since otherwise
    // \file would be equal to .\file on Windows, which does not seem to be the usual expected
    // behavior.
    if !target_path.has_root() {
        // In this case, we assume, the path is relative in which case, we'll append it to the
        // root_dir and canonicalize it
        base_dir.join(&target_path)
    } else {
        target_path.to_path_buf()
    }
}