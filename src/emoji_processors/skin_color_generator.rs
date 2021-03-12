use std::collections::HashMap;
use usvg::{Tree, Paint, Color};
use crate::emoji::{Emoji, EmojiKind};
use rctree::NodeEdge;
use std::ops::DerefMut;
use usvg::NodeKind::{Path, LinearGradient, RadialGradient};
use palette::Lab;
use std::hash::{Hash, Hasher};
use palette::white_point::D65;
use itertools::Itertools;
use crate::emoji_processor::EmojiProcessor;
use clap::{ArgMatches, Arg};
use std::path::PathBuf;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use std::io::{BufReader, Error};
use std::fs::File;
use crate::emoji_processors::skin_color_generator::SkinColorMapError::NoBaseSkincolor;
use crate::deriving_emoji_processor::{DerivingEmojiProcessor, DerivedEmojis};

type Tolerance = f32;

pub struct SkinColorMap {
    base: SkinColor,
    targets: Vec<SkinColor>,
    /// The maximum allowed CIE76 color distance to allow replacing a color.
    /// According to [Wikipedia](https://en.wikipedia.org/wiki/Color_difference#CIE76), 2.3 should be a good value.
    tolerance: Tolerance
}

#[derive(Clone)]
struct ComparableColor(usvg::Color, Lab);

struct SkinColorMapping {
    mapping: HashMap<ComparableColor, usvg::Color>,
    tolerance: Tolerance
}

#[derive(Clone)]
#[derive(Deserialize)]
pub struct SkinColor {
    #[serde(alias = "extension")]
    #[serde(deserialize_with = "deserialize_sequence")]
    suffix: Option<Vec<u32>>,
    name: Option<String>,
    #[serde(deserialize_with = "deserialize_colors")]
    colors: HashMap<String, usvg::Color>
}

/// Deserializes a map from name to color string to a map from name to color
fn deserialize_colors<'de, D>(deserializer: D) -> Result<HashMap<String, usvg::Color>, D::Error>
    where D: Deserializer<'de> {
    let basic_colors: HashMap<String, String> = serde::de::Deserialize::deserialize(deserializer)?;
    Ok(basic_colors.into_iter()
        .filter_map(|(name, color)|
            if let Ok(color) = usvg::Color::from_str(&color) {
                Some((name, color))
            } else {
                None
            })
        .collect())
}

// TODO: This might even support names, if an EmojiTable is supplied
/// Deserializes a sequence in the sequence format that is also accepted for emoji file names
fn deserialize_sequence<'de, D>(deserializer: D) -> Result<Option<Vec<u32>>, D::Error>
    where D: Deserializer<'de> {
    let deserialized_string: Option<String> = serde::de::Deserialize::deserialize(deserializer)?;
    if let Some(sequence) = deserialized_string {
        // It's easier to reuse that logic.
        // The overhead that creates is acceptable as there are usually only a few
        // (less than 10) skin colors to parse and as parsing an emoji doesn't take much time
        // either and it is already done more than a thousand times anyway.
        let fake_emoji = Emoji::from_sequence(&sequence, &None);
        if let Ok(fake_emoji) = fake_emoji {
            Ok(Some(fake_emoji.sequence))
        } else {
            // FIXME: This should result in an error
            Ok(None)
        }
    } else {
        Ok(None)
    }
}


impl SkinColorMap {
    pub fn apply_all(&self, emoji: Emoji, prepared: Tree) -> Vec<(Emoji, Tree)> {
        self.targets.iter()
            .map(|target| (target.suffix.clone(), target.name.clone(), SkinColorMapping::from_skin_colors(
                self.base.clone(),
                target.clone(),
                self.tolerance)))
            // Empty suffixes/target names are not allowed as it would create conflicts
            .filter(|(suffix, target_name, _)| target_name.is_some() && suffix.is_some())
            .map(|(suffix, target_name, mapping)|
                     (
                         Self::construct_derived_emoji(&emoji, suffix.unwrap(), target_name.unwrap()),
                         mapping.applied(&prepared)
                     )
            )
            .collect_vec()
    }

    fn construct_derived_emoji(emoji: &Emoji, mut suffix: Vec<u32>, target_name: String) -> Emoji {
        let mut new_sequence = emoji.sequence.clone();
        new_sequence.append(&mut suffix);
        let new_name = if let Some(name) = &emoji.name {
            Some(format!("{} {}", name, target_name))
        } else {
            None
        };
        let new_kinds = if let Some(kinds) = &emoji.kinds {
            Some(
                // TODO: Remove the correct EmojiKinds
                kinds.iter().filter(|kind| **kind != EmojiKind::ModifierBase)
                    .cloned()
                    .collect_vec()
            )
        } else {
            None
        };
        Emoji {
            sequence: new_sequence,
            name: new_name,
            kinds: new_kinds,
            svg_path: emoji.svg_path.clone()
        }
    }

