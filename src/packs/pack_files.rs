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
use std::io::{BufReader, Error, BufRead};
use itertools::{Itertools, Either};
use crate::emoji::{EmojiError, Emoji};
use crate::emoji_tables::{EmojiTable, ExpansionError};
use crate::packs::pack::EmojiPack;
use regex::Regex;


#[derive(Deserialize, Default, Debug)]
pub struct EmojiPackFile {
    name: Option<String>,
    unicode_version: Option<(u32, u32)>,
    table_paths: Option<Vec<PathBuf>>,
    test_paths: Option<Vec<PathBuf>>,
    emoji_dirs: Option<Vec<PathBuf>>,
    flag_dirs: Option<Vec<PathBuf>>,
    aliases: Option<PathBuf>,
    config: Option<HashMap<String, String>>
}

pub enum LoadingError {
    Io(std::io::Error),
    Multiple(Vec<LoadingError>),
    Emoji(EmojiError),
    Serde(Box<dyn std::error::Error>),
    MissingParameter,
    #[cfg(feature = "online")]
    Reqwest(reqwest::Error)
}

pub type ResultAnyway<T, E> = Result<T, (T, E)>;

const EMOJI_SEQUENCE_UNDERSCORE: &str = r"(([A-Fa-f0-9]{1,8})([_\s][A-Fa-f0-9]{1,8})*)";

impl EmojiPackFile {
    pub fn from_file(file: &Path) -> Result<Self, LoadingError> {
        let reader = std::fs::File::open(file)?;
        let reader = BufReader::new(reader);
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let result = Self::deserialize(&mut deserializer).map_err(|err| LoadingError::Serde(Box::new(err)));
        if let Ok(mut pack) = result {
            if let Some(parent) = file.parent() {
                pack.normalize_paths(parent);
            }
            Ok(pack)
        } else {
            result
        }
    }

    pub fn load_tables(&self) -> ResultAnyway<EmojiTable, LoadingError> {
        let no_path = vec![];
        let mut table = EmojiTable::new();
        let table_paths = self.table_paths.as_ref().unwrap_or(&no_path);
        let test_paths = self.test_paths.as_ref().unwrap_or(&no_path);

        let table_paths = table_paths.iter()
            .map(|path| (false, path));
        let test_paths = test_paths.iter()
            .map(|path| (true, path));

        // TODO: Parallelize
        let expansion_errors: Vec<LoadingError> = table_paths.chain(test_paths)
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

        /*
        #[cfg(feature = "online")]
        if !self.offline.unwrap_or(false) {
            if let Some(unicode_version) = self.unicode_version {
                let expansion_result = table.expand_all_online(unicode_version);
                if let Err(error) = expansion_result {
                    expansion_errors.push(error.into())
                }
            }
        }
         */

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
            Err(LoadingError::MissingParameter.into())
        }
    }

    pub fn load(self) -> ResultAnyway<EmojiPack, LoadingError> {
        let table = self.load_tables();
        let emojis = self.load_emojis(table.as_ref().ok());
        let mut errors = Vec::with_capacity(2);
        let table = table.unwrap_or_else(|(table, error)| {
            errors.push(error);
            table
        });
        let mut emojis = emojis.unwrap_or_else(|(emojis, error)| {
            errors.push(error);
            emojis
        });
        
        let aliases = self.load_aliases().unwrap_or_else(|(aliases, error)| {
            errors.push(error);
            aliases
        });
        
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
        if let Some(mut path) = self.table_paths.as_mut() {
            normalize_paths(&mut path, root_dir)
        }
        if let Some(mut path) = self.test_paths.as_mut() {
            normalize_paths(&mut path, root_dir)
        }
        if let Some(mut path) = self.emoji_dirs.as_mut() {
            normalize_paths(&mut path, root_dir)
        }
        if let Some(mut path) = self.flag_dirs.as_mut() {
            normalize_paths(&mut path, root_dir)
        }
    }
}

fn normalize_paths(target_paths: &mut Vec<PathBuf>, root_dir: &Path) {
    target_paths.iter_mut()
        .for_each(|path| normalize_path(path, root_dir));
}

fn normalize_path(target_path: &mut PathBuf, root_dir: &Path) {
    if !target_path.has_root() {
        // In this case, we assume, the path is relative in which case, we'll append it to the
        // root_dir and canonicalize it
        *target_path = root_dir.join(&target_path);
    }
}

impl From<std::io::Error> for LoadingError {
    fn from(err: Error) -> Self {
        Self::Io(err)
    }
}

impl From<EmojiError> for LoadingError {
    fn from(err: EmojiError) -> Self {
        Self::Emoji(err)
    }
}

impl<T> From<Vec<T>> for LoadingError
    where T: Into<LoadingError> {
    fn from(errs: Vec<T>) -> Self {
        Self::Multiple(errs.into_iter().map(|err| err.into()).collect())
    }
}

impl From<ExpansionError> for LoadingError {
    fn from(err: ExpansionError) -> Self {
        match err {
            ExpansionError::Io(err) => Self::Io(err),
            ExpansionError::Multiple(err) => err.into(),
            #[cfg(feature = "online")]
            ExpansionError::Reqwest(err) => Self::Reqwest(err)
        }
    }
}

impl<T> From<LoadingError> for (T, LoadingError)
    where T: Default {
    fn from(error: LoadingError) -> Self {
        (T::default(), error)
    }
}