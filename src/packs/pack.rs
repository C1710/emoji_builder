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

// #[serde(default)]
//     name: String,
//     #[serde(default)]
//     unicode_version: (u32, u32),
//     #[serde(default)]
//     table_paths: Vec<PathBuf>,
//     #[serde(default)]
//     test_paths: Vec<PathBuf>,
//     #[serde(default)]
//     emoji_dirs: Vec<PathBuf>,
//     #[serde(default)]
//     flag_dirs: Vec<PathBuf>,
//     #[serde(default)]
//     offline: bool,
//     #[serde(default)]
//     config: HashMap<String, String>

use crate::emoji_tables::EmojiTable;
use std::collections::{HashSet, HashMap};
use crate::emojis::emoji::{Emoji, EmojiError};
use std::path::{Path, PathBuf};
use std::iter::{FromIterator, Map};
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelIterator, ParallelExtend};
use itertools::{Itertools, Either};
use std::hash::Hash;
use std::cmp::max;
use std::sync::Mutex;
use std::cell::Cell;
use std::convert::TryFrom;
use std::io::{Read, BufReader};
use crate::loadables::sources::LoadableSource;
use serde::Deserialize;
use crate::loadables::NoError;

#[derive(Debug, Default)]
pub struct EmojiPack {
    pub name: Option<String>,
    pub unicode_version: Option<(u32, u32)>,
    pub table: EmojiTable,
    pub emojis: HashSet<Emoji>,
    pub config: HashMap<String, String>
}

#[derive(Deserialize, Debug)]
pub struct EmojiPackPrototype {
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

impl EmojiPack {
    pub fn emojis(&self) -> &HashSet<Emoji> {
        &self.emojis
    }

    pub fn emojis_mut(&mut self) -> &mut HashSet<Emoji> {
        &mut self.emojis
    }

    pub fn emojis_vec(&self) -> Vec<Emoji> {
        self.emojis.iter().cloned().collect()
    }

    pub fn emojis_sorted(&self) -> Vec<Emoji> {
        let mut emojis = self.emojis.iter().cloned().collect_vec();
        // Sort_unstable and stable are identical as we don't have any duplicate sequences, thanks
        // to the HashSet used previously
        emojis.sort_unstable();
        emojis
    }

