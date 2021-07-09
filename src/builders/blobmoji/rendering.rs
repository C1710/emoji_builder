/*
 * Copyright 2021 Constantin A. <emoji.builder@c1710.de>
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
 *
 */

use tiny_skia::Pixmap;
use usvg::FitTo;

use crate::builders::blobmoji::{RENDER_AND_CHARACTER_HEIGHT, RENDER_WIDTH, WAVE_FACTOR};
use crate::emoji_processor::EmojiProcessor;
use crate::emoji_processors::reduce_colors::ReduceColors;
use crate::emojis::emoji::Emoji;

/// Renders a single emoji.
/// It will not pad the image, however it will return whether it is taller than wide
/// (`FitTo::Height`) or if it's wider than tall (`FitTo::Width`).
/// The exact value is always 128px (i.e. the target size for the largest dimension).
/// # Arguments
/// * `emoji` - the emoji to be rendered
/// # Returns
/// An `Option` containing the image as a vector of RGBA pixels and the dimensions of the
/// image.
pub fn render_svg(default_font: String,
              fontdb: usvg::fontdb::Database,
              reduce_colors: Option<&Box<ReduceColors>>,
              waveflag: bool,
              emoji: &Emoji) -> Option<(Pixmap, (u32, u32))> {
    if let Some(svg_path) = &emoji.svg_path {
        let opt = usvg::Options {
            // Just as a fallback. Default is "cursive",
            // which on Windows and Mac OS it will use Comic Sans
            // which is pretty close to Comic Neue, that is used in Blobmoji
            font_family: default_font,
            fontdb: fontdb,
            ..Default::default()
        };

        let data = std::fs::read(svg_path).ok()?;
        let tree = usvg::Tree::from_data(&data, &opt);

        if let Ok(tree) = tree {
            // Reduce the colors to a certain palette if possible
            let tree = if let Some(reduce_colors) = reduce_colors {
                match reduce_colors.process(emoji, tree) {
                    Ok(tree) => tree,
                    Err((tree, err)) => {
                        error!("Could not reduce colors on emoji {}: {:?}", &emoji, err);
                        tree
                    }
                }
            } else {
                tree
            };

            // It's easier to get the dimensions here than at some later point
            let size = tree.svg_node().size;

            let waved_height = if emoji.is_flag() && waveflag {
                size.height() * (1.0 + WAVE_FACTOR as f64)
            } else {
                size.height()
            };

            let fit_to = if waved_height > size.width() {
                if emoji.is_flag() && waveflag {
                    FitTo::Height((RENDER_AND_CHARACTER_HEIGHT as f32 / (1.0 + WAVE_FACTOR)) as u32)
                } else {
                    FitTo::Height(RENDER_AND_CHARACTER_HEIGHT)
                }
            } else {
                FitTo::Width(RENDER_WIDTH)
            };

            // Now, how large will it get?
            // This is now done in the same way as the rendering
            let rendered_size = fit_to.fit_to(size.to_screen_size()).unwrap();

            // This is copied from the minimal example for resvg
            let mut pixmap = tiny_skia::Pixmap::new(rendered_size.width(), rendered_size.height()).unwrap();

            // This is the point where it's actually rendered
            let img = resvg::render(&tree, fit_to, pixmap.as_mut());

            if img.is_some() {
                Some((pixmap, rendered_size.dimensions()))
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

