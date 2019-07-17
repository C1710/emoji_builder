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
use oxipng::{optimize_from_memory, PngResult};
use oxipng::internal_tests::Headers::Safe;
use png::HasParameters;
use resvg::backend_raqote;
use resvg::FitTo;
use resvg::prelude::*;
use sha2::{Digest, Sha256};
use sha2::digest::generic_array::GenericArray;

use crate::builder::EmojiBuilder;
use crate::changes::{CheckError, FileHashes};
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
    type PreparedEmoji = (PathBuf, Result<GenericArray<u8, <Sha256 as Digest>::OutputSize>, CheckError>);

    fn new(
        build_path: PathBuf,
        verbose: bool,
        _arguments: Option<ArgMatches>,
    ) -> Result<Box<Self>, Self::Err> {
        let hash_path = build_path.join(String::from(HASHES));
        let hashes = FileHashes::from_path(hash_path.as_path());
        let hashes = match hashes {
            Ok(hashes) => hashes,
            Err(error) => {
                if verbose {
                    eprintln!("Couldn't load hash values: {:?}", error);
                }
                FileHashes::default()
            }
        };
        let builder = Box::new(Blobmoji {
            build_path,
            name: None,
            hashes,
            verbose,
        });
        Ok(builder)
    }

    fn prepare(&self, emoji: &Emoji) -> Result<Self::PreparedEmoji, Self::Err> {
        if self.verbose {
            println!("Preparing {}", emoji);
        }

        let path = self.build_path.join(PathBuf::from(Blobmoji::generate_filename(emoji)));

        if self.verbose {
            if let Err(err) = self.hashes.check(emoji) {
                eprintln!("Hash of an emoji ({}) could not be checked: {:?}", emoji, err);
            }
        }

        if (!self.hashes.check(emoji).unwrap_or(false)) || (!path.exists()) {
            if let Some(rendered) = self.render_svg(emoji) {
                let fit_to = rendered.1;
                let rendered = rendered.0;
                let quantized = match self.quantize_png(&rendered) {
                    Some(quantized) => quantized,
                    None => &rendered,
                };

                let optimized = match self.optimize_png(quantized) {
                    Ok(optimized) => optimized,
                    Err(_) => Vec::from(quantized),
                };

                self.write_png(emoji, optimized, fit_to);

                let hash = FileHashes::hash(emoji);

                Ok((path, hash))
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
        _output_file: PathBuf,
    ) -> Result<(), Self::Err> {
        // Save the new hashes

        let hash_errors = emojis.iter()
            .filter_map(|(emoji, result)| match result {
                Ok((_, hash)) => Some((emoji, hash)),
                Err(_) => None
            })
            .filter_map(|(emoji, hash)|
                match hash {
                    Ok(_) => None,
                    Err(error) => Some((emoji, error))
                })
            .collect_vec();


        let hash_error = self.hashes.write_to_path(self.build_path.join(HASHES));

        for (emoji, err) in hash_errors {
            eprintln!("Error in updating a hash value for emoji {}: {:?}", emoji, err);
        }
        if let Err(err) = hash_error {
            eprintln!("Error in writing the new hash values: {:?}\n\
            All emojis will be re-rendered the next time.", err);
        }

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
    fn render_svg(&self, emoji: &Emoji) -> Option<(Vec<u8>, FitTo)> {
        if let Some(svg_path) = &emoji.svg_path {
            let mut opt = resvg::Options::default();
            let path = PathBuf::from(&svg_path.as_os_str());
            opt.usvg.path = Some(path);
            // Just as a fallback. On Windows and Mac OS it will use Comic Sans
            // which is pretty close to Comic Neue, which is used in Blobmoji
            // TODO: Maybe make this an argument...
            opt.usvg.font_family = String::from("cursive");

            opt.fit_to = FitTo::Width(IMG_WIDTH);
            let tree = usvg::Tree::from_file(svg_path, &opt.usvg);

            if let Ok(tree) = tree {
                let size = tree.svg_node().size.to_screen_size();
                opt.fit_to = if size.height() > size.width() {
                    FitTo::Height(HEIGHT)
                } else {
                    FitTo::Width(IMG_WIDTH)
                };

                let img = backend_raqote::render_to_image(&tree, &opt);
                if let Some(img) = img {
                    let mut data = img.into_vec();
                    let data = as_u8_slice(&mut data);
                    bgra_to_rgba(data);
                    Some((Vec::from(data), opt.fit_to))
                } else {
                    eprintln!("Failed to render {}", emoji);
                    None
                }
            } else {
                let err = tree.err().unwrap();
                eprintln!("Error in loading the SVG file for {}: {:?}", emoji, err);
                None
            }
        } else {
            eprintln!("No file available for {}", emoji);
            None
        }
    }


    fn write_png(&self, emoji: &Emoji, image: Vec<u8>, fit_to: FitTo) {
        let filename = Blobmoji::generate_filename(&emoji);
        let path = self.build_path.join(&PathBuf::from(filename));
        let file = File::create(path);

        let (width, height) = match fit_to {
            FitTo::Width(width) => (Some(width), None),
            FitTo::Height(height) => (None, Some(height)),
            _ => panic!("fit_to needs to be either Height or Width")
        };

        let image = Blobmoji::enlarge_to(
            &image,
            width,
            height,
            WIDTH,
            HEIGHT,
        );

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

    const EMPTY_PIXEL: [u8; 4] = [0; 4];

    /// Adds a transparent area around an image and puts it in the center
    /// If a delta value is odd, the image will be positioned 1 pixel left of the center.
    ///
    /// Will panic, if neither source width, nor source height is given
    fn enlarge_by(
        content: &[u8],
        src_width: Option<u32>,
        src_height: Option<u32>,
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

        let pixels = content.len() as u32 / 4;

        let (src_width, src_height) = match (src_width, src_height) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, pixels / width),
            (None, Some(height)) => (pixels / height, height),
            (None, None) => panic!("No dimensions have been given"),
        };

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
            let start = line * src_width as usize * 4;
            let end = (line + 1) * src_width as usize * 4;
            image.extend_from_slice(&content[start..end]);
            image.extend_from_slice(&pad_horizontal);
            //
            if d_width % 2 != 0 {
                image.extend_from_slice(&Blobmoji::EMPTY_PIXEL);
            }
        }
        image.extend_from_slice(&pad_vertical);

        // If necessary, add an extra line at the bottom.
        if d_height % 2 != 0 {
            image.extend_from_slice(&vec![0; target_width as usize * 4]);
        }

        image
    }

    fn enlarge_to(
        content: &[u8],
        src_width: Option<u32>,
        src_height: Option<u32>,
        target_width: u32,
        target_height: u32,
    ) -> Vec<u8> {
        let pixels = content.len() as u32 / 4;

        let (width, height) = match (src_width, src_height) {
            (None, None) => panic!("No dimensions have been given"),
            (None, Some(height)) => (pixels / height, height),
            (Some(width), None) => (width, pixels / width),
            (Some(width), Some(height)) => (width, height)
        };

        assert!(target_width >= width);
        assert!(target_height >= height);

        let d_width = target_width.saturating_sub(width);
        let d_height = target_height.saturating_sub(height);
        Self::enlarge_by(content, src_width, src_height, d_width, d_height)
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