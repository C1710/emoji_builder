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

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use regex::{CaptureMatches, Regex};

use crate::emoji::EmojiError::NotAFileName;
use crate::emoji_tables::{EmojiTable, EmojiTableError};
use crate::emoji_tables::EmojiTableError::KeyNotFound;

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
    pub kind: Option<Vec<EmojiKind>>,
    /// The path of the source SVG file for this emoji.
    ///
    /// This can be used in EmojiBuilders to e.g. render an emoji.
    pub svg_path: Option<PathBuf>,
}

/// An internal representation for the different emoji types represented in the Unicode® Tables
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum EmojiKind {
    Emoji,
    EmojiZwjSequence,
    EmojiSequence,
    EmojiPresentation,
    ModifierBase,
    EmojiComponent,
    EmojiKeycapSequence,
    EmojiFlagSequence,
    EmojiModifierSequence,
    Other(String),
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
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let party_face = Emoji::from_sequence("emoji_u1f973.svg", &None).unwrap();
    /// assert_eq!(party_face, Emoji {
    ///     sequence: vec![0x1f973],
    ///     name: None,
    ///     kind: None,
    ///     svg_path: None
    /// });
    /// ```
    ///
    /// ```
    /// use emoji_builder::emoji::{Emoji, EmojiKind};
    /// use std::collections::HashMap;
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let mut table = EmojiTable::new();
    /// table.insert(vec![0x1f914 as u32], (vec![EmojiKind::Emoji], Some(String::from("Thinking Face"))));
    ///
    /// let thinking = Emoji::from_sequence("1f914.png", &Some(table)).unwrap();
    ///
    /// assert_eq!(thinking, Emoji {
    ///     sequence: vec![0x1f914],
    ///     name: Some(String::from("Thinking Face")),
    ///     kind: Some(vec![EmojiKind::Emoji]),
    ///     svg_path: None
    /// });
    /// ```
    pub fn from_sequence(sequence: &str, table: &Option<EmojiTable>) -> Result<Emoji, EmojiError> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"([a-fA-F0-9]{2,})[-_. ]").unwrap();
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
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let seq = vec![0x1f3f3, 0x200d, 0xf308];
    ///
    /// let emoji = Emoji::from_u32_sequence(seq.clone(), &None).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence: seq,
    ///     name: None,
    ///     kind: None,
    ///     svg_path: None
    /// });
    /// ```
    pub fn from_u32_sequence(
        code_sequence: Vec<u32>,
        table: &Option<EmojiTable>,
    ) -> Result<Emoji, EmojiError> {
        if !code_sequence.is_empty() {
            let mut emoji = Emoji::from(code_sequence);
            if let Some(table) = table {
                emoji.set_name(table).unwrap_or_default();
                emoji.set_kind(table).unwrap_or_default();
            }
            Ok(emoji)
        } else {
            Err(EmojiError::NoValidCodepointsFound)
        }
    }

    const FLAG_OFFSET: u32 = 0x1f1a5;
    const REGIONAL_OFFSET: u32 = 0xe0020;
    const CANCEL_TAG: u32 = 0xe007f;
    const BLACK_FLAG: u32 = 0x1f3f4;

    /// Creates an emoji from a flag sequence given in their
    /// ISO-3166-1 representation or a subdivision given in their ISO-3166-2 representation
    /// (i.e. with a dash in between).
    /// Everything after the first dot (`.`) will be ignored (usually file extensions)
    /// # Examples
    /// ```
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let germany = "DE";
    ///
    /// // https://emojipedia.org/flag-for-germany/
    /// let sequence = vec![0x1f1e9, 0x1f1ea];
    ///
    /// let emoji = Emoji::from_flag(germany, &None);
    ///
    /// assert_eq!(emoji.unwrap(), Emoji::from_u32_sequence(sequence, &None).unwrap());
    /// ```
    ///
    /// ```
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let nrw = "de-nw.svg";
    ///
    /// // https://emojipedia.org/flag-for-north-rhine-westphalia-denw/
    /// let sequence = vec![0x1f3f4, 0xe0064, 0xe0065, 0xe006e, 0xe0077, 0xe007f];
    ///
    /// let emoji = Emoji::from_flag(nrw, &None);
    ///
    /// assert_eq!(emoji.unwrap(), Emoji::from_u32_sequence(sequence, &None).unwrap());
    /// ```
    pub fn from_flag(flag: &str, table: &Option<EmojiTable>) -> Result<Emoji, EmojiError> {
        lazy_static! {
            static ref COUNTRY_FLAG: Regex = Regex::new(r"^[A-Z]+$").unwrap();
            static ref REGION_FLAG: Regex = Regex::new(r"^([A-Z]+)-([A-Z]+)$").unwrap();
        }
        // Strip any file extensions
        let flag = flag.split('.').next().unwrap().trim().to_uppercase();

        if COUNTRY_FLAG.is_match(&flag) {
            // ISO-3166-1 country code (DE)
            let codepoints = flag.chars();
            let codepoints = codepoints
                .map(|codepoint| codepoint as u32)
                .map(|codepoint| codepoint + Emoji::FLAG_OFFSET)
                .collect();
            Emoji::from_u32_sequence(codepoints, table)
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
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let path_str = String::from("1f914.svg");
    ///
    /// let path = PathBuf::from(path_str);
    /// let sequence = vec![0x1f914];
    ///
    /// let emoji = Emoji::from_file(path.clone(), &None, false).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kind: None,
    ///     svg_path: Some(path.into())
    /// })
    /// ```
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let path_str = String::from("DE.png");
    ///
    /// let path = PathBuf::from(path_str);
    /// let sequence = vec![0x1f1e9, 0x1f1ea];
    ///
    /// let emoji = Emoji::from_file(path.clone(), &None, true).unwrap();
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kind: None,
    ///     svg_path: Some(path)
    /// })
    /// ```
    pub fn from_file(
        file: PathBuf,
        table: &Option<EmojiTable>,
        flag: bool
    ) -> Result<Emoji, EmojiError> {
        let name = file.file_name();
        if let Some(name) = name {
            if let Some(name) = name.to_str() {
                let mut emoji = if flag {
                    Emoji::from_flag(name, table)
                } else {
                    Emoji::from_sequence(name, table)
                };
                if let Ok(emoji) = &mut emoji {
                    emoji.set_path(file);
                }
                return emoji;
            }
        }
        Err(NotAFileName(file.to_path_buf()))
    }

    /// Performs a lookup in the given `EmojiTable`
    /// and assigns the proper kind attribute to this `Emoji`.
    /// # Example
    /// ```
    /// use std::collections::HashMap;
    /// use emoji_builder::emoji::{EmojiKind, Emoji};
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let mut table = EmojiTable::new();
    /// let sequence = vec![0x1f914];
    /// let kind = vec![EmojiKind::Emoji];
    /// let name = String::from("Thinking Face");
    ///
    /// table.insert(sequence.clone(), (kind.clone(), Some(name.clone())));
    ///
    /// let mut emoji = Emoji::from(sequence.clone());
    /// emoji.set_kind(&table);
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: None,
    ///     kind: Some(kind),
    ///     svg_path: None
    /// });
    /// ```
    pub fn set_kind(&mut self, table: &EmojiTable) -> Result<(), EmojiTableError> {
        let seq = &self.sequence;
        match &table.get(seq) {
            Some((kind, _)) => {
                self.kind = Some(kind.clone());
                Ok(())
            }
            None => Err(KeyNotFound(seq.clone())),
        }
    }

    /// Tries to extract the `EmojiKind` from the Emoji's sequence.
    /// It will only output either `Some(Emoji)`, `Some(EmojiZwjSequence)`, `Some(EmojiSequence)` or
    /// `None` (if the sequence is empty).
    /// # Examples
    /// ```
    /// use emoji_builder::emoji::{Emoji,EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914]);
    ///
    /// let kind = emoji.guess_kind();
    ///
    /// assert_eq!(kind.unwrap(), EmojiKind::Emoji);
    /// ```
    ///
    /// ```
    /// use emoji_builder::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914, 0x200d, 0x42]);
    ///
    /// let kind = emoji.guess_kind();
    ///
    /// assert_eq!(kind, Some(EmojiKind::EmojiZwjSequence));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![0x1f914, 0x42]);
    ///
    /// let kind = emoji.guess_kind();
    ///
    /// assert_eq!(kind, Some(EmojiKind::EmojiSequence));
    /// ```
    ///
    /// ```
    /// use emoji_builder::emoji::{Emoji, EmojiKind};
    ///
    /// let emoji = Emoji::from(vec![]);
    ///
    /// let kind = emoji.guess_kind();
    ///
    /// assert_eq!(kind, None);
    /// ```
    pub fn guess_kind(&self) -> Option<EmojiKind> {
        if self.sequence.is_empty() {
            None
        } else if self.sequence.len() == 1 {
            Some(EmojiKind::Emoji)
        } else if self.sequence.contains(&0x200d) {
            Some(EmojiKind::EmojiZwjSequence)
        } else {
            Some(EmojiKind::EmojiSequence)
        }
    }

    /// Performs a lookup in the given `EmojiTable` and assigns the proper kind attribute to this `Emoji`.
    /// # Example
    /// ```
    /// use std::collections::HashMap;
    /// use emoji_builder::emoji::{EmojiKind, Emoji};
    /// use emoji_builder::emoji_tables::EmojiTable;
    ///
    /// let mut table = EmojiTable::new();
    /// let sequence = vec![0x1f914];
    /// let kind = vec![EmojiKind::Emoji];
    /// let name = String::from("Thinking Face");
    ///
    /// table.insert(sequence.clone(), (kind.clone(), Some(name.clone())));
    ///
    /// let mut emoji = Emoji::from(sequence.clone());
    /// emoji.set_name(&table);
    ///
    /// assert_eq!(emoji, Emoji {
    ///     sequence,
    ///     name: Some(name),
    ///     kind: None,
    ///     svg_path: None
    /// });
    /// ```
    pub fn set_name(&mut self, table: &EmojiTable) -> Result<(), EmojiTableError> {
        let seq = &self.sequence;
        match &table.get(seq) {
            Some((_, name)) => {
                self.name = name.clone();
                Ok(())
            }
            None => Err(KeyNotFound(seq.clone())),
        }
    }

    /// Assigns a given path to the Emoji
    pub fn set_path(&mut self, path: PathBuf) {
        self.svg_path = Some(path.into());
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
            kind: None,
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

impl Into<Vec<u32>> for Emoji {
    fn into(self) -> Vec<u32> {
        self.sequence
    }
}

impl Hash for Emoji {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sequence.hash(state)
    }
}

impl PartialEq<Emoji> for Emoji {
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

impl FromStr for EmojiKind {
    type Err = UnknownEmojiKind;

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        let kind = kind.to_lowercase().trim().replace('_', " ");
        match kind.as_str() {
            "emoji" => Ok(EmojiKind::Emoji),
            "emoji zwj sequence" => Ok(EmojiKind::EmojiZwjSequence),
            "emoji sequence" => Ok(EmojiKind::EmojiSequence),
            "emoji presentation" => Ok(EmojiKind::EmojiPresentation),
            "modifier base" => Ok(EmojiKind::ModifierBase),
            "emoji component" => Ok(EmojiKind::EmojiComponent),
            "emoji keycap sequence" => Ok(EmojiKind::EmojiKeycapSequence),
            "emoji flag sequence" => Ok(EmojiKind::EmojiFlagSequence),
            "emoji modifier sequence" => Ok(EmojiKind::EmojiModifierSequence),
            _ => Err(UnknownEmojiKind(EmojiKind::Other(kind))),
        }
    }
}

/// A very simple wrapper that indicates, that a given string representation of an Emoji kind did
/// not match any of the default cases.
/// If you don't care about that, you can simply ignore it.
pub struct UnknownEmojiKind(EmojiKind);

impl UnknownEmojiKind {
    pub fn get(self) -> EmojiKind {
        self.0
    }
}

impl From<UnknownEmojiKind> for EmojiKind {
    fn from(kind: UnknownEmojiKind) -> Self {
        kind.0
    }
}

#[derive(Debug)]
pub enum EmojiError {
    /// Indicates that either no codepoint sequence has been parsed or that a string didn't
    /// match the recognized patterns for codepoint sequences.
    NoValidCodepointsFound,
    /// Indicates that the given sequence could not be parsed as a flag sequence (i.e. it is not a valid
    /// ISO 3166-1/2 code).
    NoValidFlagSequence,
    /// Indicates that the given `PathBuf` did not find a valid file name
    /// (i.e. "if the path terminates in `..`").
    NotAFileName(PathBuf),
}
