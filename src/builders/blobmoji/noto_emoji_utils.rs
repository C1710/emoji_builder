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

use std::path::{PathBuf, Path};
use std::collections::{HashMap, HashSet};
use crate::emojis::emoji::Emoji;
use crate::builders;
use crate::builder::EmojiBuilder;
use pyo3::{PyResult, Python, IntoPy};
use itertools::Itertools;
use pyo3::prelude::PyModule;
use pyo3::types::{PyTuple, PyString, PyDict};
use crate::builders::blobmoji::{TMPL_TTX, TMPL_TTF, TTF, PNG_DIR, TTF_WITH_PUA};
use std::iter::FromIterator;

const ADD_GLYPHS_PY: &str = include_str!("add_glyphs/add_glyphs.py");
const ADD_ALIASES_PY: &str = include_str!("add_glyphs/add_aliases.py");
const ADD_EMOJI_GSUB_PY: &str = include_str!("add_glyphs/add_emoji_gsub.py");

pub fn add_glyphs(aliases: &Option<PathBuf>,
                  emojis: &HashMap<&Emoji, Result<
                  <builders::blobmoji::Blobmoji as EmojiBuilder>::PreparedEmoji,
                  <builders::blobmoji::Blobmoji as EmojiBuilder>::Err>
              >,
                  ttx_tmpl: PathBuf,
                  ttx: PathBuf,
                  // From https://github.com/googlefonts/noto-emoji/blob/main/Makefile $(EMOJI_WINDOWS).tmpl.ttx: ...
                  add_cmap4_and_glyf: bool) -> PyResult<()> {
    // seq_to_file: dir<codepoint sequence, file>
    //  cps = emoji.sequence (with strings instead of u32)
    //  seq = cps.filter(|cp| cp != fe0f)
    //  check cps (codepoints) if between 0 and 0x10ffff
    //  seq_to_file.add( sequence: path to corresponding image)
    // Unfortunately parallel processing is not possible due to Python
    let seq_to_file = emojis.iter()
        .filter_map(|(emoji, prepared)| Some(emoji).zip(prepared.as_ref().ok()))
        .map(|(emoji, prepared)| (
            // First get the sequences as a list of strings instead of u32s
            emoji.sequence.iter()
                // In order to replicate the original behavior, we'll need to filter out fe0f
                // variant selectors
                // TODO: Revisit this behavior
                .filter(|codepoint| **codepoint != 0xfe0fu32).collect_vec(),
            // Then get the file output path
            prepared.0.to_string_lossy().into_owned()
        ));

    // From https://pyo3.rs/master/python_from_rust.html
    let gil = Python::acquire_gil();
    let py = gil.python();


    // Prepare the modules that add_glyphs will need
    PyModule::from_code(
        py,
        ADD_EMOJI_GSUB_PY,
        "add_emoji_gsub.py",
        "add_emoji_gsub"
    )?;
    let add_aliases = PyModule::from_code(
        py,
        ADD_ALIASES_PY,
        "add_aliases.py",
        "add_aliases"
    )?;
    PyModule::from_code(
        py,
        PNG_PY,
        "third_party/color_emoji/png.py",
        "png"
    )?;

    let add_glyphs_module = PyModule::from_code(
        py,
        ADD_GLYPHS_PY,
        "add_glyphs.py",
        "add_glyphs")?;


    // In order to use this mapping, we'll need to replace the update_ttx-function
    // This code is mostly copied from https://github.com/googlefonts/noto-emoji/blob/f8131fc45736000552cd04a8388dc414d666a829/add_glyphs.py#L353
    let aliases = match aliases {
        Some(aliases) => Some(add_aliases.call1(
            "read_emoji_aliases", (aliases.to_string_lossy().into_owned(),))?),
        None => None
    };

    let seq_to_file: Vec<(&PyTuple, &PyString)> = seq_to_file
        .map(|(sequence, filepath)|
            (PyTuple::new(py, sequence), PyString::new(py, &filepath)))
        .collect();

    let seq_to_file_dict = PyDict::from_sequence(py, seq_to_file.into_py(py))?;

    let aliases = aliases.map(|aliases| add_glyphs_module.call1(
            "apply_aliases", (seq_to_file_dict, aliases)
        ).unwrap());

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

    add_glyphs_module.call1("update_font_data", (font, seq_to_advance, vadvance, aliases, add_cmap4_and_glyf, add_cmap4_and_glyf))?;

    font.call_method1("saveXML", (ttx.to_string_lossy().into_owned(),))?;

    Ok(())
}

