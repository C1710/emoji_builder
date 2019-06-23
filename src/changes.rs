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
//! A simple hash-based implementation to track changes of the emoji's SVG files.
//!
//! It is mostly intended for the different `EmojiBuilder`s that shouldn't do heavy rendering tasks
//! twice for the exact same file.

use std::{fs, io};
use std::collections::HashMap;
use std::io::Write;
use std::ops::Index;
use std::path::{Path, PathBuf};

use csv::Error;
use digest::generic_array::GenericArray;
use hex::FromHexError;
use sha2::{Digest, Sha256};

use crate::changes::CheckError::{Io, NoFileSpecified};
use crate::emoji::Emoji;

/// A simple struct that maps code sequences to file hashes
pub struct FileHashes(HashMap<Vec<u32>, Vec<u8>>);

#[derive(Debug)]
pub enum CheckError {
    /// An error that happened in the IO part
    Io(std::io::Error),
    /// This error indicates that the given `Emoji` doesn't carry a path for its SVG file
    NoFileSpecified,
}

impl FileHashes {
    /// Parses an CSV file to a `FileHashes` table
    /// It assumes that there is **no** header.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<FileHashes, Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(path)?;
        FileHashes::from_csv_reader(&mut reader)
    }

    /// Parses an CSV file (from whichever source) to a `FileHashes` table.
    /// It assumes that there is **no** header.
    pub fn from_reader<R: io::Read>(reader: R) -> Result<FileHashes, Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);
        FileHashes::from_csv_reader(&mut reader)
    }


    fn from_csv_reader<R: io::Read>(reader: &mut csv::Reader<R>) -> Result<FileHashes, Error> {
        let records = reader.records();
        let entries: Vec<(Vec<u32>, Vec<u8>)> = records
            .filter(std::result::Result::is_ok)
            .map(std::result::Result::unwrap)
            .filter(|record| record.len() >= 2)
            .map(|record| (parse_hex(&record[0]), hex::decode(&record[1])))
            .filter(|(_, hash)| hash.is_ok())
            .map(|(sequence, hash)| (sequence, hash.unwrap()))
            .collect();
        let mut table = HashMap::with_capacity(entries.len());
        table.extend(entries);
        Ok(FileHashes(table))
    }

    /// Checks whether the hash of the file is still the same as the one in the table.
    pub fn check(&self, emoji: &Emoji) -> Result<bool, CheckError> {
        if let Some(path) = &emoji.svg_path {
            let mut hasher = Sha256::new();
            let file = fs::File::open(path);
            let hash = self.0.get(&emoji.sequence);

            if let Some(hash) = hash {
                match file {
                    Ok(mut file) => match io::copy(&mut file, &mut hasher) {
                        Ok(_) => {
                            let result = hasher.result();
                            let result = result.as_slice();
                            let hash = hash.as_slice();
                            Ok(hash == result)
                        }
                        Err(error) => Err(Io(error)),
                    },
                    Err(error) => Err(Io(error)),
                }
            } else {
                // If there is no entry, the hash can be assumed as different
                Ok(false)
            }
        } else {
            Err(NoFileSpecified)
        }
    }

    /// Replaces (or inserts) the hash for a given `Emoji`.
    pub fn update(
        &mut self,
        emoji: &Emoji,
        hash: &[u8],
    ) -> Option<Vec<u8>> {
        self.0.insert(emoji.sequence.clone(), Vec::from(hash))
    }

    /// Computes the hash value of a single file.
    /// This is mostly useful for parallel implementations
    pub fn hash(emoji: &Emoji) -> Result<GenericArray<u8, <Sha256 as Digest>::OutputSize>, CheckError> {
        if let Some(path) = &emoji.svg_path {
            let mut hasher = Sha256::new();
            let file = fs::File::open(path);
            match file {
                Ok(mut file) => match io::copy(&mut file, &mut hasher) {
                    Ok(_) => Ok(hasher.result()),
                    Err(error) => Err(Io(error))
                },
                Err(error) => Err(Io(error))
            }
        } else {
            Err(NoFileSpecified)
        }
    }

    /// Saves the table to a CSV file.
    /// **Warning**: Any existing file with that name will be overwritten.
    pub fn write_to_path(&self, path: PathBuf) -> Result<(), Error> {
        let mut writer = csv::Writer::from_path(path)?;
        self.write_to_csv_writer(&mut writer)
    }


    /// Saves the table to something that can be written to.
    /// **Warning**: Any existing file with that name will be overwritten.
    pub fn write_to_writer<W: Write>(&self, writer: W) -> Result<(), Error> {
        let mut writer = csv::Writer::from_writer(writer);
        self.write_to_csv_writer(&mut writer)
    }

    fn write_to_csv_writer<W: Write>(&self, writer: &mut csv::Writer<W>) -> Result<(), Error> {
        for entry in &self.0 {
            let sequence = entry.0.iter();
            let sequence: Vec<String> = sequence
                .map(|codepoint| format!("{:x}", codepoint))
                .collect();
            let sequence = sequence.join(" ");
            let hash = hex::encode(entry.1);
            writer.write_record(vec![sequence, hash])?;
        }
        writer.flush()?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains<E: AsRef<[u32]>>(&self, emoji: E) -> bool {
        self.0.contains_key(emoji.as_ref())
    }

    pub fn new() -> FileHashes {
        Self::default()
    }
}

impl Default for FileHashes {
    fn default() -> Self {
        FileHashes(HashMap::new())
    }
}

impl<I: AsRef<[u32]>> Index<I> for FileHashes {
    type Output = Vec<u8>;

    fn index(&self, index: I) -> &Self::Output {
        &self.0[index.as_ref()]
    }
}

impl Into<HashMap<Vec<u32>, Vec<u8>>> for FileHashes {
    fn into(self) -> HashMap<Vec<u32>, Vec<u8>> {
        self.0
    }
}

impl AsRef<HashMap<Vec<u32>, Vec<u8>>> for FileHashes {
    fn as_ref(&self) -> &HashMap<Vec<u32>, Vec<u8>> {
        &self.0
    }
}

fn parse_hex(sequence: &str) -> Vec<u32> {
    let sequence = sequence.trim();
    let sequence = sequence.split(' ');
    sequence
        .map(|code| u32::from_str_radix(code, 16))
        .filter(|code| !code.is_err())
        .map(std::result::Result::unwrap)
        .collect()
}
