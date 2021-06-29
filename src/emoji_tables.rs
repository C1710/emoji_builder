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

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::path::Path;
use std::str::FromStr;

use itertools::Itertools;
use regex::Regex;

use crate::emoji::{EmojiKind, Emoji};
#[cfg(feature = "online")]
use std::sync::RwLock;


/// A code sequence
type EmojiTableKey = Vec<u32>;
// The EmojiKinds and optionally a description/name
type EmojiTableEntry = (Vec<EmojiKind>, Option<String>, Option<EmojiStatus>);

const EMOJI_SEQUENCE_SPACE_REGEX: &str = r"(([A-F0-9a-f]{1,8})(\s+([A-F0-9a-f]{1,8}))*)";
const EMOJI_STATUS_REGEX: &str = r"(component|fully-qualified|minimally-qualified|unqualified)";
const EMOJI_NAME_REGEX: &str = r"(.*)?\s*E(\d+.\d+) (.+)";

/// An internal representation of one or more Unicode® emoji data tables
/// <https://unicode.org/Public/emoji/12.0/>
/// It maps emoji code sequences to their kind and (if given) a description/name.
#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Eq)]
#[derive(Clone)]
pub struct EmojiTable(HashMap<EmojiTableKey, EmojiTableEntry>, HashMap<String, EmojiTableKey>);

impl EmojiTable {
    /// Creates a new, empty emoji table
    pub fn new() -> Self {
        Self(HashMap::new(), HashMap::new())
    }

    /// Reads multiple files which are formatted in the same way as the Unicode® emoji data tables
    /// (See <https://unicode.org/Public/emoji/12.0/>) and builds a lookup table
    /// to gather additional metadata for emojis.
    ///
    /// If an emoji sequence (in this case an entry with more than one codepoints) contains the VS-16
    /// (Variant Selector-16 - Emoji Representation, U+FE0F), the sequence will also be included without the VS-16.
    ///
    /// **Important** Currently, names are only extracted from emoji-test.txt-like files
    /// # Examples:
    /// ```
    /// use std::path::PathBuf;
    /// use emoji_builder::emoji::EmojiKind::EmojiZwjSequence;
    /// use emoji_builder::emoji_tables::EmojiTable;
    /// use std::collections::HashMap;
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
    /// let rainbow_entry = (vec![EmojiZwjSequence], None);
    ///
    /// assert!(table.as_ref().contains_key(&rainbow));
    /// assert!(table.as_ref().contains_key(&rainbow_no_fe0f));
    ///
    /// assert_eq!(*table.get(&rainbow).unwrap(), rainbow_entry);
    /// ```
    pub fn from_files<P: AsRef<Path>>(paths: &[P]) -> Result<EmojiTable, Error> {
        let mut table = EmojiTable::new();

        for path in paths {
            EmojiTable::expand_from_file(&mut table, path)?;
        }
        Ok(table)
    }

