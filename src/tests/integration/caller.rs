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


use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builder::EmojiBuilder;
use crate::changes::FileHashes;
use crate::emoji::Emoji;
use crate::emoji_tables::EmojiTable;
use crate::tests::integration::builder::DummyBuilder;

pub struct TestResult<T: EmojiBuilder> {
    build_path: PathBuf,
    output_path: PathBuf,
    result: Result<(), T::Err>,
    _emojis: Vec<Emoji>,
    _table: EmojiTable,
}

fn prepare<'a, T: EmojiBuilder>(emojis: &'a [Emoji], builder: &T) -> HashMap<&'a Emoji, Result<T::PreparedEmoji, T::Err>> {
    emojis.iter()
        .map(|emoji| (emoji, builder.prepare(emoji)))
        .collect()
}

fn build<T: EmojiBuilder>(emojis: HashMap<&Emoji, Result<T::PreparedEmoji, T::Err>>, builder: &mut T, output_file: PathBuf) -> Result<(), T::Err> {
    builder.build(emojis, output_file)
}

fn create<T: EmojiBuilder>(build_path: PathBuf) -> T {
    *T::new(build_path, None).unwrap()
}

fn create_temps() -> (PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap().into_path();
    let build_path = dir.join("build");
    std::fs::create_dir(&build_path).unwrap();
    // The output directory/file will NOT be created here
    let output_path = dir.join("output");
    (build_path, output_path)
}

pub fn run<T: EmojiBuilder>(emojis: &[Emoji]) -> (PathBuf, PathBuf, Result<(), T::Err>) {
    let (build_path, output_path) = create_temps();
    let mut builder: T = create(build_path.clone());
    let prepared = prepare(emojis, &builder);
    let result = build(prepared, &mut builder, output_path.clone());
    (build_path, output_path, result)
}


fn parse_emojis(emojis: &Path, flags: &Path, table: &Option<EmojiTable>) -> Vec<Emoji> {
    let emojis = emojis.read_dir().unwrap();
    let emojis = emojis
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .map(|entry| Emoji::from_path(entry.path(), table, false).unwrap());
    let flags = flags.read_dir().unwrap();
    let flags = flags
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .map(|entry| Emoji::from_path(entry.path(), table, true).unwrap());
    emojis.chain(flags).collect()
}

fn parse_tables(directory: &Path) -> Option<EmojiTable> {
    let directory = directory.read_dir().unwrap();
    let tables: Vec<_> = directory
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.path())
        .collect();
    EmojiTable::from_files(&tables).ok()
}

const TEST_EMOJIS: &str = "test_files/svg";
const TEST_FLAGS: &str = "test_files/flags";
const TEST_TABLES: &str = "test_files/tables";
const TEST_HASHES: &str = "test_files/hashes.csv";

pub fn run_with_test_files<T: EmojiBuilder>() -> TestResult<T> {
    let table = parse_tables(&PathBuf::from(TEST_TABLES));
    let emojis = parse_emojis(
        &PathBuf::from(TEST_EMOJIS),
        &PathBuf::from(TEST_FLAGS),
        &table,
    );
    let (build_path, output_path, result) = run::<T>(&emojis);
    let table = table.unwrap();
    TestResult {
        build_path,
        output_path,
        result,
        _emojis: emojis,
        _table: table,
    }
}

#[test]
fn test_dummy() {
    let result = run_with_test_files::<DummyBuilder>();
    // First of all, have there been any errors?
    assert!(result.result.is_ok(),
            "An error has occured:\n\t{:?}", result.result.unwrap_err());
    // Okay, the build directory should be empty
    assert!(std::fs::read_dir(result.build_path).unwrap().next().is_none());
    // And there should be an output file...
    assert!(result.output_path.exists());
    // And it should contain the correct hashes
    check_hashes(&result.output_path, &PathBuf::from(TEST_HASHES));
}

fn check_hashes(actual: &Path, expected: &Path) {
    let actual = FileHashes::from_path(actual).unwrap();
    let expected = FileHashes::from_path(expected).unwrap();
    let actual: HashMap<_, _> = actual.into();
    let expected: HashMap<_, _> = expected.into();
    assert_eq!(actual, expected);
}