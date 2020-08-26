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
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Error};
use std::path::Path;
use std::str::FromStr;

use itertools::Itertools;
use regex::Regex;

use crate::emoji::EmojiKind;

type EmojiTableKey = Vec<u32>;
type EmojiTableEntry = (Vec<EmojiKind>, Option<String>);

/// An internal representation of one or more Unicode® emoji data tables
/// https://unicode.org/Public/emoji/12.0/
/// It maps emoji code sequences to their kind and (if given) a description/name.
#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Eq)]
pub struct EmojiTable(HashMap<EmojiTableKey, EmojiTableEntry>, HashMap<String, EmojiTableKey>);

impl EmojiTable {
    /// Creates a new, empty emoji table
    pub fn new() -> Self {
        Self(HashMap::new(), HashMap::new())
    }

    /// Reads multiple files which are formatted in the same way as the Unicode® emoji data tables
    /// (See https://unicode.org/Public/emoji/12.0/) and builds a lookup table
    /// to gather additional metadata for emojis.
    ///
    /// If an emoji sequence (in this case an entry with more than one codepoints) contains the VS-16
    /// (Variant Selector-16 - Emoji Representation, U+FE0F), the sequence will also be included without the VS-16.
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
    /// let rainbow_entry = (vec![EmojiZwjSequence], Some(String::from("rainbow flag")));
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

