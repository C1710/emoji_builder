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


use crate::tables::regexes::{EmojiFileEntry, match_line};
use crate::loadables::loadable::LoadablePrototype;
use crate::loadables::sources::LoadableSource;
use crate::loadables::prototype_error::PrototypeLoadingError;
use crate::loadables::NoError;
use std::io::{BufReader, BufRead};

#[derive(Debug)]
pub struct EmojiTablePrototype<'a> {
    pub entries: Vec<(String, EmojiFileEntry<'a>)>
}

impl<'a, Source> LoadablePrototype<Source> for EmojiTablePrototype<'a>
    where Source: LoadableSource {
    type Error = NoError;

    fn load_prototype(source: &Source) -> Result<Self, PrototypeLoadingError<Self, Source>> where Source: LoadableSource, Self: Sized {
        let reader = source.request_root_file().map_err(PrototypeLoadingError::Source)?;
        let reader = BufReader::new(reader);
        let entries = reader.lines().flatten()
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
            .map(|line| line.trim().to_lowercase())
            // This is safe as we store the line alongside the match result
            .filter_map(|line| match_line(unsafe { (&line as *const String).as_ref() }.unwrap() as &'a str)
                .or_else(|| {
                    warn!("Malformed line: {}", &line);
                    None
                })
                .zip(Some(line))
            )
            .map(|(line, entry)| (entry, line))
            .collect();
        Ok(Self{ entries })
    }
}