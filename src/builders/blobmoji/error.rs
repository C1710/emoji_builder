/*
 * Copyright 2020 Constantin A. <emoji.builder@c1710.de>.
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



use std::fmt::Debug;

/// The error type used in the Blobmoji-builder
#[derive(Debug)]
pub enum BlobmojiError {
    /// This is actually not an error but just a hint that the prepared emoji is not supposed to be
    /// used. It might be still rendered, but its hash will not be saved so it gets overwritten
    /// later on.
    EmojiInvalidated,
    // TODO: Get rid of this error
    UnknownError,
    IoError(std::io::Error),
    IoErrors(Vec<std::io::Error>),
    CsvError(csv::Error),
    // Unfortunately, PyErr requires additional stuff to be actually helpful
    PythonError(String)
}

impl From<()> for BlobmojiError {
    fn from(_: ()) -> Self {
        BlobmojiError::UnknownError
    }
}

impl From<std::io::Error> for BlobmojiError {
    fn from(error: std::io::Error) -> Self {
        BlobmojiError::IoError(error)
    }
}

impl From<csv::Error> for BlobmojiError {
    fn from(error: csv::Error) -> Self {
        BlobmojiError::CsvError(error)
    }
}
