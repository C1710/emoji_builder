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

#[macro_use]
extern crate clap;

use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use clap::{App, ArgMatches};
use rayon::prelude::*;

use emoji_builder::emoji::Emoji;
use emoji_builder::emoji_tables;
use emoji_builder::emoji_tables::EmojiTable;

fn main() {
    let args = parse_args();

    let table_paths = args.tables_path;

    let table = match table_paths {
        Some(table_paths) => {
            let table_paths: Vec<_> = table_paths
                .read_dir()
                .unwrap()
                .filter(|entry| entry.is_ok())
                .map(|entry| entry.unwrap())
                .map(|entry| entry.path())
                .collect();
            Some(EmojiTable::from_files(&table_paths))
        }
        None => None,
    };
    let table = match table {
        Some(Ok(table)) => Some(table),
        Some(Err(_)) => None,
        None => None,
    };

    if table.is_some() {
        println!("Using unicode table");
    }

    let images = args.svg_path;

    let paths: Vec<_> = fs::read_dir(images).unwrap().collect();

    let emojis: Vec<Emoji> = paths
        .par_iter()
        .filter(|path| path.is_ok())
        .map(|path: &Result<DirEntry, _>| match path {
            Ok(path) => path,
            Err(_) => unreachable!(),
        })
        .map(std::fs::DirEntry::path)
        .filter(|path| path.is_file())
        .map(|path| Emoji::from_file(path, &table, false))
        .filter(std::result::Result::is_ok)
        .map(std::result::Result::unwrap)
        .collect();

    if args.verbose {
        for emoji in emojis {
            println!("{:?}", emoji);
        }
    }
}

struct BuilderArguments {
    svg_path: PathBuf,
    flag_path: Option<PathBuf>,
    tables_path: Option<PathBuf>,
    build_path: PathBuf,
    output_path: PathBuf,
    verbose: bool,
}

fn parse_args() -> BuilderArguments {
    let yaml = load_yaml!("cli.yaml");
    let matches: ArgMatches = App::from_yaml(yaml)
        .version(crate_version!()).get_matches();

    let verbose = matches.is_present("verbose");
    let images: PathBuf = matches.value_of("images").unwrap().into();
    let flags = matches.value_of("flags");
    let tables = matches.value_of("tables");
    let build: PathBuf = matches.value_of("build").unwrap().into();

    let output = matches.value_of("output").unwrap();
    let output_dir = matches.value_of("output_dir").unwrap();
    let output_path = PathBuf::from(output_dir).join(PathBuf::from(output));

    let flags = match flags {
        Some(flags) => Some(PathBuf::from(flags)),
        None => None,
    };

    let tables = match tables {
        Some(tables) => Some(PathBuf::from(tables)),
        None => None,
    };

    BuilderArguments {
            svg_path: images,
            flag_path: flags,
            tables_path: tables,
            build_path: build,
            output_path,
            verbose,
    }
}
