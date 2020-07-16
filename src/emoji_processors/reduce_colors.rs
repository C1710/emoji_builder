/*
 * Copyright 2020 Constantin A. <emoji.builder@c1710.de>
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

use std::fmt::{Debug, Formatter};
use std::ops::DerefMut;

use clap::{Arg, ArgMatches};
use gimp_palette::{NewPaletteError, Palette};
use itertools::Itertools;
use rctree::NodeEdge;
use usvg::{Color, Paint, Tree};
use usvg::NodeKind::{LinearGradient, Path, RadialGradient};

use crate::emoji::Emoji;
use crate::emoji_processor::EmojiProcessor;

pub struct ReduceColors {
    palette: Vec<Color>
}

pub struct PaletteError(gimp_palette::NewPaletteError);

impl EmojiProcessor<usvg::Tree> for ReduceColors {
    type Err = PaletteError;

    fn new(arguments: Option<ArgMatches>) -> Option<Result<Box<Self>, Self::Err>> {
        if let Some(matches) = arguments {
            if let Some(palette_file) = matches.value_of("reduce_to_palette") {
                match gimp_palette::Palette::read_from_file(palette_file) {
                    Ok(palette) => Some(Ok(Box::new(palette.into()))),
                    Err(e) => Some(Err(PaletteError(e)))
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn process(&self, _emoji: &Emoji, prepared: Tree) -> Result<Tree, (Tree, Self::Err)> {
        prepared.root().traverse().filter_map(|node_edge| match node_edge {
            NodeEdge::Start(node) => Some(node),
            _ => None
        })
            .for_each(|mut node| match node.borrow_mut().deref_mut() {
                Path(path) => {
                    if let Some(fill) = &mut path.fill {
                        if let Paint::Color(color) = fill.paint {
                            fill.paint = Paint::Color(self.closest_color(color))
                        };
                    };
                    if let Some(stroke) = &mut path.stroke {
                        if let Paint::Color(color) = stroke.paint {
                            stroke.paint = Paint::Color(self.closest_color(color))
                        };
                    };
                }
                LinearGradient(gradient) => (&mut gradient.base.stops).iter_mut()
                    .for_each(|stop| stop.color = self.closest_color(stop.color)),
                RadialGradient(gradient) => (&mut gradient.base.stops).iter_mut()
                    .for_each(|stop| stop.color = self.closest_color(stop.color)),
                _ => ()
            });
        Ok(prepared)
    }

    fn cli_arguments<'a, 'b>(builder_args: &[Arg<'a, 'b>]) -> Vec<Arg<'a, 'b>> {
        let short_exists = builder_args.iter()
            .filter_map(|arg| arg.s.short)
            .filter(|short| *short == 'p').count() > 0;
        let name_exists = builder_args.iter()
            .filter_map(|arg| arg.s.long)
            .filter(|long| *long == "palette")
            .count() > 0;


        let mut input_file_arg =
            Arg::with_name("reduce_to_palette")
                .long(if name_exists {
                    "reduce_to_palette"
                } else {
                    "palette"
                })
                .required(false)
                .takes_value(true)
                .help("A Color palette in GIMP's format to reduce the colors to")
                .value_name("FILE");
        if short_exists {
            input_file_arg = input_file_arg.short("p");
        }

        vec![input_file_arg]
    }
}

impl ReduceColors {
    fn closest_color(&self, old: Color) -> Color {
        if !self.palette.is_empty() && !self.palette.contains(&old) {
            *self.palette.iter()
                .min_by_key(|color| color_distance(&old, color))
                .unwrap()
        } else {
            old
        }
    }
}

/// Currently calculates the Euclidean distance between two RGB color vectors (not very good)
fn color_distance(a: &Color, b: &Color) -> u16 {
    ((
        ((a.red + b.red) as f64).powf(2.0) +
            ((a.green + b.green) as f64).powf(2.0) +
            ((a.blue + b.blue) as f64).powf(2.0)
    ).sqrt()) as u16
}

impl From<Vec<Color>> for ReduceColors {
    fn from(palette: Vec<Color>) -> Self {
        Self {
            palette
        }
    }
}

impl From<gimp_palette::Palette> for ReduceColors {
    fn from(palette: Palette) -> Self {
        palette.get_colors()
            .iter()
            .map(|color| Color::new(color.r, color.g, color.b))
            .collect_vec()
            .into()
    }
}

impl Debug for PaletteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self.0 {
            NewPaletteError::NoColors => f.write_str("No colors found in palette"),
            NewPaletteError::InvalidData { line_num, val } => f.debug_map().entry(&line_num, &val).finish(),
            NewPaletteError::IoErr(err) => err.fmt(f),
        }
    }
}