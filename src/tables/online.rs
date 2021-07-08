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


use std::collections::HashMap;
use std::sync::RwLock;

use crate::tables::emoji_tables::EmojiTable;
use crate::tables::errors::ExpansionError;

pub const EMOJI_DATA: &str = "emoji-data.txt";
pub const EMOJI_SEQUENCES: &str = "emoji-sequences.txt";
pub const EMOJI_ZWJ_SEQUENCES: &str = "emoji-zwj-sequences.txt";
pub const EMOJI_VARIATION_SEQUENCES: &str = "emoji-variation-sequences.txt";
pub const EMOJI_TEST: &str = "emoji-test.txt";
pub const DATA_FILES: [&str; 3] = [
    EMOJI_DATA,
    EMOJI_SEQUENCES,
    EMOJI_ZWJ_SEQUENCES
];

/// This function is <del>equivalent to</del> creating an `EmojiTable` and directly calling `expand_all_online` on it.`
pub fn load_online_table(version: (u32, u32)) -> Result<EmojiTable, ExpansionError> {
    let mut table = EmojiTable::new();
    match table.expand_all_online(version) {
        Ok(_) => Ok(table),
        Err(error) => Err(error)
    }
}

/// A simple helper function to build the URLs for the different files.
#[inline]
fn build_url(version: (u32, u32), file: &'static str) -> String {
    if version.0 >= 13 && [EMOJI_DATA, EMOJI_VARIATION_SEQUENCES].contains(&file) {
        format!("https://unicode.org/Public/{}.0.0/ucd/emoji/{}", version.0, file)
    } else {
        format!("https://unicode.org/Public/emoji/{}.{}/{}", version.0, version.1, file)
    }
}

pub fn get_data_file_online(client: &reqwest::blocking::Client, version: (u32, u32), file: &'static str) -> Result<std::io::Cursor<bytes::Bytes>, reqwest::Error> {
    // Check if we can return the file from the cache already
    let cache = (&*TABLE_CACHE as &TableCache).read();
    if let Ok(cache) = cache {
        if let Some(cached_files) = cache.get(&version) {
            if let Some(cached) = cached_files.get(file) {
                return Ok(std::io::Cursor::new(cached.clone()));
            }
        }
    }
    let request = client.get(&build_url(version, file)).send();
    let bytes = request?.bytes()?;

    // Insert data into the cache
    let cache = (&*TABLE_CACHE as &TableCache).write();
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

type TableCache = RwLock<HashMap<(u32, u32), HashMap<String, bytes::Bytes>>>;

// 14 Unicode/emoji main versions * 2 minor versions ~= 32 versions we could possibly cache
lazy_static! {
    static ref TABLE_CACHE: TableCache =
        RwLock::new(HashMap::with_capacity(32));
}

