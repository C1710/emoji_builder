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
//! A module that allows to easily parse [Unicode® emoji data tables][unicode]
//! (or tables in a similar format) into lookup tables and work with them.
//!
//! [unicode]: https://unicode.org/Public/emoji/13.0/

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::path::Path;
use std::str::FromStr;

use bimap::BiHashMap;
use itertools::Itertools;

use crate::emojis::emoji::Emoji;
use crate::emojis::emoji_kind::EmojiKind;
use crate::emojis::emoji_status::EmojiStatus;
#[cfg(feature = "online")]
use crate::tables::errors::ExpansionError;
#[cfg(feature = "online")]
use crate::tables::online::{DATA_FILES, EMOJI_TEST};
#[cfg(feature = "online")]
use crate::tables::online;
use crate::tables::utils;
use std::ops::{AddAssign, Add};
use std::cmp::max;
use std::hash::Hash;
use std::borrow::Borrow;
use crate::tables::regexes::{match_line, EmojiFileEntry, EmojiDataCodepoints};
use std::convert::TryFrom;
use crate::tables::prototype::EmojiTablePrototype;
use crate::loadables::NoError;

/// A code sequence
pub type EmojiTableKey = Vec<u32>;
// The EmojiKinds and optionally a description/name and a possible status of an emoji
pub type EmojiTableEntry = (Vec<EmojiKind>, Option<String>, Vec<EmojiStatus>);


/// An internal representation of one or more Unicode® emoji data tables
/// <https://unicode.org/Public/emoji/12.0/>
/// It maps emoji code sequences to their kind and (if given) a description/name.
#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Eq)]
#[derive(Clone)]
pub struct EmojiTable {
    table: HashMap<EmojiTableKey, EmojiTableEntry>,
    names: HashMap<String, EmojiTableKey>,
    fe0f_table: BiHashMap<EmojiTableKey, EmojiTableKey>,
}

impl EmojiTable {
    // -- Create new tables --

