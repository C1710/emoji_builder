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

// Microsoft, Windows are trademarks of the Microsoft group of companies.

use std::collections::HashMap;
use std::fs::{copy, create_dir_all, File, remove_file, rename};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{App, Arg, ArgMatches, SubCommand};
use itertools::Itertools;
use pyo3::Python;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sha2::digest::generic_array::GenericArray;
use tiny_skia::Pixmap;
use usvg::FitTo;

use crate::builder::{EmojiBuilder, PreparationResult};
use crate::builders::blobmoji::error::BlobmojiError;
use crate::changes::{CheckError, FileHashes};
use crate::emoji_processor::EmojiProcessor;
use crate::emoji_processors::reduce_colors::ReduceColors;
use crate::emojis::emoji::Emoji;
use crate::loadables::loadable::LoadablePrototype;
use crate::loadables::sources::fs_source::FsSource;

mod waveflag;
/// The error type that can occur for the [Blobmoji] builder
pub mod error;
mod image_utils;
mod noto_emoji_utils;
mod rendering;

#[allow(dead_code)]
/// Represents the configuration for the `Blobmoji` builder
pub struct Blobmoji {
    build_path: PathBuf,
    hashes: FileHashes,
    aliases: Option<PathBuf>,
    render_only: bool,
    default_font: String,
    fontdb: usvg::fontdb::Database,
    waveflag: bool,
    reduce_colors: Option<Box<ReduceColors>>,
    build_win: bool
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BlobmojiConfig {
    hashes: Option<PathBuf>,
    #[serde(default)]
    render_only: bool,
    default_font: Option<String>,
    #[serde(default = "default_true")]
    waveflag: bool,
    palette: Option<PathBuf>,
    #[serde(default)]
    build_win: bool
}

fn default_true() -> bool {
    true
}


const WAVE_FACTOR: f32 = 0.1;

const HASHES: &str = "hashes.csv";
const TMPL_TTX_TMPL: &str = "font.tmpl.ttx.tmpl";
const TMPL_TTX: &str = "font.tmpl.ttx";
const TMPL_TTF: &str = "font.tmpl.ttf";
const TTF: &str = "font.ttf";
const TTF_WITH_PUA: &str = "font.ttf-with-pua";
const TTF_WITH_PUA_VARSE1: &str = "font.ttf-with-pua-varse1";
const PNG_DIR: &str = "png";

const TMPL_TTX_TMPL_CONTENT: &[u8] = include_bytes!("noto-emoji/NotoColorEmoji.tmpl.ttx.tmpl");

impl EmojiBuilder for Blobmoji {
    type Err = BlobmojiError;
    /// An emoji that's "prepared" here is (currently) a path (to the saved PNG file)
    /// and a hash that represents the source SVG
    type PreparedEmoji = (
        PathBuf,
        Result<GenericArray<u8, <Sha256 as Digest>::OutputSize>, CheckError>
    );


