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

extern crate core;
extern crate emoji_builder;
extern crate itertools;
extern crate regex;
extern crate threadpool;

use core::borrow::Borrow;
use std::env::args;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;

use itertools::enumerate;
use threadpool::ThreadPool;

use emoji_builder::emoji::Emoji;
use emoji_builder::unicode_tables;

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
        Some(table_paths) => Some(unicode_tables::build_table(&table_paths)),
        None => None
    };
    let table = match table {
        Some(Ok(table)) => Some(Arc::new(table)),
        Some(Err(_)) => None,
        None => None
    };


    if table.is_some() {
        println!("Using unicode table");
    }
    let workers = 8;
    let pool = ThreadPool::new(workers);

    let (tx, rx) = channel();
    let paths = fs::read_dir(path).unwrap();
    let mut size = 0;

    for path in paths {
        size += 1;
        let tx = tx.clone();
        let table = table.clone();
        pool.execute(move || {
            if let Ok(path) = path {
                let path = path.path();
                let path = path.as_path();
                if let Some(ext) = path.extension() {
                    if ext == "svg" {
                        let emoji = Emoji::from_file(path, table, false);
                        tx.send(emoji).unwrap_or_default();
                        return;
                    }
                }
            }
            tx.send(None);
        })
    }
    for result in rx {
        if let Some(emoji) = result {
            println!("{:?}", emoji);
        };
        size -= 1;
        if size == 0 {
            break;
        }
    }
}


