/*
 * Copyright 2019 Constantin A.
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

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use regex::{CaptureMatches, Regex};

use unicode_tables::UnicodeTable;

/// A struct that holds information for one particular emoji (sequence)
#[derive(Debug)]
#[derive(Eq)]
#[derive(Clone)]
pub struct Emoji {
    pub sequence: Vec<u32>,
    pub name: Option<String>,
    pub kind: Option<Vec<EmojiKind>>,
    pub svg_path: Option<Box<Path>>,
}

/// An internal representation for the different emoji types represented in the unicode tables
#[derive(Debug)]
#[derive(Clone)]
#[derive(Hash)]
#[derive(PartialEq)]
#[derive(Eq)]
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
    /// (optionally with a `UnicodeTable` for metadata)
    /// # Examples
    /// ```
    /// use emoji_builder::emoji::Emoji;
    ///
    /// let party_face = Emoji::from_sequence("emoji_u1f973.svg", None);
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
    ///
    /// let mut table = HashMap::new();
    /// table.insert(vec![u32::from(0x1f914)], (vec![EmojiKind::Emoji], "Thinking Face"));
    ///
    /// let thinking = Emoji::from_sequence("1f914.png", Some(table));
    /// ```
    pub fn from_sequence(sequence: &str, table: Option<Arc<UnicodeTable>>) -> Option<Emoji> {
        lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[u_ -]([a-fA-F0-9]+)").unwrap();
        }
        let matches: CaptureMatches = HEX_SEQUENCE.captures_iter(sequence);
        let code_sequences: Vec<u32> = matches
            .map(|sequence| sequence[1].to_string())
            .map(|sequence| u32::from_str_radix(&sequence, 16).unwrap_or(0))
            .filter(|codepoint| codepoint > &0)
            .collect();
        Emoji::from_u32_sequence(code_sequences, table)
    }

    fn from_u32_sequence(code_sequences: Vec<u32>, table: Option<Arc<UnicodeTable>>) -> Option<Emoji> {
        if !code_sequences.is_empty() {
            let mut emoji = Emoji {
                sequence: code_sequences,
                name: None,
                kind: None,
                svg_path: None,
            };
            if let Some(table) = table {
                emoji.set_name(table.clone());
                emoji.set_kind(table);
            }
            Some(emoji)
        } else {
            None
        }
    }

    const FLAG_OFFSET: u32 = 0x1f1a5;
    const REGIONAL_OFFSET: u32 = 0xe0020;

    pub fn from_flag(flag: &str, table: Option<Arc<UnicodeTable>>) -> Option<Emoji> {
        lazy_static! {
            static ref COUNTRY_FLAG: Regex = Regex::new(r"[A-Z]+").unwrap();
            static ref REGION_FLAG: Regex = Regex::new(r"([A-Z]+)-([A-Z]+)").unwrap();
        }
        let flag = flag.trim().to_uppercase();
        if COUNTRY_FLAG.is_match(&flag) {
            let codepoints = flag.chars();
            let codepoints = codepoints
                .map(|codepoint| u32::from(codepoint))
                .map(|codepoint| codepoint + Emoji::FLAG_OFFSET)
                .collect();
            Emoji::from_u32_sequence(codepoints, table)
        } else if let Some(capt) = REGION_FLAG.captures(&flag) {
            let mut flag: String = String::from(&capt[1]);
            flag.push_str(&capt[2]);
            let codepoints = flag.chars();
            let mut codepoints: Vec<u32> = codepoints
                .map(|codepoint| u32::from(codepoint))
                .map(|codepoint| codepoint + Emoji::REGIONAL_OFFSET)
                .collect();
            codepoints.insert(0, 0xf3f4);
            Emoji::from_u32_sequence(codepoints, table)
        } else {
            None
        }
    }

    pub fn from_file(file: &Path, table: Option<Arc<UnicodeTable>>, flag: bool) -> Option<Emoji> {
        let name = file.file_stem();
        if let Some(name) = name {
            if let Some(name) = name.to_str() {
                let mut emoji = if flag {
                    Emoji::from_flag(name, table)
                } else {
                    Emoji::from_sequence(name, table)
                };
                if let Some(emoji) = &mut emoji {
                    emoji.set_path(file);
                }
                return emoji;
            }
        }
        None
    }

    ///
    pub fn set_kind(&mut self, table: Arc<UnicodeTable>) -> Option<()> {
        let seq = &self.sequence;
        match table.get(seq) {
            Some((kind, _)) => Some(self.kind = Some(kind.clone())),
            None => None
        }
    }

    fn guess_kind(&mut self) -> EmojiKind {
        if self.sequence.len() == 1 {
            EmojiKind::Emoji
        } else if self.sequence.contains(&0x200d) {
            EmojiKind::EmojiZwjSequence
        } else {
            EmojiKind::EmojiSequence
        }
    }

    pub fn set_name(&mut self, table: Arc<UnicodeTable>) -> Option<()> {
        let seq = &self.sequence;
        match table.get(seq) {
            Some((_, name)) => Some(self.name = name.clone()),
            None => None
        }
    }

    pub fn set_path(&mut self, path: &Path) {
        self.svg_path = Some(path.into());
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
            _ => Err(UnknownEmojiKind(EmojiKind::Other(kind)))
        }
    }
}

pub struct UnknownEmojiKind(EmojiKind);

impl UnknownEmojiKind {
    pub fn get(self) -> EmojiKind {
        self.0
    }
}