    /// Expands the table with the contents of an emoji table-file with  the syntax of e.g.
    /// `emoji-data.txt`.
    /// Only the emoji itself and its kind(s) is/are extended.
    /// Names are extended from `emoji-test.txt`-like files, using [EmojiTable::expand_descriptions_from_test_data]
    pub fn expand<I: BufRead>(&mut self, reader: I) -> Result<(), Error> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]{1,8}").unwrap();
            static ref RANGE: Regex = Regex::new(&format!(r"(?P<range>(?P<range_start>{hex})\.\.(?P<range_end>{hex}))", hex = &*HEX_SEQUENCE)).unwrap();
            static ref SEQUENCE: Regex = Regex::new(&format!(r"(?P<sequence>({hex})(\s+({hex}))*)", hex = &*HEX_SEQUENCE)).unwrap();
            static ref EMOJI_REGEX: Regex = Regex::new(&format!(r"(?P<codepoints>{}|{})", &*RANGE, &*SEQUENCE)).unwrap();
            // TODO: Maybe make this more specific
            static ref EMOJI_KIND_REGEX: Regex = Regex::new(r"(?P<kind>[A-Za-z_\-]+)").unwrap();
            static ref DATA_REGEX: Regex = Regex::new(&format!(r"^{}\s*;\s*{}\s*(;(?P<name>.*)\s*)?(#.*)?$", &*EMOJI_REGEX, &*EMOJI_KIND_REGEX)).unwrap();
        }

        for line in reader.lines()
            .filter_map(|line| line.ok()) {
            let line = line.trim();
            if !line.starts_with('#') && !line.is_empty() {
                let captures = (&*DATA_REGEX as &Regex).captures(line);
                if let Some(captures) = captures {
                    let kind = EmojiKind::from_str(captures.name("kind").unwrap().as_str())
                        .unwrap_or_else(|err| err.get());

                    // No, descriptions will not be used for now; these can be more easily obtained
                    // from emoji-test.txt

                    if captures.name("range").is_some() {
                        let start = captures.name("range_start").unwrap().as_str();
                        let end = captures.name("range_end").unwrap().as_str();
                        self.update_range(start, end, Some(kind));
                    } else if let Some(sequence) = captures.name("sequence") {
                        self.update_emoji(Self::get_codepoint_sequence(sequence.as_str()),
                                          Some(kind),
                                          None,
                                          None);
                    } else {
                        unreachable!("Either a range or a sequence has to be captured");
                    }
                } else {
                    error!("Malformed line in emoji-table: {}", line);
                }
            }
        }
        Ok(())
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
    /// use emoji_builder::emoji_tables::EmojiTable;
    /// use emoji_builder::emoji::EmojiKind;
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
    /// let rainbow_entry = (vec![EmojiKind::EmojiZwjSequence], None);
    ///
    /// assert!(table.as_ref().contains_key(&rainbow));
    /// assert!(table.as_ref().contains_key(&rainbow_no_fe0f));
    ///
    /// assert_eq!(*table.get(&rainbow).unwrap(), rainbow_entry);
    /// ```
    pub fn expand_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        self.expand(reader)
    }

    fn _get_description(&self, sequence: &[u32]) -> Option<String> {
        match self.0.get(sequence) {
            Some((_, description, _)) => description.clone(),
            None => None,
        }
    }

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
    /// `description`: The name of the emoji
    fn update_emoji(&mut self,
                    emoji: EmojiTableKey,
                    kind: Option<EmojiKind>,
                    description: Option<&str>,
                    status: Option<EmojiStatus>
    ) {
        let existing_entry = self.0.get_mut(&emoji);
        if let Some((kinds, existing_description, existing_status)) = existing_entry {
            Self::add_kind(kinds, kind);
            Self::update_description(existing_description, description);
            *existing_status = status.or(*existing_status)
        } else {
            let entry = (
                // We expect that at some point the emoji will have at least one kind
                kind.map(|kind| vec![kind]).unwrap_or_else(|| Vec::with_capacity(1)),
                description.map(|descr| descr.to_owned()),
                status
            );
            self.0.insert(emoji, entry);
        }
    }

    fn update_description(old_description: &mut Option<String>, new_description: Option<&str>) {
        if let Some(old_description) = old_description {
            if let Some(new_description) = new_description {
                if !new_description.trim().is_empty() {
                    *old_description = new_description.to_owned();
                }
            }
        } else {
            *old_description = new_description.map(|description| description.to_owned());
        }
    }

    fn add_kind(existing_kinds: &mut Vec<EmojiKind>, kind: Option<EmojiKind>) {
        if let Some(kind) = kind {
            if !existing_kinds.contains(&kind) {
                existing_kinds.insert(existing_kinds.binary_search(&kind).unwrap_err(), kind);
            }
        }
    }

    fn get_codepoint_sequence(raw_codepoints: &str) -> EmojiTableKey {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]+").unwrap();
        }

        let matches = HEX_SEQUENCE.find_iter(raw_codepoints);
        matches
            .map(|sequence| sequence.as_str())
            .map(|sequence| u32::from_str_radix(sequence, 16).unwrap_or_default())
            .filter(|codepoint| *codepoint > 0)
            .collect()
    }

    /// Inserts a new key-entry pair into the table and returns the last entry if there was one.
    /// This is simply passed on to the internal `HashMap`.
    /// Please be aware that no name-key-mapping is inserted.
    /// That means:
    /// ```
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let name = "thinking face";
    /// let codepoint = vec![0x1f914];
    /// let mut table = EmojiTable::new();
    /// table.insert(codepoint.clone(), (vec![], Some(name.to_string())));
    ///
    /// // We can't find the emoji by its name!
    /// assert_eq!(table.get_by_name(name), None);
    /// ```
    pub fn insert(&mut self, key: EmojiTableKey, entry: EmojiTableEntry) -> Option<EmojiTableEntry> {
        self.0.insert(key, entry)
    }

    /// Inserts a new name to codepoint mapping with the name normalized to lowercase and space
    /// as a delimiter; returns the previous key that this name mapped to if there was one.
    /// # Example
    /// ```
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let name = "thinking face";
    /// let codepoint = vec![0x1f914];
    /// let mut table = EmojiTable::new();
    /// // Even if this description string is the same as the name, it does not have to be.
    /// table.insert(codepoint.clone(), (vec![], Some(name.to_string())));
    /// table.insert_lookup_name(name, codepoint.clone());
    ///
    /// // Assert that we can find an entry with the given name (and that it's the correct one)
    /// assert_eq!(*table.get_by_name(name).unwrap().0, codepoint);
    /// ```
    pub fn insert_lookup_name(&mut self, name: &str, key: EmojiTableKey) -> Option<EmojiTableKey> {
        let lookup_name = Self::normalize_lookup_name(name);
        self.1.insert(lookup_name, key)
    }

    /// Returns the table entry for a given key
    pub fn get<T: AsRef<EmojiTableKey>>(&self, index: &T) -> Option<&EmojiTableEntry> {
        let index: &EmojiTableKey = index.as_ref();
        self.0.get(index)
    }

    /// Finds an emoji by its name (this is case-insensitive and converts delimiters to the desired format)
    /// # Examples
    /// ```
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let mut table = EmojiTable::new();
    /// let key = vec![0x1f914];
    /// let entry = (vec![], Some(String::from("Thinking")));
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
        if let Some(entry) = self.0.get(&chars) {
            Some((chars, entry))
        } else {
            let lookup_name = Self::normalize_lookup_name(name);
            if let Some(codepoint) = self.1.get(&lookup_name) {
                self.0.get(codepoint).map(|entry| (codepoint.clone(), entry))
            } else {
                None
            }
        }
    }

    /// Converts names to the format used in the lookup table for names.
    ///
    /// This method here might cause some issues when dealing with names with hyphens:
    /// For example emoji U+1F60D has the name "smiling face with heart-eyes" which is converted
    /// to "smiling face with heart eyes" here. Therefore these lookup names should not be used as
    /// display names/descriptions.
    ///
    /// Also some special characters like `:` or `,` will be removed in order to allow simpler file
    /// names.
    pub fn normalize_lookup_name(name: &str) -> String {
        lazy_static! {
            static ref DELIMITERS: Regex = Regex::new(r"[-_. ]").unwrap();
            static ref REMOVED: Regex = Regex::new(r#"[,*\\/:'"()]"#).unwrap();
        }
        (&*DELIMITERS as &Regex).split(&REMOVED.replace_all(name, "")).join(" ").to_lowercase()
    }

    /// Returns the size of the table
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Checks whether the table is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
    pub fn expand_descriptions_from_test_data<I: BufRead>(&mut self, reader: I) -> Result<(), Error> {
        lazy_static! {
            static ref EMOJI_TEST_REGEX: Regex = Regex::new(&format!(r"^{}\s*;\s*{}\s*#\s*{}$",
                                               EMOJI_SEQUENCE_SPACE_REGEX,
                                               EMOJI_STATUS_REGEX,
                                               EMOJI_NAME_REGEX)
            ).unwrap();
        };
        for line in reader.lines().flatten() {
            let line = line.trim();
            // Only check if it's not a comment/empty line
            if !line.starts_with('#') & !line.is_empty() {
                // Try to match the line
                if let Some(captures) = (&*EMOJI_TEST_REGEX as &Regex).captures(line) {
                    // Extract information
                    let codepoints: Vec<_> = Self::get_codepoint_sequence(captures.get(1).unwrap().as_str());
                    let status = captures.get(5).unwrap().as_str();
                    let status = EmojiStatus::from_str(status);
                    let _emoji = captures.get(6);
                    let _version = captures.get(7).unwrap();
                    let name = captures.get(8).unwrap().as_str();

                    self.update_emoji(codepoints.clone(), None, Some(name), status.clone().ok());

                    // Don't insert unqualified codepoints unless we don't have a mapping for this name anyway
                    if status.unwrap_or_default().is_emoji() || self.get_by_name(&name).is_none() {
                        self.insert_lookup_name(&name, codepoints.clone());
                    }
                } else {
                    warn!("Malformed line in emoji-test.txt: {}", line);
                }
            }
        };
        Ok(())
    }

    #[cfg(feature = "online")]
    const EMOJI_DATA: &'static str = "emoji-data.txt";
    #[cfg(feature = "online")]
    const EMOJI_SEQUENCES: &'static str = "emoji-sequences.txt";
    #[cfg(feature = "online")]
    const EMOJI_ZWJ_SEQUENCES: &'static str = "emoji-zwj-sequences.txt";
    #[cfg(feature = "online")]
    const EMOJI_VARIATION_SEQUENCES: &'static str = "emoji-variation-sequences.txt";
    #[cfg(feature = "online")]
    const EMOJI_TEST: &'static str = "emoji-test.txt";
    #[cfg(feature = "online")]
    const DATA_FILES: [&'static str; 3] = [
        Self::EMOJI_DATA,
        Self::EMOJI_SEQUENCES,
        Self::EMOJI_ZWJ_SEQUENCES
    ];



    /// This function is <del>equivalent to</del> creating an `EmojiTable` and directly calling `expand_all_online` on it.`
    #[cfg(feature = "online")]
    pub fn load_online(version: (u32, u32)) -> Result<EmojiTable, ExpansionError> {
        let mut table = EmojiTable::new();
        match table.expand_all_online(version) {
            Ok(_) => Ok(table),
            Err(error) => Err(error)
        }
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

        let errors: Vec<_> = Self::DATA_FILES.iter()
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
        let reader = Self::get_data_file_online(client, version, file)?;
        self.expand(reader)?;
        Ok(())
    }

    #[cfg(feature = "online")]
    #[inline]
    fn get_data_file_online(client: &reqwest::blocking::Client, version: (u32, u32), file: &'static str) -> Result<std::io::Cursor<bytes::Bytes>, reqwest::Error> {
        // Check if we can return the file from the cache already
        let cache = TABLE_CACHE.read();
        if let Ok(cache) = cache {
            if let Some(cached_files) = cache.get(&version) {
                if let Some(cached) = cached_files.get(file) {
                    return Ok(std::io::Cursor::new(cached.clone()));
                }
            }
        }
        let request = client.get(&Self::build_url(version, file)).send();
        let bytes = request?.bytes()?;

        // Insert data into the cache
        let cache = TABLE_CACHE.write();
        if let Ok(mut cache) = cache {
            if let Some(cached_files) = cache.get_mut(&version) {
                // We need to check again here, since we didn't hold the Lock for some time
                if !cached_files.contains_key(file) {
                    cached_files.insert(String::from(file), bytes.clone());
                }
            } else {
                // There are about 4 files for each version, so having 8 should be sufficient
                let mut cached_files = HashMap::with_capacity(8);
                cached_files.insert(String::from(file), bytes.clone());
                cache.insert(version, cached_files);
            }
        }

        Ok(std::io::Cursor::new(bytes))
    }

    #[cfg(feature = "online")]
    fn expand_descriptions_from_test_online(&mut self, client: &reqwest::blocking::Client, version: (u32, u32)) -> Result<(), ExpansionError> {
        let reader = Self::get_data_file_online(client, version, Self::EMOJI_TEST)?;
        self.expand_descriptions_from_test_data(reader).map_err(|err| err.into())
    }

    /// A simple helper function to build the URLs for the different files.
    #[cfg(feature = "online")]
    #[inline]
    fn build_url(version: (u32, u32), file: &'static str) -> String {
        if version.0 >= 13 && [Self::EMOJI_DATA, Self::EMOJI_VARIATION_SEQUENCES].contains(&file) {
            format!("https://unicode.org/Public/{}.0.0/ucd/emoji/{}", version.0, file)
        } else {
            format!("https://unicode.org/Public/emoji/{}.{}/{}", version.0, version.1, file)
        }
    }

    /// A helper function to get emojis by their name directly
    #[cfg(test)]
    fn get_codepoint_by_name(&self, name: &str) -> Vec<u32> {
        self.get_by_name(name).unwrap().0.clone()
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
        let table_emojis = self.0
            .iter()
            // Only validate emojis that we have names for (i.e. they're in emoji-test.txt. Otherwise they won't matter anyway)
            // And those with an EmojiKind, as otherwise it's likely not an emoji
            .filter_map(|(key, (kinds, name, status))| if
                status.unwrap_or_default().is_emoji() || (name.is_some() && !kinds.is_empty()) {
                Some(key)
            } else {
                None
            });
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

    pub fn extend(&mut self, other: EmojiTable) {
        self.0.extend(other.0.into_iter());
        self.1.extend(other.1.into_iter());
    }

    pub fn extend_preserve_own(&mut self, other: EmojiTable) {
        self.0 = other.0.into_iter().chain(self.0.drain().into_iter()).collect();
        self.1 = other.1.into_iter().chain(self.1.drain().into_iter()).collect();
    }
}

impl Default for EmojiTable {
    fn default() -> Self {
        EmojiTable::new()
    }
}

impl From<HashMap<EmojiTableKey, EmojiTableEntry>> for EmojiTable {
    fn from(table: HashMap<Vec<u32>, EmojiTableEntry, RandomState>) -> Self {
        let names_map: HashMap<String, EmojiTableKey> = table
            .iter()
            .filter_map(|(codepoint, (_, name, _))| name.as_ref().map(|name| (name.clone(), codepoint.clone())))
            .collect();
        EmojiTable(table, names_map)
    }
}

impl From<EmojiTable> for HashMap<EmojiTableKey, EmojiTableEntry> {
    fn from(table: EmojiTable) -> Self {
        table.0
    }
}

impl AsRef<HashMap<EmojiTableKey, EmojiTableEntry>> for EmojiTable {
    fn as_ref(&self) -> &HashMap<Vec<u32>, EmojiTableEntry, RandomState> {
        &self.0
    }
}

// 14 Unicode/emoji main versions * 2 minor versions ~= 32 versions
#[cfg(feature = "online")]
lazy_static! {
    static ref TABLE_CACHE: RwLock<HashMap<(u32, u32), HashMap<String, bytes::Bytes>>> =
        RwLock::new(HashMap::with_capacity(32));
}


/// A representation of errors encountered while parsing or using emoji tables.
#[derive(Debug)]
pub enum EmojiTableError {
    /// Indicates that an emoji with the given sequence is not in the table
    KeyNotFound(EmojiTableKey),
}

/// The status of an emoji according to `emoji-test.txt`
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EmojiStatus {
    /// ? TODO: Find out, what this is
    Component,
    /// It is a regular, RGI emoji
    FullyQualified,
    /// ? TODO: Find out, what this is
    MinimallyQualified,
    /// Not actually displayed as an emoji/not RGI
    Unqualified
}

impl EmojiStatus {
    pub fn is_emoji(&self) -> bool {
        matches!(self, Self::Component | Self::FullyQualified | Self::MinimallyQualified)
    }
}

impl Default for EmojiStatus {
    fn default() -> Self {
        Self::Unqualified
    }
}

impl ToString for EmojiStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Component => "component".to_string(),
            Self::Unqualified => "unqualified".to_string(),
            Self::FullyQualified => "fully-qualified".to_string(),
            Self::MinimallyQualified => "minimally-qualified".to_string()
        }
    }
}

