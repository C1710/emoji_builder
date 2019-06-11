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

use std::collections::HashSet;
use std::fs;
use std::iter::FromIterator;

use crate::emoji::Emoji;
use crate::emoji::EmojiKind::EmojiZwjSequence;
use crate::emoji_tables;

const SVG_PATH: &str = "test_files/svg";
const TABLES_PATH: &str = "test_files/unicode";

// data - data-duplicates + zwj sequences w/o fe0f + 2*(zwj sequences with fe0f)
const TABLE_ENTRIES: usize = 76 - 21 + 1 + 2 * (9);

// The number of files/entries expected
const EMOJIS: usize = 7;
const TABLES: usize = 2;


#[test]
fn emoji_build() {
    let table_paths: Vec<_> = fs::read_dir(TABLES_PATH).expect("Couldn't read the unicode directory")
        .filter(std::result::Result::is_ok)
        .map(std::result::Result::unwrap)
        .map(|entry| entry.path())
        .collect();

    assert_eq!(table_paths.len(), TABLES);

    let table = emoji_tables::build_table(&table_paths).unwrap();

    assert_eq!(table.len(), TABLE_ENTRIES);

    //let table = Arc::new(table);
    let table = Some(table);

    let emojis: HashSet<_> = fs::read_dir(SVG_PATH).expect("Couldn't read the svg directory")
        .filter(std::result::Result::is_ok)
        .map(std::result::Result::unwrap)
        .map(|entry| entry.path())
        .filter(|entry| entry.extension().is_some())
        .filter(|entry| entry.extension().unwrap() == "svg")
        .map(|path| Emoji::from_file(path, &table, false).unwrap())
        .collect();

    assert_eq!(emojis.len(), EMOJIS);

    let expected_emojis = build_emojis();

    assert!(emojis.is_subset(&expected_emojis));
    assert!(emojis.is_superset(&expected_emojis));

    let rainbow = Emoji {
        sequence: vec![0x1f3f3, 0x200d, 0x1f308],
        name: Some(String::from("rainbow flag")),
        kind: Some(vec![EmojiZwjSequence]),
        svg_path: None,
    };

    let rainbow_comp = emojis.get(&rainbow).unwrap();

    assert_eq!(rainbow_comp.name, rainbow.name);
    assert_eq!(rainbow_comp.kind, rainbow.kind);
}

fn build_emojis() -> HashSet<Emoji> {
    let rainbow = Emoji {
        sequence: vec![0x1f3f3, 0x200d, 0x1f308],
        name: Some(String::from("rainbow flag")),
        kind: Some(vec![EmojiZwjSequence]),
        svg_path: None,
    };

    let transgender = Emoji {
        sequence: vec![0x1f3f3, 0xfe0f, 0x200d, 0x26a7],
        name: None,
        kind: None,
        svg_path: None,
    };

    let otter = Emoji {
        sequence: vec![0x1f9a6],
        name: None,
        kind: None,
        svg_path: None,
    };

    let skunk = Emoji {
        sequence: vec![0x1f9a8],
        name: None,
        kind: None,
        svg_path: None,
    };

    let falafel = Emoji {
        sequence: vec![0x1f9c6],
        name: None,
        kind: None,
        svg_path: None,
    };

    let diving_mask = Emoji {
        sequence: vec![0x1f93f],
        name: Some(String::from("diving mask")),
        kind: None,
        svg_path: None,
    };

    let diya = Emoji {
        sequence: vec![0x1fa94],
        name: None,
        kind: None,
        svg_path: None,
    };

    HashSet::from_iter(vec![rainbow, transgender, otter, skunk, falafel, diving_mask, diya])
}