    /// Creates a new, empty emoji table
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
            names: HashMap::new(),
            fe0f_table: BiHashMap::new()
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            table: HashMap::with_capacity(capacity),
            names: HashMap::with_capacity(capacity),
            fe0f_table: BiHashMap::with_capacity(capacity / 3)
        }
    }

    /// Reads multiple files which are formatted in the same way as the Unicode® emoji data tables
    /// (See <https://unicode.org/Public/emoji/12.0/>) and builds a lookup table
    /// to gather additional metadata for emojis.
    ///
    /// **Important** Currently, names are only extracted from emoji-test.txt-like files
    /// # Examples:
    /// ```
    /// use std::path::PathBuf;
    /// use emoji_builder::tables::emoji_tables::EmojiTable;
    /// use std::collections::HashMap;
    /// use emoji_builder::emojis::emoji_kind::EmojiKind::EmojiZwjSequence;
    ///
    /// // Contains the entry
    /// // 1F3F3 FE0F 200D 1F308 ; Emoji_ZWJ_Sequence  ; rainbow flag #  7.0  [1] (🏳️‍🌈)
    /// let path = PathBuf::from("test_files/tables/emoji-zwj-sequences.txt");
    /// let paths = vec![path];
    ///
    /// let table = EmojiTable::from_files(&paths).unwrap();
    ///
    /// let rainbow = vec![0x1f3f3, 0xfe0f, 0x200d, 0x1f308];
    /// let rainbow_no_fe0f = vec![0x1f3f3, 0x200d, 0x1f308];
    ///
    /// let rainbow_entry = (vec![EmojiZwjSequence], None, vec![]);
    ///
    /// assert!(table.as_ref().contains_key(&rainbow));
    /// // Versions without FE0F are _not_ included anymore
    /// assert!(!table.as_ref().contains_key(&rainbow_no_fe0f));
    ///
    /// assert_eq!(rainbow_entry, *table.get(&rainbow).unwrap());
    /// ```
    pub fn from_files<P: AsRef<Path>>(paths: &[P]) -> Result<EmojiTable, Error> {
        let mut table = EmojiTable::new();

        for path in paths {
            EmojiTable::expand_from_file(&mut table, path)?;
        }
        Ok(table)
    }


    // -- Extend a table with multiple entries --

    /// Expands the table with the contents of an emoji table-file with  the syntax of e.g.
    /// `emoji-data.txt`.
    /// Only the emoji itself and its kind(s) is/are extended.
    /// Names are extended from `emoji-test.txt`-like files, using [EmojiTable::expand_descriptions_from_test_data]
    #[deprecated]
    pub fn expand<I: BufRead>(&mut self, reader: I) {
        Extend::extend(self, reader.lines().flatten())
    }

    /// Adds the entries from another Unicode® emoji data table-like file to an existing EmojiTable.
    /// # Duplicates
    /// If there are more than two entries for one emoji (sequence), the entry (i.e. Emoji kinds and description)
    /// will be updated as follows:
    /// ## Emoji kind
    /// The `EmojiKind` vector will be updated to include the new kind found in this entry.
    /// ## Description
    /// Currently, descriptions will not be used
    /// # Examples
    /// ```
    /// use emoji_builder::tables::emoji_tables::EmojiTable;
    /// use emoji_builder::emojis::emoji_kind::EmojiKind;
    /// use std::path::PathBuf;
    ///
    /// let mut table = EmojiTable::new();
    ///
    /// let path = &PathBuf::from("test_files/tables/emoji-zwj-sequences.txt");
    /// table.expand_from_file(path).unwrap();
    ///
    /// let rainbow = vec![0x1f3f3, 0xfe0f, 0x200d, 0x1f308];
    /// let rainbow_no_fe0f = vec![0x1f3f3, 0x200d, 0x1f308];
    ///
    /// let rainbow_entry = (vec![EmojiKind::EmojiZwjSequence], None, vec![]);
    ///
    /// assert!(table.as_ref().contains_key(&rainbow));
    /// assert!(!table.as_ref().contains_key(&rainbow_no_fe0f));
    ///
    /// assert_eq!(rainbow_entry, *table.get(&rainbow).unwrap());
    /// ```
    pub fn expand_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        self.extend(reader.lines().flatten());
        Ok(())
    }

    /// Uses the names of the emoji-test.txt files.
    /// These seem to be more suitable than emoji-data.txt as they don't include any emoji character
    /// ranges.
    /// An example would be <https://unicode.org/Public/emoji/13.0/emoji-test.txt>.
    ///
    /// _Please note that this parser is extremely **strict** and will crash if something is wrong
    /// with the syntax_
    ///
    /// The syntax of these files is:
    /// `Codepoint ; ("component"|"fully-qualified"|"minimally-qualified"|"unqualified") # Emoji "E"Version Emoji name`
    #[deprecated]
    pub fn expand_descriptions_from_test_data<I: BufRead>(&mut self, reader: I) {
        self.extend(reader.lines().flatten())
    }

    /// Populates the table with fresh data from the internet for the given version.
    /// # Arguments
    /// - `version`: the main and sub version of the desired emoji set (e.g. `(13, 0)` for Emoji 13.0
    ///   or `(12, 1)` for Emoji 12.1).
    /// # Data sources
    /// It will load the following files from `https://unicode.org/Public/emoji/<version>`
    /// (e.g. `https://unicode.org/Public/emoji/13.0`):
    /// - `emoji-data.txt`: The main list of single emoji codepoints.
    /// - `emoji-sequences.txt`: All sequences of codepoints _without_ the `U+200D` character.
    /// - `emoji-zwj-sequences.txt`: All sequences of codepoints _with_ the `U+200D` character.
    /// - `emoji-test.txt`: This file will be used to get the names of all emojis.
    #[cfg(feature = "online")]
    pub fn expand_all_online(&mut self, version: (u32, u32)) -> Result<(), ExpansionError> {
        let client_builder = reqwest::blocking::ClientBuilder::new();
        let client = client_builder.build()?;

        let test_expansion_result = self.expand_descriptions_from_test_online(&client, version);

        let errors: Vec<_> = DATA_FILES.iter()
            .map(|file| self.expand_data_online(&client, version, file))
            .chain(vec![test_expansion_result])
            .filter_map(|result| result.err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.into())
        }
    }

    #[cfg(feature = "online")]
    fn expand_data_online(&mut self, client: &reqwest::blocking::Client, version: (u32, u32), file: &'static str) -> Result<(), ExpansionError> {
        let reader = online::get_data_file_online(client, version, file)?;
        self.extend(reader.lines().flatten());
        Ok(())
    }

    #[cfg(feature = "online")]
    fn expand_descriptions_from_test_online(&mut self, client: &reqwest::blocking::Client, version: (u32, u32)) -> Result<(), ExpansionError> {
        let reader = online::get_data_file_online(client, version, EMOJI_TEST)?;
        self.extend(reader.lines().flatten());
        Ok(())
    }


    // -- Add single items --

    /// Inserts a new key-entry pair into the table and returns the last entry if there was one.
    /// This is simply passed on to the internal `HashMap`.
    /// Please be aware that no name-key-mapping is inserted.
    /// That means:
    /// ```
    /// use emoji_builder::tables::emoji_tables::EmojiTable;
    ///
    /// let name = "thinking face";
    /// let codepoint = vec![0x1f914];
    /// let mut table = EmojiTable::new();
    /// table.insert(codepoint.clone(), (vec![], Some(name.to_string()), vec![]));
    ///
    /// // We can't find the emoji by its name!
    /// assert_eq!(table.get_by_name(name), None);
    /// ```
    pub fn insert(&mut self, key: EmojiTableKey, entry: EmojiTableEntry) -> Option<EmojiTableEntry> {
        if key.contains(&0xfe0f) {
            self.fe0f_table.insert(key.clone(), utils::strip_fe0f(&key));
        }
        self.table.insert(key, entry)
    }

    /// Inserts a new name to codepoint mapping with the name normalized to lowercase and space
    /// as a delimiter; returns the previous key that this name mapped to if there was one.
    /// # Example
    /// ```
    /// use emoji_builder::tables::emoji_tables::EmojiTable;
    ///
    /// let name = "thinking face";
    /// let codepoint = vec![0x1f914];
    /// let mut table = EmojiTable::new();
    /// // Even if this description string is the same as the name, it does not have to be.
    /// table.insert(codepoint.clone(), (vec![], Some(name.to_string()), vec![]));
    /// table.insert_lookup_name(name, codepoint.clone());
    ///
    /// // Assert that we can find an entry with the given name (and that it's the correct one)
    /// assert_eq!(*table.get_by_name(name).unwrap().0, codepoint);
    /// ```
    pub fn insert_lookup_name(&mut self, name: &str, key: EmojiTableKey) -> Option<EmojiTableKey> {
        let lookup_name = utils::normalize_lookup_name(name);
        self.names.insert(lookup_name, key)
    }

    pub fn insert_file_entry(&mut self, entry: EmojiFileEntry) {
        match entry {
            EmojiFileEntry::Data(data) => {
                let kind = EmojiKind::from_str(data.kind)
                    .unwrap_or_else(|err| err.get());

                // No, descriptions will not be used for now; these can be more easily obtained
                // from emoji-test.txt
                match data.codepoints_content {
                    EmojiDataCodepoints::Range(range) => {
                        self.update_range(range.range_start, range.range_end, Some(kind));
                    }
                    EmojiDataCodepoints::Sequence(sequence) => {
                        self.update_emoji(
                            utils::key_from_str(sequence.sequence),
                            Some(kind),
                            None,
                            None
                        )
                    }
                }
            }
            EmojiFileEntry::Test(test) => {
                let codepoints: Vec<_> = utils::key_from_str(test.sequence);
                let status = EmojiStatus::from_str(test.status);
                let name = test.description;

                self.update_emoji(codepoints.clone(), None, Some(name), status.clone().ok());

                // Don't insert unqualified codepoints unless we don't have a mapping for this name anyway
                if status.unwrap_or_default().is_emoji() || self.get_by_name(&name).is_none() {
                    self.insert_lookup_name(&name, codepoints.clone());
                }
            }
        }
    }

    // -- Update single items --

    /// Parses lines that specify a range of emoji codepoints,
    /// like `1F3F3..1F3F5 ; Emoji #  7.0  [3] (🏳️..🏵️)    white flag..rosette`
    /// **Note**: This will only parse single codepoint emojis (i.e. ranges for sequences are not allowed).
    /// However, at least the official Unicode® emoji data tables only include single codepoint ranges.
    /// Descriptions will _not_ be parsed as they would only be available for the start and end codepoint anyway.
    ///
    /// The table will be used to find existing kinds/descriptions
    fn update_range(&mut self, start: &str, end: &str, kind: Option<EmojiKind>) {
        // Start and end are already built from a regular expression that only matches hexadecimal strings
        let start = u32::from_str_radix(start, 16).unwrap();
        let end = u32::from_str_radix(end, 16).unwrap();
        for codepoint in start..=end {
            self.update_emoji(vec![codepoint], kind.clone(), None, None);
        }
    }

    /// Updates or adds an entry in the table
    /// # Arguments
    /// `emoji`: The codepoint sequence for the emoji
    /// `kind`: The emoji kind to assign for this step
    /// `status`: Possible EmojiStatuses to add
    /// `description`: The name of the emoji
    fn update_emoji(&mut self,
                    emoji: EmojiTableKey,
                    kind: Option<EmojiKind>,
                    description: Option<&str>,
                    status: Option<EmojiStatus>
    ) {
        let existing_entry = self.get_mut(&emoji);
        if let Some((kinds, existing_description, existing_status)) = existing_entry {
            utils::add_kind(kinds, kind);
            utils::update_description(existing_description, description);
            utils::insert_in_order(existing_status, status);
        } else {
            let entry = (
                // We expect that at some point the emoji will have up to like 4 EmojiKinds
                kind.map(|kind| vec![kind]).unwrap_or_else(|| Vec::with_capacity(4)),
                description.map(|descr| descr.to_owned()),
                // If an emoji can have more than one EmojiStatus, it'll be probably max 2
                status.map(|status| vec![status]).unwrap_or_else(|| Vec::with_capacity(2))
            );
            self.table.insert(emoji, entry);
        }
    }


    // -- Get information on items --

    pub fn get_description(&self, sequence: &[u32]) -> Option<String> {
        match self.get(sequence) {
            Some((_, description, _)) => description.clone(),
            None => None,
        }
    }

    /// Returns the table entry for a given key
    pub fn get<T: Hash + Eq + ?Sized>(&self, key: &T) -> Option<&EmojiTableEntry>
        where EmojiTableKey: Borrow<T> {
        self.table.get(key)
    }

    /// Returns the table entry for a given key
    pub fn get_with_without_fe0f<T: Hash + Eq + ?Sized>(&self, key: &T) -> Option<&EmojiTableEntry>
        where EmojiTableKey: Borrow<T> {
        let key = self.find_key_present(key);
        key.map(|key| self.table.get(key).unwrap())
    }

    pub fn get_mut<T: Hash + Eq + ?Sized>(&mut self, key: &T) -> Option<&mut EmojiTableEntry>
        where EmojiTableKey: Borrow<T> {
        self.table.get_mut(key)
    }

    pub fn get_mut_with_without_fe0f<'a, T: Hash + Eq + ?Sized>(&'a mut self, key: &'a T) -> Option<&'a mut EmojiTableEntry>
        where EmojiTableKey: Borrow<T> {
        let key = self.find_key_present(key).map(|key| key as *const T);
        if let Some(key) = key {
            // We use unsafe here in order to be able to reference the key (inside the table) while
            // referencing the whole table as mut.
            // This is okay, as we only access the _value_ inside the table as mut, which doesn't
            // affect its corresponding key (or anything else in the table).
            let key: &'a T = unsafe { key.as_ref() }.unwrap();
            self.table.get_mut(key)
        } else {
            None
        }
    }

    pub fn find_key_present<'a, T: Hash + Eq + ?Sized>(&'a self, key: &'a T) -> Option<&'a T>
        where EmojiTableKey: Borrow<T> {
        if self.table.contains_key(key) {
            Some(key)
        } else {
            let possible_keys = vec![self.fe0f_table.get_by_right(key), self.fe0f_table.get_by_left(key)];
            possible_keys.into_iter()
                .flatten()
                .find_map(|key| if self.table.contains_key::<EmojiTableKey>(key) {
                    Some(&*key.borrow())
                } else {
                    None
                })
        }
    }

    /// Finds an emoji by its name (this is case-insensitive and converts delimiters to the desired format)
    /// # Examples
    /// ```
    /// use emoji_builder::tables::emoji_tables::EmojiTable;
    ///
    /// let mut table = EmojiTable::new();
    /// let key = vec![0x1f914];
    /// let entry = (vec![], Some(String::from("Thinking")), vec![]);
    /// table.insert(key.clone(), entry.clone());
    /// table.insert_lookup_name("ThInKiNg_FaCe", key.clone());
    /// assert_eq!(Some((key.clone(), &entry)), table.get_by_name("tHiNkIng-fAcE"));
    ///
    /// // Emojis themselves are already valid lookup names
    /// assert_eq!(Some((key.clone(), &entry)), table.get_by_name("🤔"));
    /// table.insert_lookup_name("thinkin'", key.clone());
    /// // We don't overwrite the old mapping, so this still works
    /// assert_eq!(Some((key.clone(), &entry)), table.get_by_name("tHiNkIng-fAcE"));
    /// assert_eq!(Some((key.clone(), &entry)), table.get_by_name("thinkin"));
    /// ```
    pub fn get_by_name(&self, name: &str) -> Option<(EmojiTableKey, &EmojiTableEntry)> {
        // First we'll try to look up the string itself, because it might be an emoji
        let chars = name.chars()
            .map(|character| character as u32)
            .collect_vec();
        if let Some(entry) = self.table.get(&chars) {
            Some((chars, entry))
        } else {
            let lookup_name = utils::normalize_lookup_name(name);
            if let Some(codepoint) = self.names.get(&lookup_name) {
                self.table.get(codepoint).map(|entry| (codepoint.clone(), entry))
            } else {
                None
            }
        }
    }

    /// A helper function to get emojis by their name directly; might panic, but that should be okay
    /// in tests.
    #[cfg(test)]
    pub fn get_codepoint_by_name(&self, name: &str) -> Vec<u32> {
        self.get_by_name(name).unwrap().0.clone()
    }


    // -- Get general information --

    /// Returns the size of the table
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Checks whether the table is empty
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }


    // https://stackoverflow.com/a/34969944
    /// Validates whether all emojis from this table can be found in a collection of emojis and vice versa.
    /// As it is usually not a problem to have additional emojis in a font, these are not returned as an error.
    /// # Returns
    /// `(result, additional_emojis)` with `result` being either `Ok(())`, if all emojis con be found
    /// or `Err(missing_emojis)` with the emojis that are missing.
    /// `additional_emojis` are those emojis that are found in the font, but not in the table; might be empty.
    pub fn validate(&self, emojis: &HashSet<EmojiTableKey>, ignore_fe0f: bool) -> (Result<(), Vec<Emoji>>, Vec<Emoji>) {
        // TODO: Introduce the status to filter out unqualified emojis/non-RGI
        let table_emojis = self.table
            .iter()
            // Only validate emojis that we have names for (i.e. they're in emoji-test.txt. Otherwise they won't matter anyway)
            // And those with an EmojiKind, as otherwise it's likely not an emoji
            .filter_map(|(key, (kinds, name, status))| if
                status.iter().any(|status| status.is_emoji()) || (name.is_some() && !kinds.is_empty()) {
                Some(key)
            } else {
                None
            });
        // TODO: Maybe add an || self.ignore_fe0f here?
        let table_emojis: HashSet<EmojiTableKey> = if ignore_fe0f {
            table_emojis
                .map(|emoji| emoji.iter()
                    .filter_map(|codepoint| if *codepoint != 0xfe0f {
                        Some(*codepoint)
                    } else {
                        None
                    } )
                    .collect_vec()
                )
                .collect()
        } else {
            table_emojis.cloned().collect()
        };
        let missing = table_emojis
            .difference(emojis)
            .filter_map(|emoji| Emoji::from_u32_sequence(emoji.clone(), Some(&self)).ok()).collect_vec();
        let emojis = if ignore_fe0f {
            // FIXME: We don't actually want to clone here
            emojis.clone()
        } else {
            emojis.iter()
                .map(|emoji| emoji.iter()
                    .filter_map(|codepoint| if *codepoint != 0xfe0f {
                        Some(*codepoint)
                    } else {
                        None
                    } )
                    .collect_vec()
                )
                .collect()
        };
        let additional = emojis
            .difference(&table_emojis)
            // Note: it doesn't make sense here to provide this emoji table as we have just found out
            // that it doesn't contain this particular emoji!
            .filter_map(|emoji| Emoji::from_u32_sequence(emoji.clone(), None).ok()).collect_vec();
        (
            if missing.is_empty() {
                Ok(())
            } else {
                Err(missing)
            },
            additional
        )
    }

}