impl FromStr for EmojiStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "component" => Ok(Self::Component),
            "unqualified" => Ok(Self::Unqualified),
            "fully-qualified" => Ok(Self::FullyQualified),
            "minimally-qualified" => Ok(Self::MinimallyQualified),
            other => Err(other.to_string())
        }
    }
}

#[derive(Debug)]
/// An error that occurs while expanding an [EmojiTable]
pub enum ExpansionError {
    /// Wrapper for [std::io::Error]
    Io(std::io::Error),
    /// Wrapper for multiple errors
    Multiple(Vec<ExpansionError>),
    #[cfg(feature = "online")]
    /// Wrappter for [reqwest::Error]
    Reqwest(reqwest::Error),
}

impl From<std::io::Error> for ExpansionError {
    fn from(err: std::io::Error) -> Self {
        ExpansionError::Io(err)
    }
}

impl From<Vec<ExpansionError>> for ExpansionError {
    fn from(errors: Vec<ExpansionError>) -> Self {
        Self::Multiple(errors)
    }
}

#[cfg(feature = "online")]
impl From<reqwest::Error> for ExpansionError {
    fn from(err: reqwest::Error) -> Self {
        ExpansionError::Reqwest(err)
    }
}

#[cfg(feature = "online")]
#[test]
fn test_online() {
    let table = EmojiTable::load_online((13, 0)).unwrap();

    let kissing_face = vec![0x1f617];
    let smiling_face = vec![0x263a, 0xfe0f];
    let woman_medium_skin_tone_white_hair = vec![0x1f469, 0x1f3fd, 0x200d, 0x1f9b3];

    assert_eq!(table.get_codepoint_by_name("kissing face"), kissing_face);
    assert_eq!(table.get_codepoint_by_name("Smiling Face"), smiling_face);
    assert_eq!(table.get_codepoint_by_name("woman: medium skin tone, white hair"), woman_medium_skin_tone_white_hair);
    assert_eq!(table.get_codepoint_by_name("woman medium SkiN ToNe WhITe hair"), woman_medium_skin_tone_white_hair);

    assert_eq!(
        table.get_by_name("woman: medium skin tone, white hair").unwrap().1.0,
        vec![EmojiKind::EmojiZwjSequence]
    );

    assert!(table.get_by_name("woman").is_some());

    assert_eq!(
        table.get_by_name("woman").unwrap().1.0,
        vec![EmojiKind::Emoji, EmojiKind::ModifierBase, EmojiKind::EmojiPresentation, EmojiKind::Other(String::from("extended pictographic"))]
    );
}