    fn new(
        build_path: PathBuf,
        matches: Option<ArgMatches>,
    ) -> Result<Box<Self>, Self::Err> {
        let hash_path = build_path.join(String::from(HASHES));
        let source = FsSource::new(hash_path).unwrap();
        let hashes = FileHashes::load_prototype(&source);
        let hashes = match hashes {
            Ok(hashes) => hashes,
            Err(error) => {
                error!("Couldn't load hashes: {:?}", error);
                FileHashes::default()
            }
        };

        let ttx_tmpl_path = build_path.join(TMPL_TTX_TMPL);

        if !&ttx_tmpl_path.exists() {
            info!("Creating new TTX template");
            let mut file = File::create(&ttx_tmpl_path)?;
            file.write_all(TMPL_TTX_TMPL_CONTENT)?;
        } else {
            info!("Using existing TTX template");
        }

        // Create the PNG directory if it doesn't exist
        let png_dir = build_path.join(PNG_DIR);
        if !png_dir.exists() {
            create_dir_all(png_dir)?;
        };

        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();


        // Collect CLI arguments
        if let Some(matches) = &matches {
            let aliases = match matches.value_of("aliases") {
                None => None,
                Some(aliases) => PathBuf::from_str(aliases).ok()
            };

            let render_only = matches.is_present("render_only");

            let default_font = String::from(matches.value_of("default_font").unwrap_or("cursive"));

            let additional_fonts = matches.values_of_os("additional_fonts");

            let waveflag = matches.is_present("waveflag");

            let reduce_colors = {
                let args = ReduceColors::cli_arguments(&Self::sub_command().p.global_args);
                let arg_names: Vec<&str> = args.iter()
                    .map(|arg| arg.b.name)
                    .collect();
                let matches: HashMap<_, _> = matches.args.iter()
                    .filter(|(arg_name, _)| arg_names.contains(arg_name))
                    .map(|(arg_name, matched_arg)| (*arg_name, matched_arg.clone()))
                    .collect();
                if let Some(reduce_colors_result) = ReduceColors::new(Some(ArgMatches {
                    args: matches,
                    subcommand: None,
                    usage: None,
                })) {
                    match reduce_colors_result {
                        Ok(reduce_colors) => Some(reduce_colors),
                        Err(err) => {
                            error!("{:?}", err);
                            None
                        }
                    }
                } else {
                    None
                }
            };

            // Copy the predefined TTX_TMPL file to the destination
            match matches.value_of("ttx_tmpl") {
                // TODO: Don't unwrap
                Some(ttx_tmpl) => std::fs::copy(PathBuf::from(ttx_tmpl), &ttx_tmpl_path).unwrap(),
                None => 0
            };

            // Load all the additional fonts
            if let Some(additional_fonts) = additional_fonts {
                let font_errors: Vec<std::io::Error> = additional_fonts
                    .map(PathBuf::from)
                    .map(|font_path| if font_path.is_file() {
                        fontdb.load_font_file(font_path)
                    } else if font_path.is_dir() {
                        fontdb.load_fonts_dir(font_path);
                        Ok(())
                    } else {
                        Ok(())
                    })
                    .filter_map(|result| result.err())
                    .collect();
                if !font_errors.is_empty() {
                    Err(BlobmojiError::IoErrors(font_errors))
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }?;

            // Check whether we want to build a Windows-compatible font as well
            let build_win = matches.is_present("win10");

            Ok(Box::new(Blobmoji {
                build_path,
                hashes,
                aliases,
                render_only,
                default_font,
                fontdb,
                waveflag,
                reduce_colors,
                build_win
            }))
        } else {
            Ok(Box::new(Blobmoji {
                build_path,
                hashes,
                aliases: None,
                render_only: false,
                default_font: String::from("cursive"),
                fontdb,
                waveflag: false,
                reduce_colors: None,
                build_win: false
            }))
        }
    }

    fn finish(&mut self, emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>) -> Result<(), Self::Err> {
        self.store_prepared(&emojis)
    }

    fn prepare(&self, emoji: &Emoji) -> PreparationResult<Self::PreparedEmoji, Self:: Err> {
        info!("Preparing {}", emoji);

        // Where to store the image?
        let path = self.build_path
            .join(PNG_DIR)
            .join(PathBuf::from(Blobmoji::generate_filename(emoji)));

        if let Err(err) = self.hashes.check(emoji) {
            warn!("Hash of an emoji ({}) could not be checked: {:?}", emoji, err);
        }

        // Only render if sth. has changed or if it isn't available
        if (!self.hashes.check(emoji).unwrap_or(false)) || (!path.exists()) {
            // Render the SVG to an appropriate, but unpadded size
            if let Some((rendered, (width, height))) = rendering::render_svg(
                self.default_font.clone(),
                self.fontdb.clone(),
                self.reduce_colors.as_ref(),
                self.waveflag,
                emoji
            ) {
                // Wave the flag if it is one and if we're supposed to.
                let (rendered, width, height) = if self.waveflag && emoji.is_flag() {
                    waveflag::waveflag(
                        rendered.data(),
                        width as usize,
                        height,
                        (height as f32 * WAVE_FACTOR) as usize)
                } else {
                    (rendered.data().to_vec(), width, height)
                };
                // The rendering already accounted for the case that this is a flag and that the
                // image will get taller.

                // Add the padding
                let mut image = image_utils::enlarge_to(
                    &rendered,
                    width,
                    height,
                    CHARACTER_WIDTH,
                    RENDER_AND_CHARACTER_HEIGHT,
                );

                // Oxipng needs to work on PNGs and not raw pixels, so it's encoded here.
                // It also makes sense to do quantization at this step, if it is performed at all
                // (which is only the case for the GPL-version which is currently not public)
                let encoded = match self.quantize_to_png(&emoji, &mut image) {
                    Some(quantized) => quantized,
                    None => image_utils::pixels_to_png(&image).unwrap()
                };

                // Lossless compression
                let optimized = match image_utils::optimize_png(&encoded) {
                    Ok(optimized) => optimized,
                    Err(e) => {
                        warn!("Error in optimizing {:?}: {:?}", emoji, e);
                        encoded
                    },
                };

                // Save it
                image_utils::write_png(&self.build_path, emoji, optimized).unwrap();

                // Save the hash value of the source (to prevent unnecessary re-renders)
                let hash = FileHashes::hash(emoji);

                Ok(((path, hash), None))
            } else {
                error!("Couldn't render Emoji {}", emoji);
                Err(BlobmojiError::UnknownError)
            }
        } else {
            info!("Emoji is already available");
            let hash = &self.hashes[emoji];
            // As the hash values can be assumed to be generated just like above,
            // We can safely assume their size to be like this
            let hash: GenericArray<u8, <Sha256 as Digest>::OutputSize> = GenericArray::clone_from_slice(hash);
            Ok(((path, Ok(hash)), None))
        }
    }


    // TODO: Implement
    fn build(
        &mut self,
        emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>,
        output_file: PathBuf,
        ) -> Result<(), Self::Err> {
        assert!(!emojis.is_empty());

        self.store_prepared(&emojis)?;

        if !self.render_only {
            // Normal
            self.build_font(&emojis, &output_file, false);
            // For Windows 10 support
            let mut output_file_stem_windows = output_file.file_stem().unwrap_or_default().to_os_string();
            output_file_stem_windows.push("_win");
            let output_file_windows = output_file
                .with_file_name(output_file_stem_windows)
                .with_extension(output_file.extension().unwrap_or_default());
            self.build_font(&emojis, &output_file_windows, true);
        }

        Ok(())
    }

    fn undo(&self,
            emoji: &Emoji,
            prepared: Result<Self::PreparedEmoji, Self::Err>
        )  -> Result<Result<Self::PreparedEmoji, Self::Err>, Self::Err> {
        if prepared.is_ok() {
            // Delete the image. It will be overwritten the next time,
            // but the building scripts might still use it
            let filename = Blobmoji::generate_filename(emoji);
            let path = self.build_path
                .join(PNG_DIR)
                .join(&PathBuf::from(filename));
            std::fs::remove_file(path)?;
        }
        // When it comes to the hash-saving part, this emoji will be ignored
        // (unless it has been re-rendered until then)
        Ok(Err(BlobmojiError::EmojiInvalidated))
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        let subcommand = SubCommand::with_name("blobmoji")
            .version("0.1.0")
            .author("Constantin A. <emoji.builder@c1710.de>")
            .arg(Arg::with_name("aliases")
                .short("a")
                .long("aliases")
                .value_name("FILE")
                // TODO: Rephrase it to an actually useful help message
                .help("Specify a file containing an alias mapping")
                .takes_value(true)
                .required(false)
            )
            .arg(Arg::with_name("render_only")
                .short("R")
                .long("render_only")
                .help("Only render the images, don't build the font")
                .takes_value(false)
                .required(false)
            )
            .arg(Arg::with_name("default_font")
                .short("F")
                .long("default_font")
                .help("The font to use if either none is specified or the chosen one is not available")
                .takes_value(true)
                .default_value("cursive")
                .required(false))
            .arg(Arg::with_name("additional_fonts")
                .long("font_files")
                .help("Additional fonts to load besides the system provided ones")
                .long_help("Additional fonts to load besides the system provided ones. \
                You may specify directories or files")
                .takes_value(true)
                .required(false)
                .value_name("FILE/DIR")
                .multiple(true))
            .arg(Arg::with_name("waveflag")
                .short("w")
                .long("waveflag")
                .help("Enable if the flags should get a wavy appearance.")
                .takes_value(false)
                .required(false))
            .arg(Arg::with_name("ttx_tmpl")
                .long("ttx-tmpl")
                .help("A template file for the font, e.g. containing version and author information")
                .takes_value(true)
                .required(false)
                .value_name("FILE"))
            .arg(Arg::with_name("win10")
                .long("win")
                .help("Build a Windows 10-compatible font as well (it contains additional font tables)")
                .long_help("Build a Windows 10-compatible font as well (it contains additional font tables).\nMicrosoft, Windows are trademarks of the Microsoft group of companies.")
                .takes_value(false)
                .required(false));
        let reduce_color_args = ReduceColors::cli_arguments(&subcommand.p.global_args);
        subcommand.args(&reduce_color_args)
    }
}

/// The width of the image that's _embedded_ into the font
const CHARACTER_WIDTH: u32 = 136;
/// The width of the image that's _rendered_
const RENDER_WIDTH: u32 = 128;
/// The height of the image (it's the same when it's rendered and when it's embedded)
const RENDER_AND_CHARACTER_HEIGHT: u32 = 128;


impl Blobmoji {
    /// Performs the quantization process which apparently does some sort of posterization to reduce
    /// the number of colors in the image.
    /// Due to licensing issues, this function (unfortunately) does nothing at all and is only
    /// implemented in a fork (which is - at the moment of writing - not released).
    ///
    /// Errors are not returned as this would need knowledge of the error type which relies on the
    /// library being present. Therefore any errors are directly shown (using `warn!`) inside of the
    /// function.
    /// This is also the reason why `emoji` is required here, it's used to generate meaningful error
    /// messages.
    fn quantize_to_png(&self, _emoji: &Emoji, _img: &mut [u8]) -> Option<Vec<u8>> {
        None
    }