    pub fn expand<I: BufRead>(&mut self, reader: I) -> Result<(), Error> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]+").unwrap();
            static ref RANGE: Regex = Regex::new(r"([a-fA-F0-9]+)\.\.([a-fA-F0-9]+)").unwrap();
        }

        for line in reader.lines() {
            if let Ok(line) = line {
                // Ignore comments
                if !line.trim().starts_with('#') {
                    let mut cols: Vec<&str> = line.split(';').collect();

                    let (kind, description) = match cols.len() {
                        2 => {
                            // codepoint; kind # description
                            // Length is already checked
                            let kind_descr = cols.pop().unwrap();
                            let mut kind_descr = kind_descr.split('#');
                            let kind = Some(kind_descr.next().unwrap().trim());
                            let description = match kind_descr.next() {
                                Some(description) => Some(description.trim()),
                                None => None,
                            };
                            (kind, description)
                        }
                        1 => (None, None),
                        0 => (None, None),
                        _ => (Some(cols[1].trim()), Some(cols[2].trim())),
                    };

                    if let Some(kind) = kind {
                        let emoji = cols[0].trim();
                        let kind = match EmojiKind::from_str(kind) {
                            Ok(kind) => kind,
                            Err(unknown_kind) => unknown_kind.into(),
                        };

                        if let Some(range) = RANGE.captures(emoji) {
                            // from..to
                            let (start, end) = (&range[1], &range[2]);
                            let extension = self.parse_range(start, end, kind).0;
                            self.0.extend(extension);
                        } else {
                            // A single sequence (or a single codepoint)
                            let ((seq, entry), name_mapping) = self.parse_sequence(emoji, kind.clone(), description);
                            self.insert(seq, entry);
                            // If we have a name, we can insert it as well
                            if let Some((name, seq)) = name_mapping {
                                self.insert_name(&name, seq);
                            }
                            let emoji = emoji.to_lowercase();
                            if emoji.contains("fe0f") {
                                // Names will be mapped to the original code sequences including the
                                // FE0F-character.
                                // TODO: Check this behaviour
                                let ((seq, entry), _) = self.parse_sequence(
                                    &emoji.to_lowercase().replace("fe0f", ""),
                                    kind,
                                    description,
                                );
                                self.insert(seq, entry);
                            }
                        }
                    }
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
    /// If the new entry _has_ a description, the old description will be updated.
    /// Otherwise it will stay the same as before (which might also be `None`).
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
    /// let rainbow_entry = (vec![EmojiKind::EmojiZwjSequence], Some(String::from("rainbow flag")));
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

    fn get_description(&self, sequence: &[u32]) -> Option<String> {
        match self.0.get(sequence) {
            Some((_, description)) => description.clone(),
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
    fn parse_range(&self, start: &str, end: &str, kind: EmojiKind) -> EmojiTable {
        // Start and end are already built from a regular expression that only matches hexadecimal strings
        let start = u32::from_str_radix(start, 16).unwrap();
        let end = u32::from_str_radix(end, 16).unwrap();
        let mut out_table = HashMap::new();
        let mut names_map = HashMap::new();
        for codepoint in start..=end {
            let codepoint = vec![codepoint];
            // Reallocation would be necessary anyway (because of the extension of the vector).
            let existing_kinds = self.get(&codepoint);
            let mut kinds = match existing_kinds {
                Some((kinds, _)) => {
                    let mut new_kinds = Vec::with_capacity(existing_kinds.unwrap().0.len() + 1);
                    new_kinds.extend_from_slice(&kinds);
                    new_kinds
                }
                None => Vec::with_capacity(1),
            };
            kinds.push(kind.clone());

            let existing_description = self.get_description(&codepoint);

            if let Some(description) = &existing_description {
                names_map.insert(description.to_lowercase(), codepoint.clone());
            }

            out_table.insert(codepoint, (kinds, existing_description));
        }
        EmojiTable(out_table, names_map)
    }

    /// Parses a regular emoji codepoint sequence and adds it including a description
    fn parse_sequence(&self,
                      emoji: &str,
                      kind: EmojiKind,
                      description: Option<&str>,
    ) -> ((EmojiTableKey, (Vec<EmojiKind>, Option<String>)), Option<(String, EmojiTableKey)>) {
        let code_sequences = Self::get_codepoint_sequence(emoji);

        // Reallocation would be necessary anyway (because of the extension of the vector)
        let existing_kinds = self.get(&code_sequences);
        let mut kinds = match existing_kinds {
            Some((kinds, _)) => {
                let mut new_kinds = Vec::with_capacity(existing_kinds.unwrap().0.len() + 1);
                new_kinds.extend_from_slice(&kinds);
                new_kinds
            }
            None => Vec::with_capacity(1),
        };

        kinds.push(kind);

        let existing_description = self.get_description(&code_sequences);

        // FIXME: There seem to be various styles for comments.
        //        For example, the official tables have descriptions come last.
        if let Some(description) = description {
            let description = description.split('#').next().unwrap_or_default().trim();
            let description = String::from(description);
            let lower_description = description.to_lowercase();
            ((code_sequences.clone(), (kinds, Some(description))),
             Some((lower_description, code_sequences)))
        } else {
            ((code_sequences, (kinds, existing_description)), None)
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
    pub fn insert(&mut self, key: EmojiTableKey, entry: EmojiTableEntry) -> Option<EmojiTableEntry> {
        self.0.insert(key, entry)
    }

    /// Inserts a new name to codepoint mapping with the name normalized to lowercase and space
    /// as a delimiter.
    pub fn insert_name(&mut self, name: &str, key: EmojiTableKey) -> Option<EmojiTableKey> {
        let lookup_name = Self::normalize_lookup_name(name);
        self.1.insert(lookup_name, key)
    }

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
    /// table.insert_name("ThInKiNg_FaCe", key.clone());
    /// assert_eq!(Some((&key, &entry)), table.get_by_name("tHiNkIng-fAcE"));
    ///
    /// // If you have trouble seeing this: We're adding the emoji itself as a name
    /// table.insert_name("🤔", key.clone());
    /// assert_eq!(Some((&key, &entry)), table.get_by_name("🤔"));
    /// // We don't overwrite the old mapping, so this still works
    /// assert_eq!(Some((&key, &entry)), table.get_by_name("tHiNkIng-fAcE"));
    /// ```
    pub fn get_by_name(&self, name: &str) -> Option<(&EmojiTableKey, &EmojiTableEntry)> {
        let lookup_name = Self::normalize_lookup_name(name);
        if let Some(codepoint) = self.1.get(&lookup_name) {
            if let Some(entry) = self.0.get(codepoint) {
                Some((codepoint, entry))
            } else {
                None
            }
        } else {
            None
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
        DELIMITERS.split(&REMOVED.replace_all(name, "")).join(" ").to_lowercase()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Uses the names of the emoji-test.txt files.
    /// These seem to be more suitable than emoji-data.txt as they don't include any emoji character
    /// ranges.
    /// An example would be https://unicode.org/Public/emoji/13.0/emoji-test.txt.
    ///
    /// _Please note that this parser is extremely **strict** and will crash if somethind is wrong__
    ///
    /// The syntax of these files is:
    /// `Codepoint ; ("component"|"fully-qualified"|"minimally-qualified"|"unqualified") # Emoji "E"Version Emoji name`
    pub fn expand_descriptions_from_test_data<I: BufRead>(&mut self, reader: I) -> Result<(), Error> {
        for line in reader.lines() {
            if let Ok(line) = line {
                if !line.trim().starts_with('#') {
                    let mut cols: Vec<&str> = line.split(';').collect();

                    //We can make some assumptions here which we cannot make in the normal expand-function
                    let mut status_description = cols.pop().unwrap().split('#');
                    let codepoints = cols.pop().unwrap().trim();
                    // We know that emoji-test.txt looks like this
                    let _status = status_description.next().unwrap().trim();
                    let description = status_description.next().unwrap().trim();

                    let mut codepoint_sequence = Self::get_codepoint_sequence(codepoints);
                    if codepoint_sequence.ends_with(&[0xfe0f]) {
                        codepoint_sequence.pop();
                    }

                    if let Some((kind, _)) = self.0.remove(&codepoint_sequence) {
                        let mut description = description.split(' ');
                        let emoji = description.next().unwrap();
                        let _version = description.next().unwrap();
                        let description: String = description.collect_vec().join(" ");
                        self.insert_name(&description, codepoint_sequence.clone());
                        self.insert(codepoint_sequence.clone(), (kind, Some(description)));
                        // Yes, you will be able to use the emojis as file names.
                        // Unless your OS prevents you from doing such cursed stuff.
                        self.insert_name(emoji, codepoint_sequence);
                    }
                }
            }
        };
        Ok(())
    }

    const EMOJI_DATA: &'static str = "emoji-data.txt";
    const EMOJI_SEQUENCES: &'static str = "emoji-sequences.txt";
    const EMOJI_ZWJ_SEQUENCES: &'static str = "emoji-zwj-sequences.txt";
    const EMOJI_TEST: &'static str = "emoji-test.txt";
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
        let client_builder = reqwest::ClientBuilder::new();
        let client = client_builder.build()?;

        Self::DATA_FILES.iter()
            .map(|file| self.expand_data_online(&client, version, file))
            // TODO: Propagate errors instead of unwrapping
            .for_each(Result::unwrap);

        self.expand_descriptions_from_test_online(&client, version)
    }

    #[cfg(feature = "online")]
    fn expand_data_online(&mut self, client: &reqwest::Client, version: (u32, u32), file: &'static str) -> Result<(), reqwest::Error> {
        let reader = Self::get_data_file_online(client, version, file)?;
        self.expand(reader).unwrap();
        Ok(())
    }

    #[cfg(feature = "online")]
    #[inline]
    fn get_data_file_online(client: &reqwest::Client, version: (u32, u32), file: &'static str) -> Result<Cursor<bytes::Bytes>, reqwest::Error> {
        let request = client.get(&Self::build_url(version, file)).send();
        let bytes = futures::executor::block_on(async {
            request.await?.bytes().await
        })?;
        Ok(Cursor::new(bytes))
    }

    #[cfg(feature = "online")]
    fn expand_descriptions_from_test_online(&mut self, client: &reqwest::Client, version: (u32, u32)) -> Result<(), ExpansionError> {
        let reader = Self::get_data_file_online(client, version, Self::EMOJI_TEST)?;
        match self.expand_descriptions_from_test_data(reader) {
            Ok(()) => Ok(()),
            Err(error) => Err(error.into())
        }
    }

    // A simple helper function to build the URLs for the different files.
    #[inline]
    fn build_url(version: (u32, u32), file: &'static str) -> String {
        format!("https://unicode.org/Public/emoji/{}.{}/{}", version.0, version.1, file)
    }
}

impl Default for EmojiTable {
    fn default() -> Self {
        EmojiTable::new()
    }
}

impl From<HashMap<EmojiTableKey, EmojiTableEntry>> for EmojiTable {
    fn from(table: HashMap<Vec<u32>, (Vec<EmojiKind>, Option<String>), RandomState>) -> Self {
        let names_map: HashMap<String, EmojiTableKey> = table
            .iter()
            .filter_map(|(codepoint, (_, name))| match name {
                Some(name) => Some((name.clone(), codepoint.clone())),
                None => None
            })
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
    fn as_ref(&self) -> &HashMap<Vec<u32>, (Vec<EmojiKind>, Option<String>), RandomState> {
        &self.0
    }
}

/// A representation of errors encountered while parsing or using emoji tables.
#[derive(Debug)]
pub enum EmojiTableError {
    KeyNotFound(EmojiTableKey),
}

pub enum _EmojiTestStatus {
    Component,
    FullyQualified,
    MinimallyQualified,
    Unqualified,
}

#[derive(Debug)]
pub enum ExpansionError {
    Io(std::io::Error),
    #[cfg(feature = "online")]
    Reqwest(reqwest::Error),
}

impl From<std::io::Error> for ExpansionError {
    fn from(err: std::io::Error) -> Self {
        ExpansionError::Io(err)
    }
}

#[cfg(feature = "online")]
impl From<reqwest::Error> for ExpansionError {
    fn from(err: reqwest::Error) -> Self {
        ExpansionError::Reqwest(err)
    }
}
