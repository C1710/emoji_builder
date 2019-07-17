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

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use crate::changes::FileHashes;
use crate::emoji::Emoji;

const SVG_FILE: &str = "test_files/svg/emoji_u1f9a6.svg";
const HASH_FILE: &str = "test_files/hash.csv";

#[test]
fn test_hashing() {
    let temp_file = tempfile::tempfile().unwrap();
    let mut empty_hashes = FileHashes::from_reader(temp_file).unwrap();

    // Check that it is *really* empty
    assert!(empty_hashes.is_empty());
    let emoji = Emoji::from_path(PathBuf::from(SVG_FILE), &None, false).unwrap();
    assert!(!empty_hashes.check(&emoji).unwrap());

    let hash = FileHashes::hash(&emoji).unwrap();
    empty_hashes.update(&emoji, &hash).unwrap_or_default();
    let filled_hashes = empty_hashes;

    // Now there's something in it. Hopefully it's the correct emoji
    assert!(filled_hashes.check(&emoji).unwrap());
    let temp_file = tempfile::tempfile().unwrap();
    filled_hashes.write_to_writer(&temp_file).unwrap();

    let correct_path = PathBuf::from(HASH_FILE);
    assert!(correct_path.exists());

    // Now check that the file has been written according to plan
    let correct_file = File::open(&correct_path).unwrap();

    for (expected, actual) in correct_file.bytes().zip(temp_file.bytes()) {
        assert_eq!(actual.unwrap(), expected.unwrap());
    }

    // Test the loading mechanism
    let correct_hashes = FileHashes::from_path(&correct_path).unwrap();
    assert_eq!(correct_hashes.len(), 1);
    assert!(correct_hashes.check(&emoji).unwrap());
}