    const EMPTY_PIXEL: [u8; 4] = [0; 4];

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

    fn store_prepared(&mut self, emojis: &HashMap<&Emoji, Result<<Blobmoji as EmojiBuilder>::PreparedEmoji, <Blobmoji as EmojiBuilder>::Err>>) -> Result<(), BlobmojiError> {
        // Collect all errors that occurred while checking the hashes and save those that were successful
        let hashing_errors = emojis.iter()
            .filter_map(|(emoji, result)| match result {
                Ok((_, hash)) => Some((emoji, hash)),
                // It's not the task of this function to handle errors
                Err(_) => None
            })
            .filter_map(|(emoji, hash)|
                match hash {
                    Ok(hash) => {
                        // Update the hash value
                        self.hashes.update(emoji, hash);
                        None
                    },
                    Err(error) => Some((emoji, error))
                })
            .collect_vec();

        // Save all hashes
        let saving_results = self.hashes.write_to_path(self.build_path.join(HASHES));

        for (emoji, err) in hashing_errors {
            error!("Error in updating a hash value for emoji {}: {:?}", emoji, err);
        }

        match saving_results {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn build_font(&self,
                  emojis: &HashMap<&Emoji, Result<<Self as EmojiBuilder>::PreparedEmoji, <Self as EmojiBuilder>::Err>>,
                  output_file: &Path,
                  add_cmap_and_glyf: bool
    ) {
        // TODO: Build the font (the following steps are copied from the original Makefile
        //       (cf. https://github.com/googlefonts/noto-emoji/blob/master/Makefile)
        // (% is just used as a placeholder, just like in the Makefile)
        // 1. (from the documentation: "[...] extends the cmap, hmtx, GSUB and GlyphOrder tabled
        //     [...] apply aliases [...]"
        //    python3 add_glyphs.py -f "%.ttx.tmpl" -o "%.ttx" -d "<PNG-dir>" -a emoji_aliases.txt
        // 2. (Seems to use the TTX-tool to overwrite the old font)
        //    rm -f %.ttf
        //    ttx   %.ttx
        // 3. ???? (we will not check sequences.
        //    If this should be implemented, it will be at a much earlier point.)
        //    (About that -5:
        //     # flag for emoji builder.  Default to legacy small metrics for the time being.)
        //    python3 third_party/color_emoji/emoji_builder.py -5 -V <name>.tmpl.ttf "<name>.ttf" "<PNG-dir>/emoji_u"
        //    python3 map_pua_emoji.py "<name>.ttf" "<name>.ttf-with-pua"
        //    // Is this step still necessary?
        //    [python3] add_vs_cmap.py -vs 2640 2642 2695 --dstdir '.' -o "<name>.ttf-with-pua-varse1" "<name>.ttf-with-pua"
        //    mv "<name>.ttf-with-pua-varse1" "<name>.ttf" ????
        //    rm "<name>.ttf-with-pua"

        // TODO: - Understand add_glyphs, find a workaround so we don't need to deal with files there
        //       - Integrate TTX (to compile the .ttx to a .ttf)
        //       - Understand what emoji_builder.py does
        //       - Understand what map_pua_emoji does
        //       - Check how and whether add_vs_cmap.py is actually needed here or if it needs to be
        //         moved to an earlier step.
        //       - Implement

        // TODO: Handle errors
        info!("Adding glyphs");
        match noto_emoji_utils::add_glyphs(
            &self.aliases,
            &emojis,
            self.build_path.join(TMPL_TTX_TMPL),
            self.build_path.join(TMPL_TTX),
            add_cmap_and_glyf
        ) {
            Ok(_) => (),
            Err(err) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                err.print(py);
            }
        };

        let tmpl_ttf = self.build_path.join(TMPL_TTF);
        // TODO: This if-condition might be unnecessary
        if tmpl_ttf.exists() {
            remove_file(tmpl_ttf).unwrap();
        }

        info!("Building TTF");
        match noto_emoji_utils::build_ttf(&self.build_path) {
            Ok(_) => (),
            Err(err) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                err.print(py);
                panic!()
            }
        };

        info!("Doing... something");
        match noto_emoji_utils::emoji_builder(&self.build_path, add_cmap_and_glyf) {
            Ok(_) => (),
            Err(err) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                err.print(py);
                panic!()
            }
        };

        info!("Mapping PUA");
        match noto_emoji_utils::map_pua(&self.build_path) {
            Ok(_) => (),
            Err(err) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                err.print(py);
                panic!()
            }
        };

        info!("Adding Version Selector");
        match noto_emoji_utils::add_vs_cmap(&self.build_path) {
            Ok(_) => (),
            Err(err) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                err.print(py);
                panic!()
            }
        };

        rename(
            self.build_path.join(TTF_WITH_PUA_VARSE1),
            self.build_path.join(TTF)
        ).unwrap();

        copy(self.build_path.join(TTF), output_file).unwrap();

        remove_file(self.build_path.join(TTF_WITH_PUA)).unwrap();
        remove_file(self.build_path.join(TMPL_TTX)).unwrap();
        remove_file(self.build_path.join(TMPL_TTF)).unwrap();
        remove_file(self.build_path.join(TTF)).unwrap();
    }
}


/// Gets the length of the hexadecimal representation of an integer
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