pub fn build_ttf(build_path: &Path) -> PyResult<()>{
    // TODO: Do this in a venv or similar
    // TODO: Don't require fonttools
    let gil = Python::acquire_gil();
    let py = gil.python();
    let ttx_module = PyModule::import(py, "fontTools.ttx")?;

    ttx_module.call1("main", (vec![build_path.join(TMPL_TTX).to_string_lossy().into_owned()],))?;

    Ok(())
}

const EMOJI_BUILDER_PY: &str = include_str!("color_emoji/emoji_builder.py");
const PNG_PY: &str = include_str!("color_emoji/png.py");

pub fn emoji_builder(build_path: &Path, keep_outlines: bool) -> PyResult<()> {
    // TODO: We need access to that file. Embedding with include_str! is probably easier
    /*let emoji_builder_path: PathBuf =
        ["noto-emoji", "third_party", "color_emoji", "emoji_builder.py"]
            .iter().collect();*/

    let tmpl_ttf = build_path
        .join(TMPL_TTF)
        .to_string_lossy()
        .into_owned();
    let ttf = build_path
        .join(TTF)
        .to_string_lossy()
        .into_owned();
    let png_dir = build_path
        .join(PNG_DIR)
        .join("emoji_u")
        .to_string_lossy()
        .into_owned();

    let mut argv = vec![
        "emoji_builder.py",
        "-S",
        "-V",
        &tmpl_ttf,
        &ttf,
        &png_dir
    ];
    if keep_outlines {
        argv.insert(2, "-O");
    }

    let gil = Python::acquire_gil();
    let py = gil.python();

    PyModule::from_code(
        py,
        PNG_PY,
        "png.py",
        "png"
    )?;

    let emoji_builder_module = PyModule::from_code(
        py,
        EMOJI_BUILDER_PY,
        "emoji_builder.py",
        "emoji_builder"
    )?;

    emoji_builder_module.call1("main", (argv,))?;

    Ok(())
}

const MAP_PUA_EMOJI_PY: &str = include_str!("map_pua_emoji/map_pua_emoji.py");
// We can reuse ADD_EMOJI_GSUB_PY from add_glyphs

pub fn map_pua(build_path: &Path) -> PyResult<()> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    // Prepare required module(s)
    PyModule::from_code(
        py,
        ADD_EMOJI_GSUB_PY,
        "add_emoji_gsub.py",
        "add_emoji_gsub"
    )?;

    let map_pua_module = PyModule::from_code(
        py,
        MAP_PUA_EMOJI_PY,
        "map_pua_emoji.py",
        "map_pua_emoji"
    )?;

    map_pua_module.call1("add_pua_cmap", (
        build_path.join(TTF).to_string_lossy().into_owned(),
        build_path.join(TTF_WITH_PUA).to_string_lossy().into_owned()
    ))?;

    Ok(())
}

pub fn add_vs_cmap(build_path: &Path) -> PyResult<()> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let vs_mapper = PyModule::import(py, "nototools.add_vs_cmap")?;
    //    [python3] add_vs_cmap.py -vs 2640 2642 2695 --dstdir '.' -o "<name>.ttf-with-pua-varse1" "<name>.ttf-with-pua"
    let kwargs = PyDict::new(py);
    let vs_added = HashSet::from_iter(vec![0x2640, 0x2642, 0x2695]);

    kwargs.set_item("presentation", "'emoji'")?;
    kwargs.set_item("output", format!("{}-{}", TTF_WITH_PUA, "varse1"))?;
    kwargs.set_item("dst_dir", build_path.to_string_lossy().into_owned())?;
    kwargs.set_item("vs_added", vs_added)?;

    vs_mapper.call_method(
        "modify_fonts",
        (vec![build_path.join(TTF_WITH_PUA).to_string_lossy().into_owned()],),
        Some(kwargs)
    )?;

    Ok(())
}