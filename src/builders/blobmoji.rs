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
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use clap::{App, ArgMatches, SubCommand};
use itertools::Itertools;
use oxipng::{optimize, optimize_from_memory, PngResult};
use oxipng::internal_tests::Headers::Safe;
use png::HasParameters;
use resvg::backend_raqote;
use resvg::FitTo;
use resvg::prelude::*;

use crate::builder::EmojiBuilder;
use crate::changes::FileHashes;
use crate::emoji::Emoji;

#[allow(dead_code)]
pub struct Blobmoji {
    build_path: PathBuf,
    name: Option<String>,
    hashes: FileHashes,
    verbose: bool,
}

const HASHES: &str = "hashes.csv";

impl EmojiBuilder for Blobmoji {
    type Err = ();
    type PreparedEmoji = PathBuf;

    fn new(
        build_path: PathBuf,
        verbose: bool,
        _arguments: Option<&ArgMatches>,
    ) -> Result<Box<Self>, Self::Err> {
        let hash_path = build_path.join(String::from(HASHES));
        let builder = Box::new(Blobmoji {
            build_path,
            name: None,
            hashes: FileHashes::from_path(hash_path.as_path()).unwrap_or_default(),
            verbose,
        });
        Ok(builder)
    }

    fn prepare(&self, emoji: &Emoji) -> Result<Self::PreparedEmoji, Self::Err> {
        if self.verbose {
            println!("Preparing {}", emoji);
        }

        if !self.hashes.check(emoji).unwrap_or(false) {
            if let Some(rendered) = self.render_svg(emoji) {
                let quantized = match self.quantize_png(&rendered) {
                    Some(quantized) => quantized,
                    None => &rendered,
                };

                let optimized = match self.optimize_png(quantized) {
                    Ok(optimized) => optimized,
                    Err(_) => Vec::from(quantized),
                };

                self.write_png(emoji, optimized);

                Ok(Blobmoji::generate_filename(emoji).into())
            } else {
                eprintln!("Couldn't render Emoji {}", emoji);
                Err(())
            }
        } else {
            if self.verbose {
                println!("Emoji is already available");
            }
            Err(())
        }
    }

    // TODO: Implement
    fn build(
        &mut self,
        emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>,
        output_file: PathBuf,
    ) -> Result<(), Self::Err> {
        // Currently the only task is to render the emojis...
        Ok(())
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("blobmoji")
    }
}

const WIDTH: u32 = 136;
const IMG_WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

