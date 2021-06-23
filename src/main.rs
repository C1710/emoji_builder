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
#[macro_use]
extern crate log;
#[macro_use]
extern crate include_dir;

use std::collections::HashMap;
use std::fs;
use std::iter::Iterator;
use std::path::PathBuf;

use clap::{App, ArgMatches, SubCommand, Arg};
use rayon::prelude::*;
use yaml_rust::Yaml;

use emoji_builder::builder::EmojiBuilder;
use emoji_builder::builders::blobmoji::Blobmoji;
use emoji_builder::emoji::Emoji;
use emoji_builder::emoji_tables::EmojiTable;
use std::fs::create_dir_all;
use std::io::{BufReader, Write};
use std::process::exit;

const LICENSES: include_dir::Dir = include_dir!("licenses");

fn main() {
    build::<Blobmoji>();
}

fn build<Builder: EmojiBuilder>() {
    let args = Builder::sub_command();
    let name = args.get_name().to_string();
    let log_modules = Builder::log_modules();
    let mut args = parse_args(vec![args], vec![log_modules]);


    let emojis = parse_emojis(&args);

    create_dir_all(&args.build_path).unwrap();
    if let Some(output_dir) = &args.output_path.parent() {
        create_dir_all(output_dir).unwrap();
    }

    // Now we are ready to start the actual build process
    let mut builder = Builder::new(
        args.build_path,
        args.builder_matches.remove(name.as_str()).unwrap_or(None),
    ).unwrap();

    let output = args.output_path;
    let prepared: HashMap<&Emoji, _> =
        emojis.par_iter()
        .map(|emoji| (emoji, builder.as_ref().prepare(emoji).map(|prepared| prepared.0)))
        .collect();
    let result = builder.as_mut().build(prepared, output);
    if let Err(err) = result {
        error!("An error occured while building the emoji set: {:?}", err);
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
            error!("Error in parsing the emoji tables: {}", err);
            None
        },
        None => None,
    };

    let table = if let Some(emoji_test) = args.emoji_test.as_ref() {
        let reader = std::fs::File::open(emoji_test).map(BufReader::new);
        if let Ok(reader) = reader {
            let mut table = table.unwrap_or_default();
            table.expand_descriptions_from_test_data(reader)
                .map(|_| table)
                .map_err(|err|
                    error!("Error in parsing emoji-test.txt: {}", err)
                )
                .ok()
        } else {
            table
        }
    } else {
        table
    };

    let table = if cfg!(feature = "online") && !args.offline {
        let mut table = table.unwrap_or_default();
        table.expand_all_online((13, 0)).unwrap_or_else(|e| warn!("Couldn't load online emoji tables: {:?}", e));
        Some(table)
    } else {
        table
    };

    if table.is_some() {
        info!("Using emoji table");
    }


    let images = &args.svg_path;

    let paths: Vec<_> = fs::read_dir(images)
        .unwrap_or_else(|_| panic!("Couldn't find image directory: {}", images.to_string_lossy())).collect();

    let flag_paths: Vec<_> = match &args.flag_path {
        None => vec![],
        Some(flags) => fs::read_dir(flags).unwrap().collect()
    };


    let emojis = paths
        .into_par_iter()
        .filter_map(|path| path.ok())
        .map(|path| path.path())
        .filter(|path| path.is_file())
        .map(|path| Emoji::from_path(path, table.as_ref(), false));

    let flags = flag_paths
        .into_par_iter()
        .filter_map(|path| path.ok())
        .map(|path| path.path())
        .filter(|path| path.is_file())
        .map(|path| Emoji::from_path(path, table.as_ref(), true));


    let emojis = emojis.chain(flags)
        .filter_map(|emoji| match emoji {
            Ok(emoji) => Some(emoji),
            Err(err) => {
                error!("{:?}", err);
                None
            }
        });


    // remove all multi character sequences if no_sequences is set
    if args.no_sequences {
        emojis.filter(|emoji| emoji.sequence.len() <= 1).collect()
    } else {
        let emojis: Vec<_> = emojis.collect();
        if let Some(table) = table {
            // Validate against the table
            let emoji_set = emojis.iter()
                .map(|emoji| emoji.sequence.clone())
                .collect();
            let (result, additional) = table.validate(&emoji_set, true);
            if let Err(missing) = result {
                missing.iter()
                    .for_each(|missing| warn!("Missing emoji: {} (Codepoint: {:X?}, Emoji: {})",
                                              missing,
                                              missing.sequence,
                                              missing.display_emoji()));
            }
            additional.iter()
                .for_each(|additional| info!("Additional emoji: {} (Codepoint: {:X?}, Emoji: )", additional, additional.sequence));
        }
        emojis
    }
}

