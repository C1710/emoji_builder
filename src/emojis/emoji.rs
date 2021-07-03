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
//! The main data structs for single emojis.

use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::RangeInclusive;
use std::path::PathBuf;

use itertools::Itertools;
use regex::{CaptureMatches, Regex};

use crate::emoji_tables::{EmojiTable, EmojiTableError};
use crate::emoji_tables::EmojiTableError::KeyNotFound;
use crate::emojis::emoji::EmojiError::NotAFileName;
use crate::emojis::emoji_kind::EmojiKind::{EmojiFlagSequence, EmojiKeycapSequence};
use crate::emojis::emoji_kind::EmojiKind;

/// A struct that holds information for one particular emoji (which might also be a sequence).
#[derive(Debug, Eq, Clone)]
pub struct Emoji {
    /// The sequence of Unicode® character codepoints that represents this emoji.
    pub sequence: Vec<u32>,
    /// The name/description (if assigned) for the Emoji
    ///
    /// This is particularly useful for error messages.
    pub name: Option<String>,
    /// The `EmojiKind`s that are available for this code sequence.
    pub kinds: Option<Vec<EmojiKind>>,
    /// The path of the source SVG file for this emoji.
    ///
    /// This can be used in EmojiBuilders to e.g. render an emoji.
    // TODO: Maybe it would be wiser to use something else than the path to the file here.
    //       Especially, if this should ever be ported to WASM it would be useful to not use paths.
    pub svg_path: Option<PathBuf>
}

