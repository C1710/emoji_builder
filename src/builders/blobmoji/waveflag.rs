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
use itertools::Itertools;

/// Adds a wavy style (a sinus based displacement) to a flag emoji.
/// # Arguments
/// * `content`: The pixels of the image (in RGBA or BGRA format)
/// * `width`, `height`: The dimensions of the image
/// * `added_lines` the height that the wave should have plus 1 (one line will be reserved for antialiasing).
/// # Returns
/// * The resulting pixels (same color format as the input)
/// * The resulting width (stays the same as the input)
/// * The resulting height (`height` + `added_lines`)
pub fn waveflag(content: &[u8], width: usize, height: u32, added_lines: usize) -> (Vec<u8>, u32, u32) {
    // First of all, add a padding for the wave
    let mut content = enlarge_by(content, width, added_lines + 1);
    let rgba_width = width * 4;

    let content_ptr = content.as_mut_ptr();

    // The first line is reserved for antialiasing, so the wave amplitude will be a tiny bit smaller
    let offsets = (0..width).map(|x| offsets(x, width, added_lines - 1)).collect_vec();

    // Go over all pixel positions with their offset
    (0..width).map(|x| (x, offsets[x]))
        .cartesian_product(0..(height as usize + added_lines))
        // Calculate the pixel's coordinates (not accounting for the subpixels)
        .map(|((x, (floor_offset, opacity)), y)|
            //  current pixel, aa_source,        source
            (x, y, y + floor_offset, y + floor_offset + 1, opacity))
        // Now get the actual positions in the image vector (i.e. including all subpixels)
        .map(|(x, target_y, aa_source_y, source_y, opacity)|
            (target_y * rgba_width + x * 4,
             aa_source_y * rgba_width + x * 4,
             source_y * rgba_width + x * 4,
             opacity))
        // Calculate and assign the pixel's new value
        .for_each(|(target, aa_source, source, opacity)| {
            blend(
                &content[aa_source..aa_source + 4],
                &content[source..source + 4],
                unsafe { content_ptr.add(target) },
                opacity);
        });

    // Remove the last line that was used only for antialiasing
    content.truncate(rgba_width * (height as usize + added_lines));
    assert_eq!(content.len() as u32, (width as u32 * (height + added_lines as u32)) * 4);
    (content, width as u32, height + added_lines as u32)
}

/// A simple function that mixes two RGBA pixels with a given factor and writes them to a third one.
/// # How does it work?
/// The antialiasing works as follows: The offset-function calculates a floating point
/// offset, e.g. the pixels are supposed to be moved upward by 4.2 pixels.
/// As you can easily see, that's not possible. The approach is to fully overwrite
/// the pixel that's 4 pixels above the source pixel and mix it with the pixel above
/// that target-pixel with an opacity of 0.2 (although this function is written from the
/// target-pixel's "perspective").
/// It's different to the "normal" blend mode found in image editors as it doesn't account for the
/// alpha values of the two pixels when mixing their colors. This makes the function much easier
/// and faster, with the cost of mixing in black when mixing with completely transparent pixels
/// from the padding (or from the source picture) which have their red, green and blue channels set
/// to 0 (which is black).
#[inline]
fn blend(
    px_a: &[u8],
    px_s: &[u8],
    px_o: *mut u8,
    opacity: f64,
) {
    unsafe {
        *px_o.add(0) = (px_s[0] as f64 * opacity + px_a[0] as f64 * (1.0 - opacity)) as u8;
        *px_o.add(1) = (px_s[1] as f64 * opacity + px_a[1] as f64 * (1.0 - opacity)) as u8;
        *px_o.add(2) = (px_s[2] as f64 * opacity + px_a[2] as f64 * (1.0 - opacity)) as u8;
        *px_o.add(3) = (px_s[3] as f64 * opacity + px_a[3] as f64 * (1.0 - opacity)) as u8;
    }
}

/// Returns `(offset(...).floor(), offset(...).floor() + 1, offset(...).fract())`,
/// with the first two values multiplied by the line width.
/// Simply used for some precomputations.
/// Unfortunately, caching doesn't seem to cause any benefits here, but it can be easily applied.
#[inline]
fn offsets(x_position: usize, width: usize, max_offset: usize) -> (usize, f64) {
    let offset = offset(x_position, width, max_offset);
    let floor = offset.floor() as usize;
    (floor, offset.fract())
}

/// Taking _wave_flag seriously, we'll actually use a sinus function here.
/// This function may be rather expensive as it's run `width` times per flag.
/// the wavelength is the width of the flag,
/// the amplitude is the maximum offset/2, however we don't want to move the flag down,
/// so the amplitude will be added to the result, such that we only get positive values.
/// You can find a picture of the function on [Github](https://github.com/C1710/emoji_builder/issues/2#issue-655167863).
#[inline]
fn offset(x_position: usize, width: usize, max_offset: usize) -> f64 {
    // Some quick tests showed that using 64 Bit seems to be a bit faster.
    let x_position = x_position as f64;
    let width = width as f64;
    let max_offset = max_offset as f64;

    let wavelength = 2.0 * std::f64::consts::PI / width;
    let amplitude = max_offset / 2.0;
    let x = x_position * wavelength;
    let offset = x.sin() * amplitude + amplitude;
    // Just to make sure that we won't go out of bounds
    offset.min(max_offset)
}

/// Adds a vertical padding of `added_lines-1` lines before and `added_lines+1` lines after the image.
fn enlarge_by(content: &[u8], width: usize, added_lines: usize) -> Vec<u8> {
    let rgba_width = width * 4;
    let mut new_content = Vec::with_capacity(
        2 * added_lines * rgba_width + content.len()
    );
    new_content.append(&mut vec![0u8; (added_lines - 1) * rgba_width]);
    new_content.extend_from_slice(content);
    // One extra line for antialiasing, will be removed later
    new_content.append(&mut vec![0u8; (added_lines + 1) * rgba_width]);
    new_content
}