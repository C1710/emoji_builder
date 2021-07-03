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
use std::io::{Read, Write};
// For some reason Cursor is marked as an unused import. However that's wrong as it's used in test_nocr().
#[cfg(test)]
use std::io::Cursor;
use std::ops::Index;
use std::path::PathBuf;

use csv::Error;
use digest::generic_array::GenericArray;
use sha2::{Digest, Sha256};

use crate::changes::CheckError::{Io, NoFileSpecified};
use crate::emojis::emoji::Emoji;
use crate::changes;
use crate::loadables::loadable::LoadablePrototype;
use crate::loadables::sources::LoadableSource;
use crate::loadables::prototype_error::PrototypeLoadingError;
use crate::loadables::sources::fs_source::FsSource;

/// A simple struct that maps code sequences to file hashes
#[derive(Debug)]
pub struct FileHashes(HashMap<Vec<u32>, Vec<u8>>);

#[derive(Debug)]
/// An error that can occur with change checking
pub enum CheckError {
    /// An error that happened in the IO part
    Io(std::io::Error),
    /// This error indicates that the given `Emoji` doesn't carry a path for its SVG file
    NoFileSpecified,
}

impl FileHashes {
    fn from_csv_reader<R: io::Read>(reader: &mut csv::Reader<R>) -> changes::FileHashes {
        let records = reader.records();
        let entries: Vec<(Vec<u32>, Vec<u8>)> = records
            .filter_map(std::result::Result::ok)
            .filter(|record| record.len() >= 2)
            .map(|record| (parse_hex(&record[0]), hex::decode(&record[1])))
            .filter_map(|(sequence, hash)| Some(sequence).zip(hash.ok()))
            .collect();
        let mut table = HashMap::with_capacity(entries.len());
        table.extend(entries);
        FileHashes(table)
    }

    #[deprecated]
    pub fn from_reader<R: std::io::Read>(reader: R) -> Self {
        Self::from_csv_reader(&mut csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader)
        )
    }

    /// Checks whether the hash of the file is still the same as the one in the table.
    pub fn check(&self, emoji: &Emoji) -> Result<bool, CheckError> {
        if let Some(path) = &emoji.svg_path {
            let mut hasher = Sha256::new();
            let file = fs::File::open(path);
            let hash = self.0.get(&emoji.sequence);

            if let Some(hash) = hash {
                match file {
                    // To get consistent results, CRs will be ignored
                    // (in order to get consistent line endings)
                    // TODO: Maybe change this behavior in the future as it's messy and actually
                    //       Only relevant in the tests.
                    //       When used in production, line endings can actually be considered to
                    //       stay the same, and even if not, re-rendering only impacts the
                    //       performance, but not the correctness of the result.
                    Ok(file) => match io::copy(&mut NoCrRead(file), &mut hasher) {
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
    /// This is mostly useful for parallel implementations.
    pub fn hash(emoji: &Emoji) -> Result<GenericArray<u8, <Sha256 as Digest>::OutputSize>, CheckError> {
        if let Some(path) = &emoji.svg_path {
            let mut hasher = Sha256::new();
            let file = fs::File::open(path);
            match file {
                // To get consistent results, CRs will be ignored
                // (in order to get consistent line endings)
                // TODO: Maybe change this behavior in the future
                Ok(file) => match io::copy(&mut NoCrRead(file), &mut hasher) {
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

    /// If a changelist is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The length of a changelist
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Checks whether an emoji occurs in a changelist
    pub fn contains<E: AsRef<[u32]>>(&self, emoji: E) -> bool {
        self.0.contains_key(emoji.as_ref())
    }

    #[deprecated]
    pub fn from_path(path: PathBuf) -> Result<Self, std::io::Error> {
        let source = FsSource::new(path.clone())
            .unwrap_or_else(|| FsSource::new_with_dir(PathBuf::new(), Some(path)));
        Self::load_prototype(&source)
            .map_err(|error|
                match error {
                    PrototypeLoadingError::Source(error) => error,
                    PrototypeLoadingError::Prototype(error) => error
                }
            )
    }

    /// Create a new, empty changelist
    pub fn new() -> FileHashes {
        Self::default()
    }
}

impl<S> LoadablePrototype<S> for FileHashes
    where S: LoadableSource {
    type Error = std::io::Error;

    fn load_prototype(source: &S) -> Result<Self, PrototypeLoadingError<Self, S>> {
        let reader = source.request_root_file().map_err(PrototypeLoadingError::Source)?;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);
        Ok(FileHashes::from_csv_reader(&mut reader))
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

impl From<FileHashes> for HashMap<Vec<u32>, Vec<u8>> {
    fn from(hashes: FileHashes) -> Self {
        hashes.0
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

/// A wrapper that discards all occurences of CR-characters (ASCII 0xD)
struct NoCrRead<R: Read>(R);

impl<R: Read> Read for NoCrRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut read_bytes = self.0.read(buf)?;
        let crs = bytecount::count(&buf[..read_bytes], 0xdu8);
        // Pretend we didn't read the CR-bytes
        read_bytes -= crs;
        // Found a C
        if crs > 0 {
            let ptr = buf.as_mut_ptr();
            for (i, d) in buf.iter().filter(|d| **d != 0xdu8).enumerate() {
                // This unsafe block allows to write to the given position although the array is already borrowed
                // However, it's okay to do that, as of these reasons:
                // 1. It's already borrowed as mutable, so no one else may write to it
                // 2. we've already read the affected value
                // 3. The add(i) is okay as we will at most reach the end of the slice with it
                unsafe {
                    *(ptr.add(i)) = *d;
                }
            }
            // Fill the rest of the slice with fresh data
            let len = buf.len();
            read_bytes += self.read(&mut buf[len - crs..])?;
        }
        buf[read_bytes..].iter_mut().for_each(|d| *d = 0x0u8);
        Ok(read_bytes)
    }
}


#[test]
fn test_nocr() {
    // First create some test-data
    let cursor = Cursor::new(vec![0x41, 0xd, 0xa, 0x42]);
    // Empty buffer
    let mut buf = [0x0u8; 4];
    // Test without removing the CRs
    let read_bytes = cursor.clone().read(&mut buf).unwrap();
    assert_eq!(read_bytes, 4);
    assert_eq!(buf, [0x41, 0xd, 0xa, 0x42]);

    // Test with removing the CRs
    let mut reader = NoCrRead(cursor);
    let read_bytes = reader.read(&mut buf).unwrap();
    assert_eq!(read_bytes, 3);
    assert_eq!(buf, [0x41u8, 0xau8, 0x42u8, 0x0u8]);

    // Test with removing the CRs, but without any CR present
    // First create some test-data
    let mut cursor = Cursor::new(vec![0x41, 0xa, 0x42]);
    // Empty buffer
    let mut buf = [0x0u8; 4];
    let read_bytes = cursor.read(&mut buf).unwrap();
    assert_eq!(read_bytes, 3);
    assert_eq!(buf, [0x41, 0xa, 0x42, 0x0]);
}