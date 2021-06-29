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

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{PathBuf, Path};
use std::io::{BufReader, BufRead, Read};
use itertools::{Itertools, Either};
use crate::emoji::Emoji;
use crate::emoji_tables::EmojiTable;
use crate::packs::pack::EmojiPack;
use regex::Regex;
use crate::loadable::{Loadable, LoadingError, LoadableImpl, normalize_paths, ResultAnyway};


#[derive(Deserialize, Default, Debug)]
pub struct EmojiPackFile {
    name: Option<String>,
    unicode_version: Option<(u32, u32)>,
    table_files: Option<Vec<PathBuf>>,
    test_files: Option<Vec<PathBuf>>,
    emoji_dirs: Option<Vec<PathBuf>>,
    flag_dirs: Option<Vec<PathBuf>>,
    aliases: Option<PathBuf>,
    config: Option<HashMap<String, String>>,
    offline: Option<bool>,
}



const EMOJI_SEQUENCE_UNDERSCORE: &str = r"(([A-Fa-f0-9]{1,8})([_\s][A-Fa-f0-9]{1,8})*)";

impl EmojiPackFile {
    pub fn load_tables(&self) -> ResultAnyway<EmojiTable, LoadingError> {
        let no_path = vec![];
        let mut table = EmojiTable::new();
        let table_paths = self.table_files.as_ref().unwrap_or(&no_path);
        let test_paths = self.test_files.as_ref().unwrap_or(&no_path);

        let table_paths = table_paths.iter()
            .map(|path| (false, path));
        let test_paths = test_paths.iter()
            .map(|path| (true, path));

        // TODO: Parallelize
        // When compiling this without the online-feature, the mut becomes unused.
        // It would however be overly complicated to conditionally remove it here
        #[allow(unused_mut)]
        let mut expansion_errors: Vec<LoadingError> = table_paths.chain(test_paths)
            .map(|(is_test, path)| (is_test, std::fs::File::open(path)))
            .map(|(is_test, file)| (is_test, file.map(BufReader::new)))
            // TODO: Use flatten once it's stabilized
            .map(|(is_test, reader)| match reader.map(|reader| if is_test {
                table.expand_descriptions_from_test_data(reader)
            } else {
                table.expand(reader)
            }) {
                Err(err) => Err(err),
                Ok(Err(err)) => Err(err),
                Ok(Ok(ok)) => Ok(ok)
            })
            .filter_map(|result| result.err())
            .map(|error| error.into())
            .collect();

        #[cfg(feature = "online")]
        if !self.offline.unwrap_or(false) {
            if let Some(unicode_version) = self.unicode_version {
                let expansion_result = table.expand_all_online(unicode_version);
                if let Err(error) = expansion_result {
                    expansion_errors.push(error.into())
                }
            }
        }

        if !expansion_errors.is_empty() {
            Err((table, expansion_errors.into()))
        } else {
            Ok(table)
        }
    }

    /*
    #[cfg(feature = "online")]
    pub fn load_table_online(&self) -> Result<EmojiTable, LoadingError> {
        if let Some(unicode_version) = self.unicode_version {
            EmojiTable::load_online(unicode_version).map_err(|error| error.into())
        } else {
            Err(LoadingError::MissingParameter)
        }
    }
     */

    pub fn load_emojis(&self, table: Option<&EmojiTable>) -> ResultAnyway<HashSet<Emoji>, LoadingError> {
        let no_paths = vec![];
        let emoji_dirs = self.emoji_dirs.as_ref().unwrap_or(&no_paths);
        let flag_dirs = self.flag_dirs.as_ref().unwrap_or(&no_paths);

        let dirs = emoji_dirs.iter().map(|emoji_dir| (false, emoji_dir))
            .chain(flag_dirs.iter().map(|flag_dir| (true, flag_dir)));

        // TODO: Parallelize
        let (emojis, errors): (HashSet<_>, Vec<_>) = dirs
            .filter_map(|(contains_flags, dir)| Some(contains_flags).zip(dir.read_dir().ok()))
            .map(|(contains_flags, dir)| dir.map(move |entry| (contains_flags, entry)))
            .flatten()
            .filter_map(|(is_flag, entry)| Some(is_flag).zip(entry.ok().map(|entry| entry.path())))
            .filter(|(_is_flag, entry)| entry.is_file())
            .map(|(is_flag, path)| Emoji::from_path(path, table, is_flag))
            .partition_map(|emoji_result| match emoji_result {
                Ok(emoji) => Either::Left(emoji),
                Err(err) => Either::Right(err)
            });

        if !errors.is_empty() {
            Err((emojis, errors.into()))
        } else {
            Ok(emojis)
        }
    }

