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


use regex::{Regex, Captures};
use crate::emojis::emoji_kind::EmojiKind;
use itertools::Itertools;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct EmojiData<'a> {
    pub codepoints_content: EmojiDataCodepoints<'a>,
    pub codepoints: &'a str,
    pub kind: &'a str,
    pub name: Option<&'a str>
}

#[derive(Debug)]
pub enum EmojiDataCodepoints<'a> {
    Range(EmojiDataRange<'a>),
    Sequence(EmojiDataSequence<'a>)
}

#[derive(Debug)]
pub struct EmojiDataRange<'a> {
    pub range: &'a str,
    pub range_start: &'a str,
    pub range_end: &'a str,
}

#[derive(Debug)]
pub struct EmojiDataSequence<'a> {
    pub sequence: &'a str
}

impl<'a> From<Captures<'a>> for EmojiData<'a> {
    fn from(captures: Captures<'a>) -> Self {
        let codepoints = captures.name("codepoints").unwrap().as_str();
        let codepoints_content = if let Some(range) = captures.name("range") {
            let range = range.as_str();
            let range_start = captures.name("range_start").unwrap().as_str();
            let range_end = captures.name("range_end").unwrap().as_str();
            EmojiDataCodepoints::Range(
                EmojiDataRange {
                    range,
                    range_start,
                    range_end
                }
            )
        } else {
            let sequence = captures.name("sequence").unwrap().as_str();
            EmojiDataCodepoints::Sequence(
                EmojiDataSequence {
                    sequence
                }
            )
        };
        let kind = captures.name("kind").unwrap().as_str();
        let name = captures.name("name").map(|name| name.as_str());

        Self {
            codepoints_content,
            codepoints,
            kind,
            name
        }
    }
}

pub fn data_regex() -> &'static Regex {
    lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]{1,8}").unwrap();
            static ref RANGE: Regex = Regex::new(&format!(r"(?P<range>(?P<range_start>{hex})\.\.(?P<range_end>{hex}))", hex = &*HEX_SEQUENCE)).unwrap();
            static ref SEQUENCE: Regex = Regex::new(&format!(r"(?P<sequence>({hex})(\s+({hex}))*)", hex = &*HEX_SEQUENCE)).unwrap();
            static ref EMOJI_REGEX: Regex = Regex::new(&format!(r"(?P<codepoints>{}|{})", &*RANGE, &*SEQUENCE)).unwrap();
            static ref EMOJI_KIND_REGEX: Regex = Regex::new(&format!(r"(?P<kind>{}+)", EmojiKind::regex())).unwrap();
            static ref DATA_REGEX: Regex = Regex::new(&format!(r"^{}\s*;\s*{}\s*(;(?P<name>.*)\s*)?(#.*)?$", &*EMOJI_REGEX, &*EMOJI_KIND_REGEX)).unwrap();
    }

    &*DATA_REGEX
}

#[derive(Debug)]
pub struct EmojiTest<'a> {
    pub sequence: &'a str,
    pub status: &'a str,
    pub emoji: Option<&'a str>,
    pub version: &'a str,
    pub description: &'a str
}

impl<'a> From<Captures<'a>> for EmojiTest<'a> {
    fn from(captures: Captures<'a>) -> Self {
        let (sequence, status, version, description) =
            vec!["sequence", "status", "version", "description"].iter()
                .map(|name| captures.name(name).unwrap().as_str())
                .collect_tuple()
                .unwrap();
        let emoji = captures.name("emoji").map(|match_| match_.as_str());

        Self {
            sequence,
            status,
            emoji,
            version,
            description
        }
    }
}

const EMOJI_SEQUENCE_SPACE_REGEX: &str = r"(?P<sequence>([A-F0-9a-f]{1,8})(\s+([A-F0-9a-f]{1,8}))*)";
const EMOJI_STATUS_REGEX: &str = r"(?P<status>component|fully-qualified|minimally-qualified|unqualified)";
const EMOJI_NAME_REGEX: &str = r"(?P<emoji>.*)?\s*E(?P<version>\d+.\d+)\s+(?P<description>.+)";

/// The syntax of these files is:
/// `Codepoint ; ("component"|"fully-qualified"|"minimally-qualified"|"unqualified") # Emoji "E"Version Emoji name`
pub fn test_regex() -> &'static Regex {
    lazy_static! {
            static ref EMOJI_TEST_REGEX: Regex = Regex::new(&format!(r"(?i)^{}\s*;\s*{}\s*#\s*{}$",
                                               EMOJI_SEQUENCE_SPACE_REGEX,
                                               EMOJI_STATUS_REGEX,
                                               EMOJI_NAME_REGEX)
            ).unwrap();
    };
    &*EMOJI_TEST_REGEX
}

fn unified() -> &'static regex::RegexSet {
    lazy_static! {
        static ref UNIFIED_REGEX: regex::RegexSet = regex::RegexSet::new(&[
            data_regex().as_str(),
            test_regex().as_str()
        ]).unwrap();
    }
    
    &*UNIFIED_REGEX
}


impl<'a> TryFrom<&'a str> for EmojiTest<'a> {
    type Error = ();

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let captures = test_regex().captures(value).ok_or(())?;
        Ok(Self::from(captures))
    }
}

impl<'a> TryFrom<&'a str> for EmojiData<'a> {
    type Error = ();

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let captures = data_regex().captures(value).ok_or(())?;
        Ok(Self::from(captures))
    }
}

pub fn match_line(line: &str) -> Option<EmojiFileEntry> {
    debug!("Parsing: {}", line);
    let entry = EmojiData::try_from(line).map(EmojiFileEntry::Data)
        .or_else(|_| EmojiTest::try_from(line).map(EmojiFileEntry::Test))
        .ok();
    debug!("Parsed:  {:?}", entry);
    entry
}

#[derive(Debug)]
pub enum EmojiFileEntry<'a> {
    Data(EmojiData<'a>),
    Test(EmojiTest<'a>)
}