impl Blobmoji {
    fn render_svg(&self, emoji: &Emoji) -> Option<Vec<u8>> {
        if let Some(svg_path) = &emoji.svg_path {
            let mut opt = resvg::Options::default();
            let path = PathBuf::from(&svg_path.as_os_str());
            opt.usvg.path = Some(path);
            opt.fit_to = FitTo::Width(IMG_WIDTH);

            let tree = usvg::Tree::from_file(svg_path, &opt.usvg);
            if let Ok(tree) = tree {
                let mut img = backend_raqote::render_to_image(&tree, &opt);
                if let Some(img) = img {
                    let mut data = img.into_vec();
                    let data = as_u8_slice(&mut data);
                    bgra_to_rgba(data);
                    Some(Vec::from(data))
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


    fn write_png(&self, emoji: &Emoji, image: Vec<u8>) {
        let filename = Blobmoji::generate_filename(&emoji);
        let path = self.build_path.join(&PathBuf::from(filename));
        let file = File::create(path);

        let image = Blobmoji::enlarge_width(&image);

        if let Ok(file) = file {
            let writer = &mut BufWriter::new(file);

            let mut encoder = png::Encoder::new(writer, WIDTH, HEIGHT);
            encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&image).unwrap();
        }
    }

    fn quantize_png<'a>(&self, img: &'a [u8]) -> Option<&'a [u8]> {
        Some(img)
    }

    fn optimize_png(&self, img: &[u8]) -> PngResult<Vec<u8>> {
        let mut opt = oxipng::Options::default();
        opt.fix_errors = true;
        opt.strip = Safe;
        opt.color_type_reduction = true;
        opt.palette_reduction = true;
        opt.bit_depth_reduction = true;

        optimize_from_memory(img, &opt)
    }

    /// The output images are supposed to be a bit wider than the square images.
    /// This function currently copies the whole image which is kinda inefficient.
    fn enlarge_width(content: &[u8]) -> Vec<u8> {
        Blobmoji::enlarge_by(content, IMG_WIDTH, HEIGHT, WIDTH - IMG_WIDTH, 0)
    }

    const EMPTY_PIXEL: [u8; 4] = [0; 4];

    /// Adds a transparent area around an image and puts it in the center
    /// If a delta value is odd, the image will be positioned 1 pixel left of the center
    fn enlarge_by(
        content: &[u8],
        src_width: u32,
        src_height: u32,
        d_width: u32,
        d_height: u32,
    ) -> Vec<u8> {
        // The padding will be added as follows:
        //
        // |  pad_vert   |
        // |-------------|
        // |  |      |   |
        // |ph| cont |ph |
        // |  |      |   |
        // |-------------|
        // |  pad_vert   |
        // |             |

        // The padding will be computed width the smaller side (if one is smaller than the other)
        // If d % 2 = 1, round it down by 1,
        // If d % 2 = 0, don't round
        // i.e., subtract d % 2!
        let d_width_rounded = d_width - (d_width % 2);
        let d_height_rounded = d_height - (d_height % 2);

        let target_width = src_width + d_width;
        let target_height = src_height + d_height;

        let pad_horizontal = d_width_rounded * 4;
        let pad_vertical = d_height_rounded * target_width * 4;

        let pad_horizontal = vec![0; pad_horizontal as usize / 2];
        let pad_vertical = vec! {0; pad_vertical as usize / 2};

        let mut image = Vec::with_capacity((target_width * target_height * 4) as usize);

        image.extend_from_slice(&pad_vertical);
        for line in 0..src_height as usize {
            image.extend_from_slice(&pad_horizontal);
            image.extend_from_slice(content);
            image.extend_from_slice(&pad_horizontal);
            //
            if d_width % 2 != 0 {
                image.extend_from_slice(&Blobmoji::EMPTY_PIXEL);
            }
        }
        image.extend_from_slice(&pad_vertical);

        if d_height % 2 != 0 {
            image.extend_from_slice(&vec![0; target_width as usize]);
        }

        image
    }

    fn generate_filename(emoji: &Emoji) -> String {
        let mut codepoints = emoji.sequence.iter()
            .map(|codepoint| format!("{:x}", codepoint));
        let codelength: usize = emoji.sequence.iter()
            .map(|codepoint| hex_len(*codepoint))
            .sum();
        let delimiters = emoji.sequence.len() - 1;
        // codelength + delimiters + "emoji".len() + "_u".len() + ".png".len()
        let mut filename = String::with_capacity(codelength + delimiters + 5 + 2 + 4);
        filename.push_str("emoji_u");
        filename.push_str(&codepoints.join("_"));
        filename.push_str(".png");
        filename
    }
}


/// For some reason, resvg/raqote produces a vector of u32 values, which contain the color value for
/// one pixel in the format BGRA (and not RGBA like it is required by the following tools).
fn bgra_to_rgba(data: &mut [u8]) {
    for i in (0..data.len()).step_by(4) {
        data.swap(i, i + 2);
    }
}

#[test]
fn test_rgba() {
    let mut bgra: Vec<u8> = vec![0, 42, 17, 21, 1, 2, 3, 4, 5, 6, 7, 8];
    bgra_to_rgba(&mut bgra);
    let rgba: Vec<u8> = vec![17, 42, 0, 21, 3, 2, 1, 4, 7, 6, 5, 8];
    assert_eq!(rgba, bgra);
}

/// See https://stackoverflow.com/a/29042896
fn as_u8_slice(v: &mut [u32]) -> &mut [u8] {
    unsafe {
        std::slice::from_raw_parts_mut(
            v.as_ptr() as *mut u8,
            v.len() * std::mem::size_of::<u32>(),
        )
    }
}

/// Efficiently gets the length of the hexadecimal representation of an integer
fn hex_len(mut i: u32) -> usize {
    let mut len = 0;
    while i > 0 {
        i >>= 4;
        len += 1;
    };
    len
}

#[test]
fn test_hex() {
    let a = 0x1f914;
    let b = 0xfffff;
    let c = 0x00000;
    let d = 0x00001;
    assert_eq!(5, hex_len(a));
    assert_eq!(5, hex_len(b));
    assert_eq!(0, hex_len(c));
    assert_eq!(1, hex_len(d));
}