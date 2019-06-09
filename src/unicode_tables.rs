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

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use regex::Regex;

use emoji::EmojiKind;

pub type UnicodeTable = HashMap<Vec<u32>, (Vec<EmojiKind>, Option<String>)>;


pub fn build_table(paths: &[PathBuf]) -> Result<UnicodeTable, Error> {
    let mut table = HashMap::new();

    for path in paths {
        expand(&mut table, path)?;
    }
    Ok(table)
}

pub fn expand(table: &mut UnicodeTable, path: &Path) -> Result<(), Error> {
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
                            None => None
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
                        Err(unknown_kind) => unknown_kind.get()
                    };

                    if let Some(capt) = RANGE.captures(emoji) {
                        let (start, end) = (&capt[1], &capt[2]);
                        let extension = parse_range(table, start, end, kind);
                        table.extend(extension);
                    } else {
                        let (seq, entry) = parse_sequence(table, emoji, kind.clone(), description);
                        table.insert(seq, entry);
                        let emoji = emoji.to_lowercase();
                        if emoji.contains("fe0f") {
                            let (seq, entry) =
                                parse_sequence(table, &emoji.to_lowercase().replace("fe0f", ""), kind, description);
                            table.insert(seq, entry);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}


fn parse_range(table: &UnicodeTable, start: &str, end: &str, kind: EmojiKind) -> UnicodeTable {
    // Start and end are already built from a regular expression that only matches hexadecimal strings
    let start = u32::from_str_radix(start, 16).unwrap();
    let end = u32::from_str_radix(end, 16).unwrap();
    let mut out_table = HashMap::new();
    for codepoint in start..end + 1 {
        let codepoint = vec![codepoint];
        // TODO: Cloning is inefficient
        let mut kinds = match table.get(&codepoint) {
            Some((kinds, _)) => kinds.clone(),
            None => Vec::with_capacity(1)
        };

        kinds.push(kind.clone());

        out_table.insert(codepoint, (kinds, None));
    }
    out_table
}


fn parse_sequence(table: &UnicodeTable, emoji: &str, kind: EmojiKind, description: Option<&str>) -> (Vec<u32>, (Vec<EmojiKind>, Option<String>)) {
    lazy_static! {
        static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]+").unwrap();
    }

    let matches = HEX_SEQUENCE.find_iter(emoji);
    let code_sequences: Vec<u32> = matches
        .map(|sequence| sequence.as_str().to_string())
        .map(|sequence| u32::from_str_radix(&sequence, 16).unwrap_or_default())
        .filter(|codepoint| codepoint > &0)
        .collect();


    // TODO: Cloning is inefficient
    let mut kinds = match table.get(&code_sequences) {
        Some((kinds, _)) => kinds.clone(),
        None => Vec::with_capacity(1)
    };

    kinds.push(kind);

    if let Some(description) = description {
        let description = description.split('#').next().unwrap_or_default().trim();
        let description = String::from(description);
        (code_sequences, (kinds, Some(description)))
    } else {
        (code_sequences, (kinds, None))
    }
}