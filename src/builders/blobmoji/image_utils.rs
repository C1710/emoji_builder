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


use png::EncodingError;
use png::ColorType::Rgba;
use png::BitDepth::Eight;
use crate::builders::blobmoji::{CHARACTER_WIDTH, RENDER_AND_CHARACTER_HEIGHT, Blobmoji, PNG_DIR};
use oxipng::{PngResult, optimize_from_memory};
use oxipng::internal_tests::Headers::Safe;
use std::path::{Path, PathBuf};
use crate::emoji::Emoji;
use std::fs::File;
use std::io::Write;

pub fn pixels_to_png(img: &[u8]) -> Result<Vec<u8>, EncodingError> {
    // According to this post, PNG files have a header of 8 bytes: https://stackoverflow.com/questions/10423942/what-is-the-header-size-of-png-jpg-jpeg-bmp-gif-and-other-common-graphics-for
    let mut png_target = Vec::with_capacity(img.len() + 8);
    let mut encoder = png::Encoder::new(&mut png_target, CHARACTER_WIDTH, RENDER_AND_CHARACTER_HEIGHT);
    encoder.set_color(Rgba);
    encoder.set_depth(Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(img)?;
    // writer still borrows png_target. Fortunately we don't need it anymore
    drop(writer);
    Ok(png_target)
}


/// Runs `oxipng` on the image. It has to be encoded as PNG first
pub fn optimize_png(img: &[u8]) -> PngResult<Vec<u8>> {
    let opt = oxipng::Options {
        fix_errors: true,
        strip: Safe,
        color_type_reduction: true,
        palette_reduction: true,
        bit_depth_reduction: true,
        ..Default::default()
    };

    optimize_from_memory(img, &opt)
}


/// Saves the already encoded PNG file
pub fn write_png(build_path: &Path, emoji: &Emoji, image: Vec<u8>) -> std::io::Result<()> {
    let filename = Blobmoji::generate_filename(emoji);
    let path = build_path
        .join(PNG_DIR)
        .join(&PathBuf::from(filename));
    let mut file = File::create(path)?;
    file.write_all(&image)
}


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

    assert_eq!(image.len(), 4 * (target_width as usize * target_height as usize));

    image
}


pub fn enlarge_to(
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
    let enlarged = enlarge_by(content, src_width, src_height, d_width, d_height);

    assert_eq!(enlarged.len(), 4 * target_width as usize * target_height as usize);

    enlarged
}