impl Emoji {
    /// Parses a character sequence (e.g. from a filename) into an emoji object
    /// (optionally with an `EmojiTable` for metadata).
    /// Please note that after the last codepoint there needs to be either a dash (`-`),
    /// underscore (`_`), space (` `) or dot (`.`).
    /// These are also the  allowed delimiters.
    ///
    /// If you wish to use another delimiter, you'll (currenty) need to use `from_u32_sequence`.
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let party_face = Emoji::from_sequence("emoji_u1f973.svg", None).unwrap();
    /// assert_eq!(party_face, Emoji {
    ///     sequence: vec![0x1f973],
    ///     name: None,
    ///     kinds: None,
    ///     svg_path: None
    /// });
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    /// use std::collections::HashMap;
    /// use emoji_builder::emoji_tables::{EmojiTable };
    ///
    /// let mut table = EmojiTable::new();
    /// table.insert(vec![0x1f914 as u32], (vec![EmojiKind::Emoji], Some(String::from("Thinking Face")), vec![EmojiStatus::FullyQualified]));
    ///
    /// let thinking = Emoji::from_sequence("1f914.png", Some(&table)).unwrap();
    ///
    /// assert_eq!(thinking, Emoji {
    ///     sequence: vec![0x1f914],
    ///     name: Some(String::from("Thinking Face")),
    ///     kinds: Some(vec![EmojiKind::Emoji]),
    ///     svg_path: None
    /// });
    /// ```
    pub fn from_sequence(sequence: &str, table: Option<&EmojiTable>) -> Result<Emoji, EmojiError> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"([a-fA-F0-9]{1,8})([-_. ]|$)").unwrap();
        }
        let matches: CaptureMatches = HEX_SEQUENCE.captures_iter(&sequence);
        let code_sequences: Vec<u32> = matches
            .map(|sequence| sequence[1].to_string())
            .map(|sequence| u32::from_str_radix(&sequence, 16).unwrap_or(0))
            .filter(|codepoint| *codepoint > 0)
            .collect();
        Emoji::from_u32_sequence(code_sequences, table)
    }

    /// Generates an Emoji from a given codepoint sequence
    /// (and maybe an `EmojiTable` for additional metadata).
    /// If the sequence is empty, it will return an error.
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let seq = vec![0x1f3f3, 0x200d, 0xf308];
    ///
    /// let emoji = Emoji::from_u32_sequence(seq.clone(), None).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence: seq,
    ///     name: None,
    ///     kinds: None,
    ///     svg_path: None
    /// });
    /// ```
    pub fn from_u32_sequence(
        code_sequence: Vec<u32>,
        table: Option<&EmojiTable>,
    ) -> Result<Emoji, EmojiError> {
        if !code_sequence.is_empty() {
            let mut emoji = Emoji::from(code_sequence);
            if let Some(table) = table {
                emoji.set_name(table).unwrap_or_default();
                emoji.set_kind(table).unwrap_or_default();
            }
            Ok(emoji)
        } else {
            Err(EmojiError::NoValidCodepointsFound(String::from("Empty code sequence")))
        }
    }

    const FLAG_OFFSET: u32 = 0x1f185;
    const REGIONAL_OFFSET: u32 = 0xe0000;
    const CANCEL_TAG: u32 = 0xe007f;
    const BLACK_FLAG: u32 = 0x1f3f4;


    /// Creates an emoji from a flag sequence given in their
    /// ISO-3166-1 representation or a subdivision given in their ISO-3166-2 representation
    /// (i.e. with a dash in between).
    ///
    /// Everything after the first dot (`.`) will be ignored (usually file extensions)
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let germany = "DE";
    ///
    /// // https://emojipedia.org/flag-for-germany/
    /// let sequence = vec![0x1f1e9, 0x1f1ea];
    ///
    /// let emoji = Emoji::from_flag(germany, None);
    ///
    /// assert_eq!(emoji.unwrap(), Emoji::from_u32_sequence(sequence, None).unwrap());
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let nrw = "de-nw.svg";
    ///
    /// // https://emojipedia.org/flag-for-north-rhine-westphalia-denw/
    /// let sequence = vec![0x1f3f4, 0xe0064, 0xe0065, 0xe006e, 0xe0077, 0xe007f];
    ///
    /// let emoji = Emoji::from_flag(nrw, None);
    ///
    /// assert_eq!(emoji.unwrap(), Emoji::from_u32_sequence(sequence, None).unwrap());
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let salzburg = "at-5.boo";
    ///
    /// // https://emojipedia.org/flag-for-salzburg-at5/
    /// let sequence = vec![0x1f3f4, 0xe0061, 0xe0074, 0xe0035, 0xe007f];
    ///
    /// let emoji = Emoji::from_flag(salzburg, None);
    ///
    /// assert_eq!(emoji.unwrap(), Emoji::from_u32_sequence(sequence, None).unwrap());
    /// ```
    pub fn from_flag(flag: &str, table: Option<&EmojiTable>) -> Result<Emoji, EmojiError> {
        lazy_static! {
            static ref COUNTRY_FLAG: Regex = Regex::new(r"^[a-z]+$").unwrap();
            static ref REGION_FLAG: Regex = Regex::new(r"^([a-z]+)-([a-z0-9]+)$").unwrap();
        }
        // Strip any file extensions
        let flag = flag.split('.').next().unwrap().trim().to_lowercase();

        if COUNTRY_FLAG.is_match(&flag) {
            // ISO-3166-1 country code (DE)
            let codepoints = flag.chars();
            let codepoints = codepoints
                .map(|codepoint| codepoint as u32)
                .map(|codepoint| codepoint + Emoji::FLAG_OFFSET)
                .collect();
            let mut emoji = Emoji::from_u32_sequence(codepoints, table);
            if let Ok(emoji) = &mut emoji {
                if let Some(kind) = &mut emoji.kinds {
                    kind.push(EmojiKind::EmojiFlagSequence);
                }
            };
            emoji
        } else if let Some(capt) = REGION_FLAG.captures(&flag) {
            // ISO 3166-2 subdivision code (DE-NW)
            let mut flag = String::with_capacity(capt[1].len() + capt[2].len() + 1);
            // The 'X' is just a placeholder which will be replaced by the BLACK_FLAG codepoint later
            flag.push('X');
            flag.push_str(&capt[1]);
            flag.push_str(&capt[2]);

            let codepoints = flag.chars();
            let mut codepoints: Vec<u32> = codepoints
                .map(|codepoint| codepoint as u32)
                .map(|codepoint| codepoint + Emoji::REGIONAL_OFFSET)
                .chain(vec![Emoji::CANCEL_TAG])
                .collect();
            codepoints[0] = Emoji::BLACK_FLAG;
            Emoji::from_u32_sequence(codepoints, table)
        } else {
            Err(EmojiError::NoValidFlagSequence)
        }
    }

    /// Creates an emoji data object from a given file path.
    /// **Note:** it will _not_ use anything inside the file.
    /// It also does _not_ check whether the file exists
    /// # Examples
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let path_str = String::from("1f914.svg");
    ///
    /// let path = PathBuf::from(path_str);
    /// let sequence = vec![0x1f914];
    ///
    /// let emoji = Emoji::from_path(path.clone(), None, false).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kinds: None,
    ///     svg_path: Some(path.into())
    /// })
    /// ```
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let path_str = String::from("DE.png");
    ///
    /// let path = PathBuf::from(path_str);
    /// let sequence = vec![0x1f1e9, 0x1f1ea];
    ///
    /// let emoji = Emoji::from_path(path.clone(), None, true).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kinds: None,
    ///     svg_path: Some(path)
    /// })
    /// ```
    pub fn from_path(
        file: PathBuf,
        table: Option<&EmojiTable>,
        flag: bool
    ) -> Result<Emoji, EmojiError> {
        let name = file.file_stem();
        if let Some(name) = name {
            if let Some(name) = name.to_str() {
                let mut emoji = if flag {
                    Emoji::from_flag(name, table)
                } else {
                    // First, try to find the emoji by its name, then by its sequence
                    match table {
                        Some(table) => match Self::from_name(name, table) {
                            Ok(emoji) => Ok(emoji),
                            Err(err) => if let EmojiError::NoValidCodepointsFound(_) = err {
                                debug!("{} is not a recognized emoji name", name);
                                // Now try to parse it as a sequence
                                Self::from_sequence(name, Some(table))
                            } else {
                                // If it was something else than a failed lookup, pass the error
                                Err(err)
                            }
                        },
                        // In this case, we have no other choice but to interpret it as a sequence
                        None => Self::from_sequence(name, None)
                    }
                };
                if let Ok(emoji) = &mut emoji {
                    emoji.set_path(file);
                }
                return emoji;
            }
        }
        Err(NotAFileName(file.to_path_buf()))
    }

    fn from_name(name: &str, table: &EmojiTable) -> Result<Emoji, EmojiError> {
        match table.get_by_name(name) {
            Some((sequence, (kinds, _, _))) => Ok(Emoji {
                sequence,
                name: Some(name.to_string()),
                kinds: Some(kinds.clone()),
                svg_path: None,
            }),
            None => Err(EmojiError::NoValidCodepointsFound(name.to_owned()))
        }
    }

    /// Performs a lookup in the given `EmojiTable`
    /// and assigns the proper kind attribute to this `Emoji`.
    /// # Example
    /// ```
    /// use std::collections::HashMap;
    /// use emoji_builder::emojis::emoji::{EmojiKind, Emoji};
    /// use emoji_builder::emoji_tables::EmojiTable;
    /// use emoji_builder::emoji_tables::EmojiStatus;
    ///
    /// let mut table = EmojiTable::new();
    /// let sequence = vec![0x1f914];
    /// let kind = vec![EmojiKind::Emoji];
    /// let name = String::from("Thinking Face");
    ///
    /// table.insert(sequence.clone(), (kind.clone(), Some(name.clone()), vec![EmojiStatus::FullyQualified]));
    ///
    /// let mut emoji = Emoji::from(sequence.clone());
    /// emoji.set_kind(&table);
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kinds: Some(kind),
    ///     svg_path: None
    /// });
    /// ```
    pub fn set_kind(&mut self, table: &EmojiTable) -> Result<(), EmojiTableError> {
        let seq = &self.sequence;
        match &table.get(seq) {
            Some((kind, _, _)) => {
                self.kinds = Some(kind.clone());
                Ok(())
            }
            None => Err(KeyNotFound(seq.clone())),
        }
    }

    /// Tries to extract the `EmojiKind` from the Emoji's sequence.
    /// Currently the following emoji kinds can be detected:
    /// - `Emoji`
    /// - `EmojiSequence`
    /// - `EmojiZwjSequence`
    /// - `EmojiFlagSequence`
    /// - `EmojiKeycapSequence`
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji,EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914]);
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind.unwrap(), vec![EmojiKind::Emoji]);
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914, 0x200d, 0x42]);
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind, Some(vec![EmojiKind::EmojiZwjSequence]));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914, 0x42]);
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind, Some(vec![EmojiKind::EmojiSequence]));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![]);
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind, None);
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    /// use emoji_builder::emojis::emoji::EmojiKind::{EmojiSequence, EmojiFlagSequence};
    ///
    /// let emoji = Emoji::from_flag("DE", None).unwrap();
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind, Some(vec![EmojiFlagSequence, EmojiSequence]));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::{Emoji, EmojiKind};
    /// use emoji_builder::emojis::emoji::EmojiKind::{EmojiSequence, EmojiKeycapSequence};
    ///
    /// let emoji = Emoji::from(vec![0x2a, 0xfe0f, 0x20e3]);
    ///
    /// let kind = emoji.guess_kinds();
    ///
    /// assert_eq!(kind, Some(vec![EmojiKeycapSequence, EmojiSequence]));
    /// ```
    pub fn guess_kinds(&self) -> Option<Vec<EmojiKind>> {
        if self.sequence.is_empty() {
            None
        } else if self.sequence.len() == 1 {
            Some(vec![EmojiKind::Emoji])
        } else {
            let flag = self.is_flag();
            let keycap = self.sequence.contains(&0x20e3);

            let mut kinds = Vec::with_capacity(1 + flag as usize + keycap as usize);
            if flag {
                kinds.push(EmojiKind::EmojiFlagSequence);
            }
            if keycap {
                kinds.push(EmojiKeycapSequence)
            }
            if self.sequence.contains(&0x200d) {
                kinds.push(EmojiKind::EmojiZwjSequence);
            } else {
                kinds.push(EmojiKind::EmojiSequence);
            }
            Some(kinds)
        }
    }

    /// Performs a lookup in the given `EmojiTable` and assigns the proper kind attribute to this `Emoji`.
    /// # Example
    /// ```
    /// use std::collections::HashMap;
    /// use emoji_builder::emojis::emoji::{EmojiKind, Emoji};
    /// use emoji_builder::emoji_tables::{EmojiTable, EmojiStatus};
    ///
    /// let mut table = EmojiTable::new();
    /// let sequence = vec![0x1f914];
    /// let kind = vec![EmojiKind::Emoji];
    /// let name = String::from("Thinking Face");
    ///
    /// table.insert(sequence.clone(), (kind.clone(), Some(name.clone()), vec![EmojiStatus::FullyQualified]));
    ///
    /// let mut emoji = Emoji::from(sequence.clone());
    /// emoji.set_name(&table);
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: Some(name),
    ///     kinds: None,
    ///     svg_path: None
    /// });
    /// ```
    pub fn set_name(&mut self, table: &EmojiTable) -> Result<(), EmojiTableError> {
        let seq = &self.sequence;
        match &table.get(seq) {
            Some((_, name, _)) => {
                self.name = name.clone();
                Ok(())
            }
            None => Err(KeyNotFound(seq.clone())),
        }
    }

    /// Assigns a given path to the Emoji
    pub fn set_path(&mut self, path: PathBuf) {
        self.svg_path = Some(path);
    }

    const COUNTRY_RANGE: RangeInclusive<u32> = 0x1f1e6..=0x1f1ff;
    const REGION_LETTERS: RangeInclusive<u32> = 0xe0061..=0xe007a;
    const REGION_DIGITS: RangeInclusive<u32> = 0xe0030..=0xe0039;

    /// Returns the ISO 3166-1/2 code in upper case if this emoji represents a flag sequence.
    /// Whether it is a flag is decided by its structure.
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let germany = Emoji::from_flag("de", None).unwrap();
    ///
    /// assert_eq!(germany.get_flag_name().unwrap(), "DE");
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let thinking = Emoji::from_u32_sequence(vec![0x1f914], None).unwrap();
    ///
    /// assert_eq!(thinking.get_flag_name(), None);
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let salzburg = Emoji::from_flag("AT-5", None).unwrap();
    ///
    /// assert_eq!(salzburg.get_flag_name().unwrap(), "AT-5");
    /// ```
    pub fn get_flag_name(&self) -> Option<String> {
        self.get_country_name().or_else(|| self.get_subdiv_name())
    }

    fn get_country_name(&self) -> Option<String> {
        if self.is_country_flag() {
            let country: String = self.sequence.iter()
                .map(|codepoint| codepoint - Self::FLAG_OFFSET)
                // We're in the ASCII range now
                .map(|codepoint| codepoint as u8)
                .map(|codepoint| codepoint as char)
                .collect();
            Some(country.to_uppercase())
        } else {
            None
        }
    }

    /// Returns the ISO 3166-2 code (if this is a subdivision flag)
    fn get_subdiv_name(&self) -> Option<String> {
        if self.is_subdiv_flag() {
            let seq = &self.sequence;
            let last_index = seq.len() - 1;
            let country = seq[1..3].iter()
                .map(|codepoint| codepoint - Self::REGIONAL_OFFSET)
                // We're back to ASCII
                .map(|codepoint| (codepoint as u8) as char);
            let subdiv = seq[3..last_index].iter()
                .map(|codepoint| codepoint - Self::REGIONAL_OFFSET)
                // We're back to ASCII
                .map(|codepoint| (codepoint as u8) as char);
            let name: String = country.chain(vec!['-']).chain(subdiv).collect();
            Some(name.to_uppercase())
        } else {
            None
        }
    }

    /// Checks whether this emoji represents a flag by either formal reasons
    /// (i.e. it includes the kind `EmojiFlagSequence`) or if its codepoints are
    /// valid country or subdivision flags
    /// # Examples
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let germany = Emoji::from_flag("DE", None).unwrap();
    ///
    /// assert!(germany.is_flag());
    /// ```
    pub fn is_flag(&self) -> bool {
        let empty = vec![];
        let kinds = match &self.kinds {
            Some(kinds) => kinds,
            None => &empty
        };
        kinds.contains(&EmojiFlagSequence)
            || self.is_country_flag()
            || self.is_subdiv_flag()
    }

    /// Checks whether this is a country's flag (e.g. DE, EU, etc.)
    pub fn is_country_flag(&self) -> bool {
        !self.sequence.is_empty()
            && self.sequence.iter()
            .all(|codepoint| Self::COUNTRY_RANGE.contains(codepoint))
    }

    /// Checks whether this is a subdivision flag (e.g. DE-NW, AT-5, US-CA, ...)
    pub fn is_subdiv_flag(&self) -> bool {
        let seq = &self.sequence;
        let last_index = seq.len() - 1;
        seq.len() >= 5
            && seq[0] == Self::BLACK_FLAG
            && *seq.last().unwrap_or(&0u32) == Self::CANCEL_TAG
            && seq[1..last_index].iter()
            .all(|codepoint| Self::REGION_LETTERS.contains(codepoint) ||
                Self::REGION_DIGITS.contains(codepoint))
    }

    /// Returns the emoji itself
    /// ## Example
    /// ```
    ///
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// // Face with heart eyes
    /// let emoji = Emoji::from_u32_sequence(vec![0x1f60d], None).unwrap();
    ///
    /// assert_eq!(String::from("😍"), emoji.display_emoji());
    /// ```
    pub fn display_emoji(&self) -> String {
        self.sequence.iter().filter_map(|codepoint| char::from_u32(*codepoint))
            .collect()
    }
    
    pub fn alias(&self, alias_sequence: Vec<u32>) -> Self {
        Self {
            sequence: alias_sequence,
            ..self.clone()
        }
    }
}

