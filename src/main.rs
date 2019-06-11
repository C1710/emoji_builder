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

use std::env::args;
use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use rayon::prelude::*;

use emoji_builder::emoji::Emoji;
use emoji_builder::emoji_tables;

fn main() {
    let mut args = args();
    args.next();
    let path = args.next().expect("You need to specify a path");
    let table_paths: Vec<_> = args.map(PathBuf::from).collect();
    let table_paths = match table_paths.len() {
        0 => None,
        _ => Some(table_paths)
    };

    let table = match table_paths {
        Some(table_paths) => Some(emoji_tables::build_table(&table_paths)),
        None => None
    };
    let table = match table {
        Some(Ok(table)) => Some(table),
        Some(Err(_)) => None,
        None => None
    };


    if table.is_some() {
        println!("Using unicode table");
    }

    //let (tx, rx) = channel();
    let paths = fs::read_dir(path).unwrap();

    let paths: Vec<_> = paths.collect();

    let _emojis: Vec<_> = paths.par_iter()
        .filter(|path| path.is_ok())
        .map(|path: &Result<DirEntry, _>| match path {
            Ok(path) => path,
            Err(_) => unreachable!()
        })
        .map(std::fs::DirEntry::path)
        .filter(|path| path.extension().is_some() && path.extension().unwrap() == "svg")
        .map(|path| Emoji::from_file(path, &table, false))
        .collect();
}


