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
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::fs;
use std::iter::Iterator;
use std::path::PathBuf;

use clap::{App, ArgMatches};
use rayon::prelude::*;
use yaml_rust::Yaml;

use emoji_builder::builder::EmojiBuilder;
use emoji_builder::builders::blobmoji::Blobmoji;
use emoji_builder::emoji::Emoji;
use emoji_builder::emoji_tables::EmojiTable;

fn main() {
    build::<Blobmoji>();
}

fn build<Builder: EmojiBuilder>() {
    let args = Builder::sub_command();
    let name = args.get_name().to_string();
    let mut args = parse_args(vec![args]);

    if args.verbose {
        println!("Verbose mode enabled.");
    }

    let emojis = parse_emojis(&args);

    // Now we are ready to start the actual build process
    let mut builder = Builder::new(
        args.build_path,
        args.verbose,
        args.builder_matches.remove(name.as_str()).unwrap_or(None),
    ).unwrap();

    let output = args.output_path;
    let prepared: HashMap<&Emoji, _> = emojis.par_iter()
        .map(|emoji| (emoji, builder.as_ref().prepare(emoji)))
        .collect();
    let result = builder.as_mut().build(prepared, output);
    if let Err(err) = result {
        eprintln!("An error occured while building the emoji set: {:?}", err);
    }
}

fn parse_emojis(args: &BuilderArguments) -> Vec<Emoji> {
    let table_paths = &args.tables_path;

    let table = match table_paths {
        Some(table_paths) => {
            let table_paths: Vec<_> = table_paths
                .read_dir()
                .unwrap()
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect();
            Some(EmojiTable::from_files(&table_paths))
        }
        None => None,
    };
    let table = match table {
        Some(Ok(table)) => Some(table),
        Some(Err(err)) => {
            eprintln!("Error in parsing the emoji tables: {}", err);
            None
        },
        None => None,
    };

    if table.is_some() && args.verbose {
        println!("Using emoji table");
    }

    let images = &args.svg_path;

    let paths: Vec<_> = fs::read_dir(images)
        .expect(&format!("Couldn't find image directory: {}", images.to_string_lossy())).collect();

    let flag_paths: Vec<_> = match &args.flag_path {
        None => vec![],
        Some(flags) => fs::read_dir(flags).unwrap().collect()
    };


    let emojis = paths
        .into_par_iter()
        .filter_map(|path| path.ok())
        .map(|path| path.path())
        .filter(|path| path.is_file())
        .map(|path| Emoji::from_path(path, &table, false));

    let flags = flag_paths
        .into_par_iter()
        .filter_map(|path| path.ok())
        .map(|path| path.path())
        .filter(|path| path.is_file())
        .map(|path| Emoji::from_path(path, &table, true));


    emojis.chain(flags)
        .filter_map(std::result::Result::ok)
        .collect()
}

struct BuilderArguments<'a> {
    svg_path: PathBuf,
    flag_path: Option<PathBuf>,
    tables_path: Option<PathBuf>,
    build_path: PathBuf,
    output_path: PathBuf,
    verbose: bool,
    builder_matches: HashMap<String, Option<ArgMatches<'a>>>,
}

fn parse_args<'a>(builder_args: Vec<App<'a, 'a>>) -> BuilderArguments<'a> {
    lazy_static! {
        static ref YAML: Yaml = load_yaml!("cli.yaml").clone();
    }
    let names: Vec<String> = builder_args.iter().map(|args| String::from(args.get_name())).collect();
    let app: App<'a, 'a> = App::from_yaml(&YAML)
        .version(crate_version!())
        .subcommands(builder_args);
    let matches: ArgMatches = app
        .get_matches();

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

    let subcommands: Vec<_> = names.iter()
        .map(|name| matches.subcommand_matches(name).cloned())
        .collect();

    // We want to move the name here, but then it would not be possible to use it in
    // subcommand_matches anymore, so this is done earlier
    let builder_matches: HashMap<_, _> = names.into_iter()
        .zip(subcommands)
        .collect();

    BuilderArguments {
        svg_path: images,
        flag_path: flags,
        tables_path: tables,
        build_path: build,
        output_path,
        verbose,
        builder_matches,
    }
}