impl From<&[u32]> for Emoji {
    fn from(sequence: &[u32]) -> Self {
        Emoji::from(Vec::from(sequence))
    }
}

impl From<Vec<u32>> for Emoji {
    fn from(sequence: Vec<u32>) -> Self {
        Emoji {
            sequence,
            name: None,
            kinds: None,
            svg_path: None,
        }
    }
}

impl AsRef<Vec<u32>> for Emoji {
    fn as_ref(&self) -> &Vec<u32> {
        &self.sequence
    }
}

impl AsRef<[u32]> for Emoji {
    fn as_ref(&self) -> &[u32] {
        &self.sequence.as_ref()
    }
}

impl From<Emoji> for Vec<u32> {
    fn from(emoji: Emoji) -> Self {
        emoji.sequence
    }
}

impl Hash for Emoji {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sequence.hash(state)
    }
}

impl PartialEq<Emoji> for Emoji {
    /// Compares two Emojis by their code sequence
    fn eq(&self, other: &Emoji) -> bool {
        self.sequence == other.sequence
    }
}

impl PartialEq<[u32]> for Emoji {
    fn eq(&self, other: &[u32]) -> bool {
        self.sequence == other
    }
}

impl PartialEq<Emoji> for [u32] {
    fn eq(&self, other: &Emoji) -> bool {
        other.sequence == self
    }
}

