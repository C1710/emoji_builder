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
use crate::emoji::Emoji;
use std::path::Path;
use std::iter::FromIterator;
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelIterator, ParallelExtend};
use itertools::Itertools;
use std::hash::Hash;
use std::cmp::max;
use crate::packs::pack_files::EmojiPackFile;
use std::sync::Mutex;
use std::cell::Cell;
use crate::loadable::{LoadingError, Loadable};
use std::convert::{TryFrom, TryInto};
use std::io::Read;

#[derive(Debug, Default)]
pub struct EmojiPack {
    pub name: Option<String>,
    pub unicode_version: Option<(u32, u32)>,
    pub table: EmojiTable,
    pub emojis: HashSet<Emoji>,
    pub config: HashMap<String, String>
}

impl EmojiPack {
    pub fn from_file_anyway(path: &Path) -> Result<Self, (Option<Self>, LoadingError)> {
        match EmojiPackFile::from_file(path)
            .map(|pack_file| pack_file.load()) {

            Ok(Ok(pack)) => Ok(pack),
            Ok(Err((pack, err))) => Err((Some(pack), err)),
            Err(err) => Err((None, err))
        }
    }

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

impl Loadable for EmojiPack {
    fn from_file(file: &Path) -> Result<Self, LoadingError> {
        EmojiPackFile::from_file(file).try_into()
    }

    fn from_reader<R>(reader: R) -> Result<Self, LoadingError> where R: Read {
        EmojiPackFile::from_reader(reader).try_into()
    }
}

impl TryFrom<EmojiPackFile> for EmojiPack {
    type Error = LoadingError;

    fn try_from(pack_file: EmojiPackFile) -> Result<Self, Self::Error> {
        match pack_file.load() {
            Err((_pack, err)) => Err(err),
            Ok(pack) => Ok(pack),
        }
    }
}

impl TryFrom<Result<EmojiPackFile, LoadingError>> for EmojiPack {
    type Error = LoadingError;

    fn try_from(pack_file: Result<EmojiPackFile, LoadingError>) -> Result<Self, Self::Error> {
        match pack_file.map(Self::try_from) {
            Ok(Ok(pack)) => Ok(pack),
            Ok(Err(err)) => Err(err),
            Err(err) => Err(err)
        }
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