    pub fn load_aliases(&self) -> ResultAnyway<HashMap<Emoji, Emoji>, LoadingError> {
        lazy_static! {
            static ref ALIASES_REGEX: Regex = Regex::new(&format!(r"(?P<from>{})\s*;\s*(?P<to>{})\s*#.*",
                                                      EMOJI_SEQUENCE_UNDERSCORE,
                                                      EMOJI_SEQUENCE_UNDERSCORE)).unwrap();
        }
        if let Some(aliases) = self.aliases.as_ref() {
            let reader = std::fs::File::open(aliases).map_err(LoadingError::from)?;
            let reader = BufReader::new(reader);
            let aliases = reader.lines()
                .filter_map(|line| line.ok())
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .filter_map(|line| (&*ALIASES_REGEX).captures(&line)
                    .map(|capture| (
                        Emoji::from_sequence(capture.name("from").unwrap().as_str(), None).unwrap(),
                        Emoji::from_sequence(capture.name("to").unwrap().as_str(), None).unwrap()
                    ))
                )
                .collect();
            Ok(aliases)
        } else {
            Err((HashMap::new(), LoadingError::MissingParameter))
        }
    }

    pub fn load(self) -> ResultAnyway<EmojiPack, LoadingError> {
        let mut errors = Vec::with_capacity(8);

        let table = self.load_tables();

        let emojis = if self.emoji_dirs.is_some() {
            self.load_emojis(table.as_ref().ok())
        } else {
            Ok(HashSet::new())
        };

        let table = table.unwrap_or_else(|(table, error)| {
            errors.push(error);
            table
        });

        let mut emojis = emojis
            .unwrap_or_else(|(emojis, error)| {
                errors.push(error);
                emojis
            }
        );

        let aliases = if self.aliases.is_some() {
            self.load_aliases().unwrap_or_else(|(aliases, error)| {
                errors.push(error);
                aliases
            })
        } else {
            HashMap::new()
        };
        
        aliases.into_iter()
            .map(|(from, to)| to.alias(from.sequence))
            // We don't want to make an already existing emoji an alias of another one
            .for_each(|emoji| if !emojis.contains(&emoji) {
                emojis.insert(emoji);
            });
        
        let pack = EmojiPack {
            name: self.name,
            unicode_version: self.unicode_version,
            table,
            emojis,
            config: self.config.unwrap_or_default(),
        };

        if errors.is_empty() {
            Ok(pack)
        } else {
            Err((pack, errors.into()))
        }
    }

    fn normalize_paths(&mut self, root_dir: &Path) {
        if let Some(mut path) = self.table_files.as_mut() {
            normalize_paths(&mut path, root_dir);
        }
        if let Some(mut path) = self.test_files.as_mut() {
            normalize_paths(&mut path, root_dir);
        }
        if let Some(mut path) = self.emoji_dirs.as_mut() {
            normalize_paths(&mut path, root_dir);
        }
        if let Some(mut path) = self.flag_dirs.as_mut() {
            normalize_paths(&mut path, root_dir);
        }
    }
}

impl Loadable for EmojiPackFile {
    fn from_file(file: &Path) -> Result<Self, LoadingError> {
        let reader = std::fs::File::open(file)?;
        let reader = BufReader::new(reader);
        let result = Self::from_reader(reader);
        if let Ok(mut pack) = result {
            if let Some(parent) = file.parent() {
                pack.normalize_paths(parent);
            }
            Ok(pack)
        } else {
            result
        }
    }

    fn from_reader<R>(reader: R) -> Result<Self, LoadingError> where R: Read {
        Self::from_reader_impl(reader)
    }
}