    pub fn table(&self) -> &EmojiTable {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut EmojiTable {
        &mut self.table
    }

    pub fn config(&self) -> &HashMap<String, String> {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.config
    }

    pub fn push(&mut self, other: EmojiPack) {
        if let Some(name) = self.name.as_mut() {
            if let Some(other_name) = other.name.as_ref() {
                name.push_str(" + ");
                name.push_str(other_name);
            }
        }
        self.unicode_version = max(self.unicode_version, other.unicode_version);
        self.table.extend(other.table);
        self.emojis.extend(other.emojis.into_iter());
        self.config.extend(other.config.into_iter());
    }
    
    pub fn push_ref(&mut self, other: &EmojiPack) {
        if let Some(name) = self.name.as_mut() {
            if let Some(other_name) = other.name.as_ref() {
                name.push_str(" + ");
                name.push_str(other_name);
            }
        }
        self.unicode_version = max(self.unicode_version, other.unicode_version);
        self.table.extend(other.table.clone());
        self.emojis.extend(other.emojis.iter().cloned());
        self.config.extend(other.config.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
        );
    }

    pub fn push_low_importance(&mut self, other: EmojiPack) {
        self.table.extend_preserve_own(other.table);
        self.unicode_version = max(self.unicode_version, other.unicode_version);
        self.emojis = other.emojis.into_iter().chain(self.emojis.drain().into_iter()).collect();
        self.config = other.config.into_iter().chain(self.config.drain().into_iter()).collect();
    }
    
    pub fn validate(&self) -> (Result<(), Vec<Emoji>>, Vec<Emoji>) {
        let sequences = self.emojis.iter()
            .map(|emoji| emoji.sequence.clone())
            .collect();
        self.table.validate(&sequences, false)
    }
}

macro_rules! print_errors {
    ($($errors:ident),*) => {
        $(
            $errors.iter()
                .for_each(|error| error!("{:?}", error));
        )*

    };
}


impl<S> TryFrom<(EmojiPackPrototype, S)> for EmojiPack
    where S: LoadableSource {
    type Error = NoError;

    fn try_from((prototype, source): (EmojiPackPrototype, S)) -> Result<Self, Self::Error> {
        let table = load_tables(
            prototype.table_files.unwrap_or_default(),
            prototype.test_files.unwrap_or_default(),
            &source,
            prototype.offline,
            prototype.unicode_version
        ).unwrap_or_default();
        let (emojis, emoji_errors, source_errors) = load_emojis_from_source(&source, prototype.emoji_dirs.unwrap_or_default(), Some(&table), false);
        let (flags, flag_errors, flag_source_errors) = load_emojis_from_source(&source, prototype.flag_dirs.unwrap_or_default(), Some(&table), true);

        let emojis: HashSet<_> = emojis.into_iter().chain(flags.into_iter()).collect();

        let config = prototype.config.unwrap_or_default();

        let unicode_version = prototype.unicode_version;
        let name = prototype.name;

        // TODO: Error handling?
        print_errors!(source_errors, flag_source_errors, emoji_errors, flag_errors);

        Ok(Self {
            name,
            unicode_version,
            table,
            emojis,
            config
        })
    }
}

fn load_emojis_from_source<S>(
    source: &S,
    emoji_dirs: Vec<PathBuf>,
    table: Option<&EmojiTable>,
    flags: bool) -> (Vec<Emoji>, Vec<EmojiError>, Vec<S::Error>)
    where S: LoadableSource {
    // Doing this all in one step is way to complicated...
    let (emoji_dirs, source_errors): (Vec<_>, Vec<_>) = emoji_dirs.iter()
        .map(|emoji_dir| source.request_source(emoji_dir))
        .map(|emoji_dir| emoji_dir.map(|emoji_dir| emoji_dir.contents()))
        .partition_map(|result| match result {
            Ok(Ok(emoji_dir)) => Either::Left(emoji_dir),
            Ok(Err(error)) => Either::Right(error),
            Err(error) => Either::Right(error)
        });

    let (emojis, emoji_errors) = emoji_dirs.iter()
        .map(|emojis| emojis.into_iter())
        .map(|emojis| emojis.filter_map(|emoji_source|
            emoji_source.root_file()
        ))
        .flatten()
        .map(|emoji| Emoji::from_path(
            emoji.to_path_buf(),
            table,
            flags
        ))
        .partition_map(|emoji_result| match emoji_result {
            Ok(emoji) => Either::Left(emoji),
            Err(error) => Either::Right(error)
        });
    (emojis, emoji_errors, source_errors)
}

fn load_tables<S>(table_files: Vec<PathBuf>,
                  test_files: Vec<PathBuf>,
                  source: &S,
                  offline: Option<bool>,
                  unicode_version: Option<(u32, u32)>)
    -> Result<EmojiTable, (Vec<S::Error>, Option<crate::emoji_tables::ExpansionError>)>
    where S: LoadableSource {

    let mut table = EmojiTable::new();

    let table_paths = table_files.iter()
        .map(|path| (false, path));
    let test_paths = test_files.iter()
        .map(|path| (true, path));

    // TODO: Parallelize
    // When compiling this without the online-feature, the mut becomes unused.
    // It would however be overly complicated to conditionally remove it here
    #[allow(unused_mut)]
        let mut read_errors: Vec<_> = table_paths.chain(test_paths)
        .map(|(is_test, path)| (is_test, source.request(path)))
        .map(|(is_test, reader)| (is_test, reader.map(BufReader::new)))
        .filter_map(|(is_test, reader)| match reader {
            Ok(reader) => {
                    if is_test {
                        table.expand_descriptions_from_test_data(reader);
                    } else {
                        table.expand(reader);
                    }
                    None
                },
            Err(error) => Some(error)
        })
        .collect();

    let mut expansion_error = None;

    #[cfg(feature = "online")]
    if !offline.unwrap_or(false) {
        if let Some(unicode_version) = unicode_version {
            let expansion_result = table.expand_all_online(unicode_version);
            if let Err(error) = expansion_result {
                expansion_error = Some(error);
            }
        }
    }

    if !read_errors.is_empty() || expansion_error.is_some() {
        Err((read_errors, expansion_error))
    } else {
        Ok(table)
    }
}

impl FromIterator<EmojiPack> for EmojiPack {
    fn from_iter<T: IntoIterator<Item=EmojiPack>>(iter: T) -> Self {
        let mut base_pack = EmojiPack::default();
        iter.into_iter().for_each(|pack| base_pack.push(pack));
        base_pack
    }
}

impl Extend<EmojiPack> for EmojiPack {
    fn extend<T: IntoIterator<Item=EmojiPack>>(&mut self, iter: T) {
        iter.into_iter().for_each(|pack| self.push(pack));
    }
}

impl<'a> Extend<&'a EmojiPack> for EmojiPack {
    fn extend<T: IntoIterator<Item=&'a EmojiPack>>(&mut self, iter: T) {
        iter.into_iter().for_each(|pack| self.push_ref(pack));
    }
}

impl ParallelExtend<EmojiPack> for EmojiPack {
    fn par_extend<I>(&mut self, par_iter: I) where
        I: IntoParallelIterator<Item=EmojiPack> {
        let mutex = Mutex::new(Cell::new(self));
        // Using interior mutability here is okay as we use a Mutex to ensure that only one thread
        // can update self at a time
        par_iter.into_par_iter().for_each(|pack| mutex.lock().unwrap().get_mut().push_low_importance(pack));
    }
}

impl FromParallelIterator<EmojiPack> for EmojiPack {
    fn from_par_iter<I>(par_iter: I) -> Self where
        I: IntoParallelIterator<Item=EmojiPack> {
        let mut pack = EmojiPack::default();
        pack.par_extend(par_iter);
        pack
    }
}

impl <S> ParallelExtend<(S, EmojiPack)> for EmojiPack
    where S: Ord + Sync + Send + Hash + Copy {
    fn par_extend<I>(&mut self, par_iter: I) where
        I: IntoParallelIterator<Item=(S, EmojiPack)> {
        let packs: HashMap<S, EmojiPack> = par_iter.into_par_iter().collect();
        self.extend(
            packs.into_iter()
                .sorted_by_key(|(index, _)| *index)
                .map(|(_, pack)| pack)
        );
    }
}

impl <S> FromParallelIterator<(S, EmojiPack)> for EmojiPack
    where S: Ord + Sync + Send + Hash + Copy {
    fn from_par_iter<I>(par_iter: I) -> Self where
        I: IntoParallelIterator<Item=(S, EmojiPack)> {
        let packs: HashMap<S, EmojiPack> = par_iter.into_par_iter().collect();
        packs.into_iter()
            .sorted_by_key(|(index, _)| *index)
            .map(|(_, pack)| pack)
            .collect()
    }
}