impl PartialOrd for Emoji {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.sequence.partial_cmp(&other.sequence)
    }
}

impl Ord for Emoji {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sequence.cmp(&other.sequence)
    }
}

impl Display for Emoji {
    /// Tries to show the appropriate (if possible human-understandable) name for this emoji.
    /// If the name attribute is not `None`, it will output that one.
    /// If not it will either output the flag sequence (e.g. `Flag EU`) or the code sequence
    /// in square brackets (e.g. `[1F3F3-FE0F-200D-1F308]`).
    /// # Examples
    /// ```
    ///
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let thinking = Emoji::from_u32_sequence(vec![0x1f914], None).unwrap();
    ///
    /// assert_eq!("[1F914]", format!("{}", thinking));
    /// ```
    ///
    /// ```
    ///
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let rainbow = Emoji::from_u32_sequence(vec![0x1f3f3, 0xfe0f, 0x200d, 0x1f308], None).unwrap();
    ///
    /// assert_eq!("[1F3F3-FE0F-200D-1F308]", format!("{}", rainbow));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let nrw = Emoji::from_flag("de-nw", None).unwrap();
    ///
    /// assert_eq!("Flag DE-NW", format!("{}", nrw));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emojis::emoji::Emoji;
    ///
    /// let mut party = Emoji::from_u32_sequence(vec![0x1f973], None).unwrap();
    /// party.name = Some(String::from("Party face"));
    ///
    /// assert_eq!("Party face", format!("{}", party));
    /// ```
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}", name)
        } else if let Some(name) = self.get_flag_name() {
            write!(f, "Flag {}", name)
        } else {
            write!(f, "[{}]", self.sequence.iter()
                .map(|codepoint| format!("{:X}", codepoint))
                .join("-"))
        }
    }
}

#[derive(Debug)]
/// An error that can occur while creating an [Emoji]
pub enum EmojiError {
    /// Indicates that either no codepoint sequence has been parsed or that a string didn't
    /// match the recognized patterns for codepoint sequences or that the given table does not contain
    /// the name of the emoji.
    NoValidCodepointsFound(String),
    /// Indicates that the given sequence could not be parsed as a flag sequence (i.e. it is not a valid
    /// ISO 3166-1/2 code).
    NoValidFlagSequence,
    /// Indicates that the given `PathBuf` did not find a valid file name
    /// (i.e. "if the path terminates in `..`").
    NotAFileName(PathBuf),
}