    pub fn from_files(base: PathBuf, skincolors: &[PathBuf]) -> std::io::Result<SkinColorMap> {
        // TODO: Check whether it makes sense to use par_iter
        let mut skin_colors: Vec<SkinColor> = skincolors.iter().chain(&vec![base])
            .filter_map(|path| File::open(path).ok())
            .map(BufReader::new)
            .filter_map(|reader| serde_json::from_reader(reader).ok())
            .collect_vec();
        // The last element always exists and it's always the base skin color. The rest might
        // be empty though
        let base = skin_colors.pop().unwrap();
        Ok(SkinColorMap {
            base,
            targets: skin_colors,
            // TODO: Make this configurable
            tolerance: 2.3
        })
    }

    pub fn from_directory(skincolor_directory: &PathBuf) -> Result<Self, SkinColorMapError> {
        // Build the map
        let json_files = PathBuf::from(skincolor_directory).read_dir()?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| if entry.path().is_file() {
                Some(entry.path())
            } else {
                None
            })
            .filter(|file_path| file_path.extension().is_some())
            .filter(|file_path| file_path.extension().unwrap().to_string_lossy().to_lowercase().contains("json"));
        let (base_files, skintone_files): (Vec<PathBuf>, Vec<PathBuf>) = json_files
            .filter(|file_path| file_path.file_name().is_some())
            .partition(|file_path| file_path.file_name().unwrap().to_string_lossy().to_lowercase().contains("base"));
        if base_files.len() > 1 {
            warn!("Multiple base skin color candidates found. Choosing {:?}", base_files[0]);
        }
        if base_files.is_empty() {
            error!("No base skin color found");
            return Err(NoBaseSkincolor);
        }
        Ok(SkinColorMap::from_files(base_files[0].clone(), &skintone_files)?)
    }
}

#[derive(Debug)]
pub enum SkinColorMapError {
    Io(std::io::Error),
    NoBaseSkincolor
}

impl From<std::io::Error> for SkinColorMapError {
    fn from(error: Error) -> Self {
        Self::Io(error)
    }
}

impl DerivingEmojiProcessor<usvg::Tree> for SkinColorMap {
    type DerivationTag = String;

    fn derivations(&self, emoji: &Emoji) -> Option<DerivedEmojis<Self::DerivationTag>> {
        if let Some(kinds) = &emoji.kinds {
            if kinds.contains(&EmojiKind::ModifierBase) {
                let derived: Vec<(Emoji, Self::DerivationTag)> = self.targets.iter()
                    .filter_map(|target| target.suffix.clone().zip(target.name.clone()))
                    .map(|(suffix, name)| (Self::construct_derived_emoji(emoji, suffix, name.clone()), name))
                    .collect_vec();
                let derivation = DerivedEmojis {
                    base:emoji.clone(),
                    derived
                };
                Some(derivation)
            } else {
                None
            }
        } else {
            None
        }
    }

    // TODO: Maybe use the derivations here
    fn derive(&self, derivations: DerivedEmojis<Self::DerivationTag>, prepared: Tree) -> Vec<(Emoji, Tree)> {
        self.apply_all(derivations.base, prepared)
    }
}

// FIXME: This won't work for now
impl EmojiProcessor<usvg::Tree> for SkinColorMap {
    type Err = SkinColorMapError;

