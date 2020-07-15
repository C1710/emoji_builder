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

use std::collections::{HashMap, HashSet};
use std::fs::{copy, create_dir_all, File, remove_file, rename};
use std::io::Write;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{App, Arg, ArgMatches, SubCommand};
use itertools::Itertools;
use oxipng::{optimize_from_memory, PngResult};
use oxipng::internal_tests::Headers::Safe;
use png::BitDepth::Eight;
use png::ColorType::RGBA;
use png::EncodingError;
use pyo3::{IntoPy, PyResult, Python};
use pyo3::prelude::PyModule;
use pyo3::types::{PyDict, PyString, PyTuple};
use sha2::{Digest, Sha256};
use sha2::digest::generic_array::GenericArray;
use usvg::{FitTo, SystemFontDB};

use crate::builder::EmojiBuilder;
use crate::builders;
use crate::changes::{CheckError, FileHashes};
use crate::emoji::Emoji;

mod waveflag;

#[allow(dead_code)]
pub struct Blobmoji {
    build_path: PathBuf,
    hashes: FileHashes,
    aliases: Option<PathBuf>,
    render_only: bool,
    default_font: String,
    fontdb: usvg::fontdb::Database,
    waveflag: bool,
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
    type Err = ();
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
        let hashes = FileHashes::from_path(hash_path.as_path());
        let hashes = match hashes {
            Ok(hashes) => hashes,
            Err(error) => {
                match error.kind() {
                    csv::ErrorKind::Io(error) => match error.kind() {
                        std::io::ErrorKind::NotFound => warn!("File with hashes not found, probably because it's the first build. {:?}", error),
                        _ => error!("Couldn't load hashes: {:?}", error)
                    },
                    _ => error!("Couldn't load hashes: {:?}", error)
                };
                FileHashes::default()
            }
        };

        let aliases = match &matches {
            None => None,
            Some(arg_matches) => match arg_matches.value_of("aliases") {
                None => None,
                Some(aliases) => PathBuf::from_str(aliases).ok()
            }
        };

        let render_only = match &matches {
            None => false,
            Some(matches) => matches.is_present("render_only")
        };

        let default_font = match &matches {
            None => String::from("cursive"),
            Some(matches) => String::from(matches.value_of("default_font").unwrap_or("cursive"))
        };

        let addtional_fonts = match &matches {
            None => None,
            Some(matches) => matches.values_of_os("additional_fonts")
        };

        let waveflag = match &matches {
            None => false,
            Some(matches) => matches.is_present("waveflag")
        };

        let ttx_tmpl_path = build_path.join(TMPL_TTX_TMPL);
        if !ttx_tmpl_path.exists() {
            // TODO: Don't unwrap
            info!("Creating new TTX template");
            let mut file = File::create(ttx_tmpl_path).unwrap();
            file.write_all(TMPL_TTX_TMPL_CONTENT).unwrap();
        } else {
            info!("Using existing TTX template");
        }

        // Create the PNG directory if it doesn't exist
        let png_dir = build_path.join(PNG_DIR);
        if !png_dir.exists() {
            create_dir_all(png_dir).unwrap();
        };

        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();

        // Load all the additional fonts
        if let Some(additional_fonts) = addtional_fonts {
            additional_fonts
                .map(PathBuf::from)
                .for_each(|font_path| if font_path.is_file() {
                    fontdb.load_font_file(font_path).unwrap()
                } else if font_path.is_dir() {
                    fontdb.load_fonts_dir(font_path)
                });
        }

