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

use itertools::Itertools;
use regex::Regex;

use crate::emojis::emoji_kind::EmojiKind;
use crate::tables::emoji_tables::EmojiTableKey;

pub fn strip_fe0f(codepoint_with_fe0f: &[u32]) -> EmojiTableKey {
    codepoint_with_fe0f.iter()
        .filter(|codepoint| **codepoint != 0xfe0f)
        .copied()
        .collect()
}

pub fn insert_in_order<T>(existing: &mut Vec<T>, new: Option<T>)
    where T: Eq + Ord {
    if let Some(new) = new {
        if !existing.contains(&new) {
            existing.insert(existing.binary_search(&new).unwrap_err(), new);
        }
    }
}


pub fn update_description(old_description: &mut Option<String>, new_description: Option<&str>) {
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

pub fn add_kind(existing_kinds: &mut Vec<EmojiKind>, kind: Option<EmojiKind>) {
    insert_in_order(existing_kinds, kind)
}

pub fn key_from_str(raw_codepoints: &str) -> EmojiTableKey {
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