    fn new(arguments: Option<ArgMatches>) -> Option<Result<Box<Self>, Self::Err>> {
        if let Some(arguments) = arguments {
            let skincolor_directory = arguments.value_of("skintone_dir");
            if let Some(skincolor_directory) = skincolor_directory {
                let skincolor_directory = PathBuf::from(skincolor_directory);
                let skincolor_map = SkinColorMap::from_directory(&skincolor_directory);
                Some(if let Ok(skincolor_map) = skincolor_map {
                    Ok(Box::new(skincolor_map))
                } else {
                    Err(skincolor_map.err().unwrap())
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn cli_arguments<'a, 'b>(builder_args: &[Arg<'a, 'b>]) -> Vec<Arg<'a, 'b>> {
        vec![Arg::with_name("skintones_dir")
            .long(if !builder_args.iter()
                .filter_map(|arg| arg.s.long)
                .filter(|long| *long == "skintones").count() > 0 {
                "skintones"
            } else {
                "skintones_dir"
            })
            .help("A directory containing mappings for the skintones")
            .takes_value(true)
            .required(false)
            .value_name("DIR")]
    }
}

impl SkinColorMapping {
    fn apply(&self, image: &mut Tree) {
        image.root().traverse().filter_map(|node_edge| match node_edge {
            NodeEdge::Start(node) => Some(node),
            _ => None
        })
            .for_each(|mut node| match node.borrow_mut().deref_mut() {
                Path(path) => {
                    if let Some(fill) = &mut path.fill {
                        if let Paint::Color(color) = fill.paint {
                            fill.paint = self.usvg_closest_paint_color_within_tolerance(color);
                        };
                    };
                    if let Some(stroke) = &mut path.stroke {
                        if let Paint::Color(color) = stroke.paint {
                            stroke.paint = self.usvg_closest_paint_color_within_tolerance(color);
                        };
                    };
                }
                LinearGradient(gradient) => (&mut gradient.base.stops).iter_mut()
                    .for_each(|stop| stop.color = self.closest_color_within_tolerance(stop.color.into()).into()),
                RadialGradient(gradient) => (&mut gradient.base.stops).iter_mut()
                    .for_each(|stop| stop.color = self.closest_color_within_tolerance(stop.color.into()).into()),
                _ => ()
            });
    }

    fn applied(&self, image: &Tree) -> Tree {
        let mut applied_image = image.clone();
        self.apply(&mut applied_image);
        applied_image
    }

    fn usvg_closest_paint_color_within_tolerance(&self, color: Color) -> Paint {
        Paint::Color(self.closest_color_within_tolerance(color.into()).into())
    }

    /// Returns the closest color found in the source colors given in this mapping or the original
    /// one if there is none.
    fn closest_color_within_tolerance(&self, old: ComparableColor) -> ComparableColor {
        if !self.mapping.is_empty() && self.mapping.get(&old).is_none()  {
            let closest = (self.mapping.keys()
                // Converting to u32 here is okay as the result is inside the u32 range
                .min_by_key(|color| cie76_color_distance(old.as_ref(), color.as_ref()) as u32)
                .unwrap()).clone();
            if cie76_color_distance(&old.as_ref(), closest.as_ref()) <= (self.tolerance * self.tolerance) {
                closest
            } else {
                old
            }
        } else {
            old
        }
    }

    fn from_skin_colors(base: SkinColor, target: SkinColor, tolerance: Tolerance) -> Self {
        let base_colors = base.colors;
        let mut target_colors = target.colors;

        let mapping: HashMap<ComparableColor, Color> = base_colors.into_iter()
            .filter_map(|(name, source_color)| {
                let target_color = target_colors.remove(&name);
                if let Some(target_color) = target_color {
                    Some((ComparableColor::from(source_color), target_color))
                } else {
                    None
                }
            })
            .collect();

        Self {
            mapping,
            tolerance
        }
    }
}


fn usvg_color_to_lab(color: &Color) -> Lab {
    palette::Srgb::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
    ).into()
}


fn lab_to_usvg_color(lab: Lab) -> Color {
    let rgb = palette::Srgb::from(lab);
    Color {
        red: (rgb.red * 255.0) as u8,
        green: (rgb.green * 255.0) as u8,
        blue: (rgb.blue * 255.0) as u8,
    }
}


/// Calculates the (or rather one) square of the CIE76 distance.
/// Source: https://en.wikipedia.org/wiki/Color_difference#CIE76
/// It seems to be rather inaccurate, but it should be good enough for this use case
/// (in the worst case, the tolerance can still be reduced).
///
/// Be aware that this is actually the square of the distance value.
fn cie76_color_distance(a: &Lab, b: &Lab) -> f32 {
    (a.l - b.l) * (a.l - b.l) +
    (a.a - b.a) * (a.a - b.a) +
    (a.b - b.b) * (a.b - b.b)
}

impl Hash for ComparableColor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (
            self.0.red,
            self.0.green,
            self.0.blue
        ).hash(state)
    }
}

impl PartialEq for ComparableColor {
    fn eq(&self, other: &Self) -> bool {
        self.0.red == other.0.red &&
        self.0.green == other.0.green &&
        self.0.blue == other.0.blue
    }
}

impl Eq for ComparableColor {}

impl From<Color> for ComparableColor {
    fn from(color: Color) -> Self {
        Self(color, usvg_color_to_lab(&color))
    }
}

impl From<ComparableColor> for Color {
    fn from(color: ComparableColor) -> Self {
        color.0
    }
}

impl From<Lab> for ComparableColor {
    fn from(color: Lab) -> Self {
        Self(lab_to_usvg_color(color), color)
    }
}

impl From<ComparableColor> for Lab {
    fn from(color: ComparableColor) -> Self {
        color.1
    }
}

impl AsRef<Lab> for ComparableColor {
    fn as_ref(&self) -> &Lab<D65, f32> {
        &self.1
    }
}

impl From<SkinColorMap> for Vec<SkinColorMapping> {
    fn from(map: SkinColorMap) -> Self {
        let base = map.base.clone();
        let tolerance = map.tolerance;
        map.targets.into_iter()
            .map(|target| SkinColorMapping::from_skin_colors(base.clone(), target, tolerance))
            .collect_vec()
    }
}