        let builder = Box::new(Blobmoji {
            build_path,
            hashes,
            aliases,
            render_only,
            default_font,
            fontdb,
            waveflag
        });
        Ok(builder)
    }

    fn prepare(&self, emoji: &Emoji) -> Result<Self::PreparedEmoji, Self::Err> {
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
            if let Some((rendered, (width, height))) = self.render_svg(emoji) {
                // Wave the flag if it is one and if we're supposed to.
                let (rendered, width, height) = if self.waveflag && emoji.is_flag() {
                    waveflag::waveflag(
                        &rendered,
                        width as usize,
                        height,
                        (height as f32 * WAVE_FACTOR) as usize)
                } else {
                    (rendered, width, height)
                };
                // The rendering already accounted for the case that this is a flag and that the
                // image will get taller.

                // Add the padding
                let image = Blobmoji::enlarge_to(
                    &rendered,
                    width,
                    height,
                    CHARACTER_WIDTH,
                    RENDER_AND_CHARACTER_HEIGHT,
                );

                // Compress the image for the first time
                let quantized = match self.quantize_png(image) {
                    Some(quantized) => quantized,
                    None => rendered,
                };

                // Oxipng needs to work on PNGs and not raw pixels, so it's encoded here.
                let encoded = Blobmoji::pixels_to_png(&quantized).unwrap();

                // Lossless compression
                let optimized = match self.optimize_png(&encoded) {
                    Ok(optimized) => optimized,
                    Err(e) => {
                        warn!("Error in optimizing {:?}: {:?}", emoji, e);
                        encoded
                    },
                };

                // Save it
                self.write_png(emoji, optimized).unwrap();

                // Save the hash value of the source (to prevent unnecessary re-renders)
                let hash = FileHashes::hash(emoji);

                Ok((path, hash))
            } else {
                error!("Couldn't render Emoji {}", emoji);
                Err(())
            }
        } else {
            info!("Emoji is already available");
            let hash = &self.hashes[emoji];
            // As the hash values can be assumed to be generated just like above,
            // We can safely assume their size to be like this
            let hash: GenericArray<u8, <Sha256 as Digest>::OutputSize> = GenericArray::clone_from_slice(hash);
            Ok((path, Ok(hash)))
        }
    }


    // TODO: Implement
    fn build(
        &mut self,
        emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>,
        output_file: PathBuf,
        ) -> Result<(), Self::Err> {
        assert!(!emojis.is_empty());

        self.store_prepared(&emojis);

        if !self.render_only {
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
            match self.add_glyphs(
                &emojis,
                self.build_path.join(TMPL_TTX_TMPL),
                self.build_path.join(TMPL_TTX)
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
            match self.build_ttf() {
                Ok(_) => (),
                Err(err) => {
                    let gil = Python::acquire_gil();
                    let py = gil.python();
                    err.print(py);
                    panic!()
                }
            };

            info!("Doing... something");
            match self.emoji_builder() {
                Ok(_) => (),
                Err(err) => {
                    let gil = Python::acquire_gil();
                    let py = gil.python();
                    err.print(py);
                    panic!()
                }
            };

            info!("Mapping PUA");
            match self.map_pua() {
                Ok(_) => (),
                Err(err) => {
                    let gil = Python::acquire_gil();
                    let py = gil.python();
                    err.print(py);
                    panic!()
                }
            };

            info!("Adding Version Selector");
            match self.add_vs_cmap() {
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
        }

        // Currently the only task is to render the emojis...
        Ok(())
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("blobmoji")
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
    }

    fn log_modules() -> Vec<String> {
        vec![
            String::from("oxipng"),
            String::from(module_path!())
        ]
    }

    fn finish(&mut self, emojis: HashMap<&Emoji, Result<Self::PreparedEmoji, Self::Err>>) -> Result<(), Self::Err> {
        self.store_prepared(&emojis);
        Ok(())
    }
}

/// The width of the image that's _embedded_ into the font
const CHARACTER_WIDTH: u32 = 136;
/// The width of the image that's _rendered_
const RENDER_WIDTH: u32 = 128;
/// The height of the image (it's the same when it's rendered and when it's embedded)
const RENDER_AND_CHARACTER_HEIGHT: u32 = 128;


impl Blobmoji {
    /// Renders a single emoji.
    /// It will not pad the image, however it will return whether it is taller than wide
    /// (`FitTo::Height`) or if it's wider than tall (`FitTo::Width`).
    /// The exact value is always 128px (i.e. the target size for the largest dimension).
    /// # Arguments
    /// * `emoji` - the emoji to be rendered
    /// # Returns
    /// An `Option` containing the image as a vector of RGBA pixels and the dimensions of the
    /// image.
    fn render_svg(&self, emoji: &Emoji) -> Option<(Vec<u8>, (u32, u32))> {
        if let Some(svg_path) = &emoji.svg_path {
            let mut opt = usvg::Options::default();
            let path = PathBuf::from(&svg_path.as_os_str());
            opt.path = Some(path);
            // Just as a fallback. Default is "cursive",
            // which on Windows and Mac OS it will use Comic Sans
            // which is pretty close to Comic Neue, that is used in Blobmoji
            opt.font_family = self.default_font.clone();
            opt.fontdb = self.fontdb.clone();

            let tree = usvg::Tree::from_file(svg_path, &opt);

            if let Ok(tree) = tree {
                // It's easier to get the dimensions here
                let size = tree.svg_node().size.to_screen_size();
                let wave_padding = if emoji.is_flag() && self.waveflag {
                    (size.height() as f32 * WAVE_FACTOR) as u32
                } else {
                    0
                };
                // Adjust the target size if it's going to be taller than 128px
                let fit_to = if (size.height() + wave_padding) > size.width() {
                    // We might need to account for waving flags
                    FitTo::Height((RENDER_AND_CHARACTER_HEIGHT as f32 * (1.0 - WAVE_FACTOR)) as u32)
                } else {
                    FitTo::Width(RENDER_WIDTH)
                };

                // This is the point where it's actually rendered
                let img = resvg::render(&tree, fit_to, None);

                if let Some(img) = img {
                    let width = img.width();
                    let height = img.height();
                    let data = img.take();
                    Some((data, (width, height)))
                } else {
                    error!("Failed to render {}", emoji);
                    None
                }
            } else {
                let err = tree.err().unwrap();
                error!("Error in loading the SVG file for {}: {:?}", emoji, err);
                None
            }
        } else {
            error!("No file available for {}", emoji);
            None
        }
    }

    fn pixels_to_png(img: &[u8]) -> Result<Vec<u8>, EncodingError> {
        // According to this post, PNG files have a header of 8 bytes: https://stackoverflow.com/questions/10423942/what-is-the-header-size-of-png-jpg-jpeg-bmp-gif-and-other-common-graphics-for
        let mut png_target = Vec::with_capacity(img.len() + 8);
        let mut encoder = png::Encoder::new(&mut png_target, CHARACTER_WIDTH, RENDER_AND_CHARACTER_HEIGHT);
        encoder.set_color(RGBA);
        encoder.set_depth(Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(img)?;
        // writer still borrows png_target. Fortunately we don't need it anymore
        std::mem::drop(writer);
        Ok(png_target)
    }

    /// Saves the already encoded PNG file
    fn write_png(&self, emoji: &Emoji, image: Vec<u8>) -> std::io::Result<()> {
        let filename = Blobmoji::generate_filename(&emoji);
        let path = self.build_path
            .join(PNG_DIR)
            .join(&PathBuf::from(filename));
        let mut file = File::create(path)?;
        file.write_all(&image)
    }

    fn quantize_png(&self, img: Vec<u8>) -> Option<Vec<u8>> {
        // Unfortunately the library that's originally used here, is not (at least not easily)
        // compatible with Windows 10.
        // There exists an updated versions, but the license that's used there is too restrictive.
        Some(img)
    }

    /// Runs `oxipng` on the image. It has to be encoded as PNG first
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
    fn enlarge_by(
        content: &[u8],
        src_width: u32,
        src_height: u32,
        d_width: u32,
        d_height: u32,
    ) -> Vec<u8> {
        // The padding will be added as follows:
        //
        // |  pad_vert   |  pad_vert = padding vertical = d_height/2
        // |-------------|
        // |  |      |   |
        // |ph| cont |ph |  ph = padding horizontal = d_width/2
        // |  |      |   |
        // |-------------|
        // |  pad_vert   |
        // |             |


        // If the delta value is odd, we need to have the left/top padding one pixel smaller.
        // The approach here is to add the shorter padding and add a one pixel padding later.
        // If d % 2 = 1, round it down by 1,
        // If d % 2 = 0, don't round
        // That's the same as subtracting d % 2
        let d_width_rounded = d_width - (d_width % 2);
        let d_height_rounded = d_height - (d_height % 2);

        // This is what we eventually want to have
        let target_width = src_width + d_width;
        let target_height = src_height + d_height;

        // The smaller padding side's lengths. As we assume that every pixel consists of 4 subpixels
        // (RGBA), we'll need to multiply by 4 here.
        let pad_horizontal = d_width_rounded * 4;
        let pad_vertical = d_height_rounded * target_width * 4;

        // Prepare the actual padding data
        let pad_horizontal = vec![0; pad_horizontal as usize / 2];
        let pad_vertical = vec![0; pad_vertical as usize / 2];

        // This is the target image
        let mut image = Vec::with_capacity((target_width * target_height * 4) as usize);

        // Add the top padding (the shorter one)
        image.extend_from_slice(&pad_vertical);
        for line in 0..src_height as usize {
            // Add the left padding
            image.extend_from_slice(&pad_horizontal);
            // Add the image's line
            let start = line * src_width as usize * 4;
            let end = (line + 1) * src_width as usize * 4;
            image.extend_from_slice(&content[start..end]);
            // Add the right padding
            image.extend_from_slice(&pad_horizontal);
            // If necessary, add an extra pixel at the right side
            if d_width % 2 != 0 {
                image.extend_from_slice(&Blobmoji::EMPTY_PIXEL);
            }
        }
        // Add the bottom padding
        image.extend_from_slice(&pad_vertical);

        // If necessary, add an extra line at the bottom.
        if d_height % 2 != 0 {
            image.extend_from_slice(&vec![0; target_width as usize * 4]);
        }

        image
    }

    fn enlarge_to(
        content: &[u8],
        src_width: u32,
        src_height: u32,
        target_width: u32,
        target_height: u32,
    ) -> Vec<u8> {
        assert!(target_width >= src_width);
        assert!(target_height >= src_height);

        // Although the two asserts already make sure that we don't get that case, saturating_sub
        // is used to prevent overflows.
        let d_width = target_width.saturating_sub(src_width);
        let d_height = target_height.saturating_sub(src_height);
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


    const ADD_GLYPHS_PY: &'static str = include_str!("add_glyphs/add_glyphs.py");
    const ADD_ALIASES_PY: &'static str = include_str!("add_glyphs/add_aliases.py");
    const ADD_EMOJI_GSUB_PY: &'static str = include_str!("add_glyphs/add_emoji_gsub.py");

    // TODO: Implement
    fn add_glyphs(&self,
                  emojis: &HashMap<&Emoji, Result<
                      <builders::blobmoji::Blobmoji as EmojiBuilder>::PreparedEmoji,
                      <builders::blobmoji::Blobmoji as EmojiBuilder>::Err>
                  >,
                  ttx_tmpl: PathBuf,
                  ttx: PathBuf) -> PyResult<()> {
        // seq_to_file: dir<codepoint sequence, file>
        //  cps = emoji.sequence (with strings instead of u32)
        //  seq = cps.filter(|cp| cp != fe0f)
        //  check cps (codepoints) if between 0 and 0x10ffff
        //  seq_to_file.add( sequence: path to corresponding image)
        // Unfortunately parallel processing is not possible due to Python
        let seq_to_file = emojis.iter()
            .filter(|(_, prepared)| prepared.is_ok())
            .map(|(emoji, prepared)| (
                // First get the sequences as a list of strings instead of u32s
                emoji.sequence.iter()
                    // In order to replicate the original behavior, we'll need to filter out fe0f
                    // variant selectors
                    // TODO: Revisit this behavior
                    .filter(|codepoint| **codepoint != 0xfe0fu32).collect_vec(),
                // Then get the file output path
                prepared.as_ref().unwrap().0.to_string_lossy().into_owned()
            ));

        // From https://pyo3.rs/master/python_from_rust.html
        let gil = Python::acquire_gil();
        let py = gil.python();


        // Prepare the modules that add_glyphs will need
        PyModule::from_code(
            py,
            Blobmoji::ADD_EMOJI_GSUB_PY,
            "add_emoji_gsub.py",
            "add_emoji_gsub"
        )?;
        PyModule::from_code(
            py,
            Blobmoji::ADD_ALIASES_PY,
            "add_aliases.py",
            "add_aliases"
        )?;
        PyModule::from_code(
            py,
            Blobmoji::PNG_PY,
            "third_party/color_emoji/png.py",
            "png"
        )?;

        let add_glyphs_module = PyModule::from_code(
            py,
            Blobmoji::ADD_GLYPHS_PY,
            "add_glyphs.py",
            "add_glyphs")?;


        // In order to use this mapping, we'll need to replace the update_ttx-function
        // This code is mostly copied from https://github.com/googlefonts/noto-emoji/blob/f8131fc45736000552cd04a8388dc414d666a829/add_glyphs.py#L353
        let aliases = match &self.aliases {
            Some(aliases) => Some(add_glyphs_module.call1(
                "add_aliases.read_emoji_aliases", (aliases.to_string_lossy().into_owned(),))?),
            None => None
        };

        let seq_to_file: Vec<(&PyTuple, &PyString)> = seq_to_file
            .map(|(sequence, filepath)|
            (PyTuple::new(py, sequence), PyString::new(py, &filepath)))
            .collect();

        let seq_to_file_dict = PyDict::from_sequence(py, seq_to_file.into_py(py))?;

        let aliases = match aliases {
            Some(aliases) => Some(add_glyphs_module.call1(
                "apply_aliases", (seq_to_file_dict, aliases)
            ).unwrap()),
            None => None
        };

        let ttx_module = PyModule::import(py, "fontTools.ttx")?;


        let font = ttx_module.call0("TTFont")?;
        // FIXME: Input file missing
        font.call_method1("importXML", (ttx_tmpl.to_string_lossy().into_owned(), ))?;

        let hhea = font.get_item("hhea")?;
        let ascent = hhea.getattr("ascent")?;
        let descent = hhea.getattr("descent")?;

        let ascent:  i32 = ascent.extract()?;
        let descent: i32 = descent.extract()?;
        let lineheight = ascent - descent;

        let map_fn = add_glyphs_module.call1(
            "get_png_file_to_advance_mapper",
            (lineheight,)
        )?;
        let seq_to_advance = add_glyphs_module.call1(
            "remap_values",
            (seq_to_file_dict, map_fn)
        )?;

        let vadvance = if font.call_method1("__contains__", ("vhea",))?.extract()? {
            font.get_item("vhea")?.getattr("advanceHeightMax")?.extract()?
        } else {
            lineheight
        };

        add_glyphs_module.call1("update_font_data", (font, seq_to_advance, vadvance, aliases))?;

        font.call_method1("saveXML", (ttx.to_string_lossy().into_owned(),))?;

        Ok(())
    }

    fn build_ttf(&self) -> PyResult<()>{
        // TODO: Do this in a venv or similar
        // TODO: Don't require fonttools
        let gil = Python::acquire_gil();
        let py = gil.python();
        let ttx_module = PyModule::import(py, "fontTools.ttx")?;

        ttx_module.call1("main", (vec![self.build_path.join(TMPL_TTX).to_string_lossy().into_owned()],))?;

        Ok(())
    }

    const EMOJI_BUILDER_PY: &'static str = include_str!("color_emoji/emoji_builder.py");
    const PNG_PY: &'static str = include_str!("color_emoji/png.py");

    fn emoji_builder(&self) -> PyResult<()> {
        // TODO: Do with PyO3
        // This is too complicated to do with PyO3
        // TODO: We need access to that file. Embedding with include_str! is probably easier
        /*let emoji_builder_path: PathBuf =
            ["noto-emoji", "third_party", "color_emoji", "emoji_builder.py"]
                .iter().collect();*/

        let tmpl_ttf = self.build_path
            .join(TMPL_TTF)
            .to_string_lossy()
            .into_owned();
        let ttf = self.build_path
            .join(TTF)
            .to_string_lossy()
            .into_owned();
        let png_dir = self.build_path
            .join(PNG_DIR)
            .join("emoji_u")
            .to_string_lossy()
            .into_owned();

        let argv = vec![
            "emoji_builder.py",
            "-S",
            "-V",
            &tmpl_ttf,
            &ttf,
            &png_dir
        ];

        let gil = Python::acquire_gil();
        let py = gil.python();

        PyModule::from_code(
            py,
            Blobmoji::PNG_PY,
            "png.py",
            "png"
        )?;

        let emoji_builder_module = PyModule::from_code(
            py,
            Blobmoji::EMOJI_BUILDER_PY,
            "emoji_builder.py",
            "emoji_builder"
        )?;

        emoji_builder_module.call1("main", (argv,))?;


        /*// TODO: Don't use a fixed python executable name
        let exit = Command::new("python3")
            .arg(emoji_builder_path)
            .arg("-S")
            .arg("-V")
            .arg(self.build_path.join(TMPL_TTF))
            .arg(self.build_path.join(TTF))
            // The directory with all the PNGs and a /emoji_u(?)
            .arg("")
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()?.wait()?;
        assert!(exit.success());*/

        Ok(())
    }

    const MAP_PUA_EMOJI_PY: &'static str = include_str!("map_pua_emoji/map_pua_emoji.py");
    // We can reuse ADD_EMOJI_GSUB_PY from add_glyphs

    fn map_pua(&self) -> PyResult<()> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        // Prepare required module(s)
        PyModule::from_code(
            py,
            Blobmoji::ADD_EMOJI_GSUB_PY,
            "add_emoji_gsub.py",
            "add_emoji_gsub"
        )?;

        let map_pua_module = PyModule::from_code(
            py,
            Blobmoji::MAP_PUA_EMOJI_PY,
            "map_pua_emoji.py",
            "map_pua_emoji"
        )?;

        map_pua_module.call1("add_pua_cmap", (
            self.build_path.join(TTF).to_string_lossy().into_owned(),
            self.build_path.join(TTF_WITH_PUA).to_string_lossy().into_owned()
            ))?;

        Ok(())
    }

    fn add_vs_cmap(&self) -> PyResult<()> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let vs_mapper = PyModule::import(py, "nototools.add_vs_cmap")?;
        //    [python3] add_vs_cmap.py -vs 2640 2642 2695 --dstdir '.' -o "<name>.ttf-with-pua-varse1" "<name>.ttf-with-pua"
        let kwargs = PyDict::new(py);
        let vs_added = HashSet::from_iter(vec![0x2640, 0x2642, 0x2695]);

        kwargs.set_item("presentation", "'emoji'")?;
        kwargs.set_item("output", format!("{}-{}", TTF_WITH_PUA, "varse1"))?;
        kwargs.set_item("dst_dir", self.build_path.to_string_lossy().into_owned())?;
        kwargs.set_item("vs_added", vs_added)?;

        vs_mapper.call_method(
            "modify_fonts",
            (vec![self.build_path.join(TTF_WITH_PUA).to_string_lossy().into_owned()],),
            Some(kwargs)
        )?;

        Ok(())
    }

    fn store_prepared(&mut self, emojis: &HashMap<&Emoji, Result<<Blobmoji as EmojiBuilder>::PreparedEmoji, <Blobmoji as EmojiBuilder>::Err>>) {
        // Collect all errors that occurred while checking the hashes and save those that were successful
        let hashing_errors = emojis.iter()
            .filter_map(|(emoji, result)| match result {
                Ok((_, hash)) => Some((emoji, hash)),
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
        if let Err(err) = saving_results {
            error!("Error in writing the new hashes: {:?}\n\
            All emojis will be re-rendered the next time.", err);
        }
    }
}


/*/// TODO: Replace this with a more intelligent and elegant solution...
/// For some reason, resvg/raqote produces a vector of u32 values, which contain the color value for
/// one pixel in the format BGRA (and not RGBA like it is required by the following tools).
/// _Note: It's actually ARGB, just reversed_
fn bgra_to_rgba(data: &mut [u8]) {
    for i in (0..data.len()).step_by(4) {
        data.swap(i, i + 2);
    }
}*/

/*#[test]
fn test_rgba() {
    let mut bgra: Vec<u8> = vec![0, 42, 17, 21, 1, 2, 3, 4, 5, 6, 7, 8];
    bgra_to_rgba(&mut bgra);
    let rgba: Vec<u8> = vec![17, 42, 0, 21, 3, 2, 1, 4, 7, 6, 5, 8];
    assert_eq!(rgba, bgra);
}*/

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