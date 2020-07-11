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
use std::ptr::slice_from_raw_parts_mut;


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
    let mut content = enlarge_by(content, width, added_lines);

    let rgba_width = width * 4;
    let content_ptr = content.as_mut_ptr();

    // Go over all the pixels of the actual image plus that one empty line at the end
    ((added_lines * rgba_width)..content.len())
        .step_by(4)
        // Position is the location of the red subpixel (or whatever comes first)
        .for_each(|position| {
            // Get the x coordinate
            let x = (position % rgba_width) >> 2;
            // The first line is reserved for antialiasing, so the wave amplitude will be a tiny bit smaller
            let (floor_offset, ceil_offset, opacity) = offsets(x, width, added_lines-1);

            // This is the position of the red subpixel where the red subpixel at position will be moved to
            let target = position - floor_offset;
            // This is one line above that one (this is where the antialiasing will happen)
            let aa_target = position - ceil_offset;
            /*
               The antialiasing works as follows: The offset-function calculates a floating point
               offset, e.g. the pixels at x-position 42 are supposed to be moved upward by 4.2
               pixels.
               As you can easily see, that's not possible. The approach is to fully overwrite
               the pixel that's 4 pixels above the source pixel and blend it in with the pixel above
               that target-pixel with a opacity of 0.2.
               As this doesn't do anything with transparent pixels (if you add a transparent layer
               to a regular image, it won't change anything either), the alpha channel of both
               pixels will be mixed, i.e. we'll multiply the source's alpha channel with 0.2 and the
               antialiasing-target-pixel's alpha channel with 0.8 and add them together.
            */
            // This is the resulting alpha value after applying the opacity.
            let source_alpha = (content[position + 3] as f32 / 255.0) * opacity;
            // This is the alpha value of the antialiasing-target-pixel
            let aa_target_alpha = content[aa_target + 3] as f32 / 255.0;

            unsafe {
                // We'll need to use raw pointers here as the borrow checker would otherwise cause
                // problems.
                // The maximum offset from one of those points that is used is 3.
                // However, it is safe, for the following reasons:
                // 1. aa_target is one line above target,
                //    so they have more than enough space between them.
                // 2. If target = position (which is possible), we'll end up with something like
                //    content[position + x] = content[position + x], which is pointless, but safe.
                // 3. As the maximum offset is added_lines - 1 and therefore the vertical
                //    offset for antialiasing is added_lines, the earliest address we might access is
                //    content[position-added_lines*rgba_width]. This is always in content, as we
                //    position >= added_lines * rgba_width
                // 4. we can assume that the length of content is divisible by 4.
                //    As step_by always includes the first element, the last value position can have
                //    is content.len()-4 (as the range excludes the value of content.len()).
                //    Therefore we can access these three bytes safely.
                let source_rgb = slice_from_raw_parts_mut(
                    content_ptr.add(position), 3);
                let target_rgb = slice_from_raw_parts_mut(
                    content_ptr.add(target), 3);
                let aa_target_rgb = slice_from_raw_parts_mut(
                    content_ptr.add(aa_target), 3);

                // By using zip, we get a (kind of) triple of the source, the target and the anti-
                // aliasing-target-subpixels.
                // We'll only iterate over the RGB-parts as the alpha value needs to be treated
                // differently.
                source_rgb.as_mut().unwrap().iter_mut()
                    .zip(target_rgb.as_mut().unwrap())
                    .zip(aa_target_rgb.as_mut().unwrap())

                    .for_each(|((source_subpx, target_subpx), aa_target_subpx)| {
                        // Simply copy the value to the target pixel. It will be modified by the
                        // next line that's processed
                        *target_subpx = *source_subpx;
                        // from https://stackoverflow.com/a/1944193, modified
                        *aa_target_subpx = (
                            // Blend the target pixel
                            (*aa_target_subpx as f32 * aa_target_alpha) +
                            // Blend the source pixel (with the appropriate opacity)
                            (*source_subpx as f32 * source_alpha * (1.0 - aa_target_alpha))) as u8;
                    });

                // Simply copy the source opacity to the target position
                content[target + 3] = content[position + 3];
                // The opacity of the antialiasing-pixel will be a mixture of its original opacity
                // and the new one, with the mixing factor determined by the offset.
                content[aa_target + 3] = ((source_alpha + aa_target_alpha * (1.0 - opacity)) * 255.0) as u8;
                // Make the old pixel invisible if it has been moved
                // (The color will probably be erased at optimization)
                // (As we're working on the old data block, the old pixel information for the last
                //  few lines is still there and needs to be erased. Erasing all of them seems easier)
                if floor_offset > 0 {
                    content[position + 3] = 0;
                }
            }
        });
    // Remove the last line that was used only for antialiasing
    content.truncate(rgba_width * (height as usize + added_lines));
    (content, width as u32, height + added_lines as u32)
}

/// Returns `(offset(...).floor(), offset(...).floor() + 1, offset(...).fract())`,
/// with the first two values multiplied by the line width.
/// Simply used for some precomputations.
/// Unfortunately, caching doesn't seem to cause any benefits here, but it can be easily applied.
#[inline]
fn offsets(x_position: usize, width: usize, max_offset: usize) -> (usize, usize, f32) {
    let rgba_width = width * 4;
    let offset = offset(x_position, width, max_offset);
    let floor = offset.floor() as usize * rgba_width;
    (floor, floor + rgba_width, offset.fract())
}

/// Taking _wave_flag seriously, we'll actually use a sinus function here.
/// the wavelength is the width of the flag,
/// the amplitude is the maximum offset/2, however we don't want to move the flag down,
/// so the amplitude will be added to the result, such that we only get positive values.
/// You can find a picture of the function on [Github](https://github.com/C1710/emoji_builder/issues/2#issue-655167863).
#[inline]
fn offset(x_position: usize, width: usize, max_offset: usize) -> f32 {
    // Some quick tests showed that using 64 Bit seems to be a bit faster.
    let x_position = x_position as f64;
    let width = width as f64;
    let max_offset = max_offset as f64;

    let wavelength = 2.0 * std::f64::consts::PI / width;
    let amplitude = max_offset / 2.0;
    let x = x_position * wavelength;
    let offset = x.sin() * amplitude + amplitude;
    // Just to make sure that we won't go out of bounds
    offset.min(max_offset) as f32
}

/// Adds a vertical padding of added_lines before the image and one line after it
fn enlarge_by(content: &[u8], width: usize, added_lines: usize) -> Vec<u8> {
    let mut new_content = Vec::with_capacity(
        added_lines * width * 4 + content.len()
    );
    new_content.append(&mut vec![0u8; added_lines * width * 4]);
    new_content.extend_from_slice(content);
    // One extra line for antialiasing, will be removed later
    new_content.append(&mut vec![0u8; width * 4]);
    new_content
}