struct BuilderArguments<'a> {
    svg_path: PathBuf,
    flag_path: Option<PathBuf>,
    tables_path: Option<PathBuf>,
    build_path: PathBuf,
    output_path: PathBuf,
    builder_matches: HashMap<String, Option<ArgMatches<'a>>>,
    no_sequences: bool,
    emoji_test: Option<PathBuf>,
    offline: bool
}

fn parse_args<'a>(builder_args: Vec<App<'a, 'a>>, builder_log_modules: Vec<Vec<String>>) -> BuilderArguments<'a> {
    lazy_static! {
        static ref YAML: Yaml = load_yaml!("cli.yaml").clone();
    }
    let names: Vec<String> = builder_args.iter().map(|args| String::from(args.get_name())).collect();
    let log_modules = builder_log_modules
        .into_iter()
        .flatten();
    // IntelliJ thinks this is an error, but it isn't.
    // As you can see above, &YAML really has the type &Yaml
    let app: App<'a, 'a> = App::from_yaml(&*YAML)
        .version(crate_version!())
        .subcommand(SubCommand::with_name("licenses")
            .arg(Arg::with_name("output_dir")
                .help("The directory to copy the license files to")
                .default_value("licenses")
                .value_name("DIR")
            )
            .arg(Arg::with_name("print")
                .help("Prints the files to stdout")
                .takes_value(false)
                .short("p")
                .long("print")
            )
            .help("Extracts the license information for the used dependencies to the specified directory"))
        .subcommands(builder_args);
    let matches: ArgMatches = app
        .get_matches();

    stderrlog::new()
        .module(module_path!())
        .modules(log_modules)
        .verbosity(matches.occurrences_of("verbose") as usize)
        .init().unwrap();

    if let Some(matches) = matches.subcommand_matches("licenses") {
        let print = matches.is_present("print");
        if !print {
            let output_dir = matches.value_of("output_dir").unwrap();
            let output_dir = PathBuf::from(output_dir);
            create_dir_all(&output_dir).unwrap();

            recurse_included_dir(&LICENSES).iter()
                .map(|file| ((&output_dir).join(file.path()), file.contents()))
                .for_each(|(path, content)| {
                    if let Some(parent) = path.parent() {
                        create_dir_all(parent).unwrap_or_else(|err| error!("{:?}", err));
                    }
                    if !path.exists() {
                        match std::fs::File::create(path) {
                            Ok(mut file) => file.write_all(content).unwrap_or_else(|err| error!("{:?}", err)),
                            Err(err) => error!("{:?}", err)
                        }
                    } else {
                        info!("Not overwriting {:#?}", path);
                    }
                }
                );
        } else {
            recurse_included_dir(&LICENSES).iter()
                .map(|file| (file.path(), file.contents_utf8()))
                .filter_map(|(path, content)| if let Some(content) = content {
                    Some((path, content))
                } else {
                    warn!("Empty file: {:?}", path);
                    None
                })
                .for_each(|(path, content)| {
                    println!("{:?}:", path);
                    println!("  {}", content.replace('\n', "\n  "));
                })
        }

        exit(0);
    }


    let images: PathBuf = matches.value_of("images").unwrap().into();
    let flags = matches.value_of("flags");
    let tables = matches.value_of("tables");
    let build: PathBuf = matches.value_of("build").unwrap().into();

    let output = matches.value_of("output").unwrap();
    let output_dir = matches.value_of("output_dir").unwrap();
    let output_path = PathBuf::from(output_dir).join(PathBuf::from(output));

    let no_sequences = matches.is_present("no_sequences");

    let flags = flags.map(PathBuf::from);

    let emoji_test = matches.value_of("emoji_test").map(PathBuf::from);

    let offline = matches.is_present("offline");

    let tables = tables.map(PathBuf::from);

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
        builder_matches,
        no_sequences,
        emoji_test,
        offline
    }
}


fn recurse_included_dir<'a>(dir: &'a include_dir::Dir) -> Vec<&'a include_dir::File<'a>> {
    dir.files().iter()
        .chain(dir.dirs().iter()
            .map(|dir| recurse_included_dir(dir))
            .flatten()
        )
        .collect()
}