// -- Trait impls --

impl Default for EmojiTable {
    fn default() -> Self {
        EmojiTable::new()
    }
}

impl AsRef<HashMap<EmojiTableKey, EmojiTableEntry>> for EmojiTable {
    fn as_ref(&self) -> &HashMap<Vec<u32>, EmojiTableEntry, RandomState> {
        &self.table
    }
}

impl Add<EmojiTable> for EmojiTable {
    type Output = EmojiTable;

    fn add(self, other: EmojiTable) -> Self::Output {
        let table = self.table.into_iter()
            .chain(other.table.into_iter())
            .collect();
        let mut fe0f_table = BiHashMap::with_capacity(
            // We'll probably have many duplicates, so we won't make it larger than both of them
            max(self.fe0f_table.len(), other.fe0f_table.len())
        );
        self.fe0f_table.into_iter()
            .chain(other.fe0f_table.into_iter())
            .for_each(|(with_fe0f, without_fe0f)| {
                fe0f_table.insert(with_fe0f, without_fe0f);
            });
        let names = self.names.into_iter()
            .chain(other.names.into_iter())
            .collect();

        EmojiTable { table, names, fe0f_table }
    }
}

impl <'a, 'b> Add<&'b EmojiTable> for &'a EmojiTable {
    type Output = EmojiTable;

