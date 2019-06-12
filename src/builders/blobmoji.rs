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
//! The _Blobmoji_ build routine is capable of creating CBDT/CBLC emoji fonts as
//! well as ones that can be used in the [EmojiCompat-Library][emojiCompat] (e.g. with a
//! [file-based][filemojicompat] implementation) on Android.
//!
//! The exact emoji set that this is written for is [Blobmoji][blob], a fork of
//! [Noto Emoji][noto] with a continued support of the Blob emojis.
//!
//! [emojiCompat]: https://developer.android.com/guide/topics/ui/look-and-feel/emoji-compat
//! [blob]: https://github.com/c1710/blobmoji
//! [noto]: https://github.com/googlefonts/noto-emoji
//! [filemojicompat]: https://github.com/c1710/filemojicompat

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{App, ArgMatches, SubCommand};
use oxipng::{optimize_from_memory, PngResult};
use oxipng::internal_tests::Headers::Safe;
use resvg::backend_raqote;
use resvg::FitTo;
use resvg::prelude::*;

use crate::builder::EmojiBuilder;
use crate::changes::FileHashes;
use crate::emoji::Emoji;

#[allow(dead_code)]
struct Blobmoji {
    build_path: PathBuf,
    name: Option<String>,
    images: HashMap<Vec<u32>, Option<Vec<u8>>>,
    hashes: FileHashes,
    verbose: bool,
}

const HASHES: &str = "hashes.csv";

impl EmojiBuilder for Blobmoji {
    type Err = ();

    fn new(
        build_path: PathBuf,
        verbose: bool,
        _arguments: &ArgMatches,
    ) -> Result<Box<Self>, Self::Err> {
        let hash_path = build_path.join(String::from(HASHES));
        let builder = Box::new(Blobmoji {
            build_path: build_path,
            name: None,
            images: HashMap::new(),
            hashes: FileHashes::from_path(hash_path.as_path()).unwrap_or_default(),
            verbose,
        });
        Ok(builder)
    }

    fn prepare(&mut self, emoji: &Emoji) -> Result<(), Self::Err> {
        if self.hashes.check(emoji).unwrap_or(false) {
            if let Some(rendered) = self.render_svg(emoji) {
                let quantized = match self.quantize_png(&rendered) {
                    Some(quantized) => quantized,
                    None => &rendered,
                };

                let optimized = match self.optimize_png(quantized) {
                    Ok(optimized) => optimized,
                    Err(_) => Vec::from(quantized),
                };

                self.images.insert(emoji.sequence.clone(), Some(optimized));
            }
        }

        Err(())
    }

    fn build<I>(&mut self, emojis: I, output_file: PathBuf) -> Result<(), Self::Err>
        where
            I: IntoIterator<Item=Emoji>,
    {
        unimplemented!()
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("blobmoji")
    }
}

const WIDTH: u32 = 128;

impl Blobmoji {
    fn render_svg(&self, emoji: &Emoji) -> Option<Vec<u8>> {
        if let Some(svg_path) = &emoji.svg_path {
            let mut opt = resvg::Options::default();
            let path = PathBuf::from(&svg_path.as_os_str());
            opt.usvg.path = Some(path);
            opt.fit_to = FitTo::Width(WIDTH);

            let tree = usvg::Tree::from_file(svg_path, &opt.usvg);
            if let Ok(tree) = tree {
                let mut img = backend_raqote::render_to_image(&tree, &opt);
                if let Some(img) = &mut img {
                    let data = img.get_data();
                    Some(Vec::from(as_u8_slice(data)))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn quantize_png<'a>(&self, img: &'a [u8]) -> Option<&'a [u8]> {
        Some(img)
    }

    fn optimize_png(&self, img: &[u8]) -> PngResult<Vec<u8>> {
        let mut opt = oxipng::Options::default();
        opt.fix_errors = true;
        opt.strip = Safe;

        optimize_from_memory(img, &opt)
    }
}

/// From https://stackoverflow.com/a/29042896
fn as_u8_slice(v: &[u32]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            v.as_ptr() as *const u8,
            v.len() * std::mem::size_of::<u32>(),
        )
    }
}
