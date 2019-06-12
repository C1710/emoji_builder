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
//! [unicode]: https://unicode.org/Public/emoji/12.0/

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::ops::Index;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use regex::Regex;

use crate::emoji::{Emoji, EmojiKind};

type EmojiTableKey = Vec<u32>;
type EmojiTableEntry = (Vec<EmojiKind>, Option<String>);

/// An internal representation of one or more Unicode® emoji data tables
/// https://unicode.org/Public/emoji/12.0/
/// It maps emoji code sequences to their kind and (if given) a description/name
#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Eq)]
pub struct EmojiTable(HashMap<EmojiTableKey, EmojiTableEntry>);

impl EmojiTable {
    /// Creates a new, empty emoji table
    pub fn new() -> Self {
        Self(HashMap::new())
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
    ///
    /// // Contains the entry
    /// // 1F3F3 FE0F 200D 1F308 ; Emoji_ZWJ_Sequence  ; rainbow flag #  7.0  [1] (🏳️‍🌈)
    /// let path = PathBuf::from("test_files/unicode/emoji-zwj-sequences.txt");
    /// let paths = vec![path];
    ///
    /// let table = EmojiTable::from_files(&paths).unwrap();
    ///
    /// let rainbow = vec![0x1f3f3, 0xfe0f, 0x200d, 0x1f308];
    /// let rainbow_no_fe0f = vec![0x1f3f3, 0x200d, 0x1f308];
    ///
    /// let rainbow_entry = (vec![EmojiZwjSequence], Some(String::from("rainbow flag")));
    ///
    /// assert!(table.get_map().contains_key(&rainbow));
    /// assert!(table.get_map().contains_key(&rainbow_no_fe0f));
    ///
    /// assert_eq!(*table.get(&rainbow).unwrap(), rainbow_entry);
    /// ```
    pub fn from_files(paths: &[PathBuf]) -> Result<EmojiTable, Error> {
        let mut table = EmojiTable::new();

        for path in paths {
            EmojiTable::expand(&mut table, path)?;
        }
        Ok(table)
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
    /// let path = &PathBuf::from("test_files/unicode/emoji-zwj-sequences.txt");
    /// table.expand(path).unwrap();
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
    pub fn expand(&mut self, path: &Path) -> Result<(), Error> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]+").unwrap();
            static ref RANGE: Regex = Regex::new(r"([a-fA-F0-9]+)\.\.([a-fA-F0-9]+)").unwrap();
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            if let Ok(line) = line {
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

                        if let Some(capt) = RANGE.captures(emoji) {
                            let (start, end) = (&capt[1], &capt[2]);
                            let extension = self.parse_range(start, end, kind).0;
                            self.0.extend(extension);
                        } else {
                            let (seq, entry) = self.parse_sequence(emoji, kind.clone(), description);
                            self.insert(seq, entry);
                            let emoji = emoji.to_lowercase();
                            if emoji.contains("fe0f") {
                                let (seq, entry) = self.parse_sequence(
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

    fn get_description(&self, sequence: &EmojiTableKey) -> Option<String> {
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

            out_table.insert(codepoint, (kinds, existing_description));
        }
        EmojiTable(out_table)
    }

    fn parse_sequence(&self,
                      emoji: &str,
                      kind: EmojiKind,
                      description: Option<&str>,
    ) -> (EmojiTableKey, (Vec<EmojiKind>, Option<String>)) {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]+").unwrap();
        }

        let matches = HEX_SEQUENCE.find_iter(emoji);
        let code_sequences: Vec<u32> = matches
            .map(|sequence| sequence.as_str().to_string())
            .map(|sequence| u32::from_str_radix(&sequence, 16).unwrap_or_default())
            .filter(|codepoint| codepoint > &0)
            .collect();

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

        if let Some(description) = description {
            let description = description.split('#').next().unwrap_or_default().trim();
            let description = String::from(description);
            (code_sequences, (kinds, Some(description)))
        } else {
            (code_sequences, (kinds, existing_description))
        }
    }

    pub fn get_map(&self) -> &HashMap<EmojiTableKey, EmojiTableEntry> {
        &self.0
    }

    /// Inserts a new key-entry pair into the table and returns the last entry if there was one.
    pub fn insert(&mut self, key: EmojiTableKey, entry: EmojiTableEntry) -> Option<EmojiTableEntry> {
        self.0.insert(key, entry)
    }

    pub fn get<T: AsRef<EmojiTableKey>>(&self, index: &T) -> Option<&EmojiTableEntry> {
        let index: &EmojiTableKey = index.as_ref();
        self.0.get(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for EmojiTable {
    fn default() -> Self {
        EmojiTable::new()
    }
}

impl From<HashMap<EmojiTableKey, EmojiTableEntry>> for EmojiTable {
    fn from(table: HashMap<Vec<u32>, (Vec<EmojiKind>, Option<String>), RandomState>) -> Self {
        EmojiTable(table)
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