    fn add(self, other: &'b EmojiTable) -> Self::Output {
        let table = self.table.iter()
            .chain(other.table.iter())
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let mut fe0f_table = BiHashMap::with_capacity(
            // We'll probably have many duplicates, so we won't make it larger than both of them
            max(self.fe0f_table.len(), other.fe0f_table.len())
        );
        self.fe0f_table.iter()
            .chain(other.fe0f_table.iter())
            .for_each(|(with_fe0f, without_fe0f)| {
                fe0f_table.insert(with_fe0f.clone(), without_fe0f.clone());
            });
        let names = self.names.iter()
            .chain(other.names.iter())
            .map(|(name, sequence)| (name.clone(), sequence.clone()))
            .collect();

        EmojiTable { table, names, fe0f_table }
    }
}

impl AddAssign<EmojiTable> for EmojiTable {
    fn add_assign(&mut self, other: EmojiTable) {
        self.table.extend(other.table.into_iter());

        // Update the FE0F-table
        other.fe0f_table.into_iter()
            .for_each(|(with_fe0f, without_fe0f)| {
                self.fe0f_table.insert(with_fe0f, without_fe0f);
            }
            );
        self.names.extend(other.names.into_iter());
    }
}

impl<'a, S> Extend<S> for EmojiTable
    where S: Borrow<str> {
    fn extend<T: IntoIterator<Item=S>>(&mut self, iter: T) {
        iter.into_iter()
            .filter(|line: &S| !line.borrow().trim().starts_with('#') && !line.borrow().trim().is_empty())
            .map(|line: S| line.borrow().trim().to_lowercase())
            .for_each(|line| {
                let entry = match_line(&line);
                if let Some(entry) = entry {
                    self.insert_file_entry(entry);
                } else {
                    warn!("Malformed line: {}", line);
                }
            });
    }
}

impl<'a> TryFrom<EmojiTablePrototype<'a>> for EmojiTable {
    type Error = NoError;

    fn try_from(prototype: EmojiTablePrototype<'a>) -> Result<Self, Self::Error> {
       let mut table = Self::with_capacity(prototype.entries.len());
        prototype.entries.into_iter()
            .for_each(|(_, entry)| table.insert_file_entry(entry));
        Ok(table)
    }
}


// TODO: Implement iterators
pub enum Fe0fHandling {
    RemoveFe0f,
    WithWithoutFe0f,
    Default
}