/*
 * MIT License
 *
 * Copyright (c) 2005 Mozilla Foundation
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use qcms::{Profile, Transform};
use std::ptr::null_mut;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub enum DataType {
    RGB8 = 0,
    RGBA8 = 1,
    BGRA8 = 2,
    Gray8 = 3,
    GrayA8 = 4,
    CMYK = 5,
}

#[wasm_bindgen]
pub enum Intent {
    Perceptual = 0,
    RelativeColorimetric = 1,
    Saturation = 2,
    AbsoluteColorimetric = 3,
}

impl DataType {
    fn bytes_per_pixel(&self) -> usize {
        match self {
            DataType::Gray8 => 1,
            DataType::GrayA8 => 2,
            DataType::RGB8 => 3,
            DataType::RGBA8 | DataType::BGRA8 | DataType::CMYK => 4,
        }
    }
}

fn to_datatype(datatype: &DataType) -> qcms::DataType {
    match datatype {
        DataType::RGB8 => qcms::DataType::RGB8,
        DataType::RGBA8 => qcms::DataType::RGBA8,
        DataType::BGRA8 => qcms::DataType::BGRA8,
        DataType::Gray8 => qcms::DataType::Gray8,
        DataType::GrayA8 => qcms::DataType::GrayA8,
        DataType::CMYK => qcms::DataType::CMYK,
    }
}

fn to_intent(intent: Intent) -> qcms::Intent {
    match intent {
        Intent::Perceptual => qcms::Intent::Perceptual,
        Intent::RelativeColorimetric => qcms::Intent::RelativeColorimetric,
        Intent::Saturation => qcms::Intent::Saturation,
        Intent::AbsoluteColorimetric => qcms::Intent::AbsoluteColorimetric,
    }
}

#[wasm_bindgen(raw_module = "./qcms_utils.js")]
extern "C" {
    fn copy_result(ptr: *const u8, len: usize);
}

// The destination is always sRGB, and building its precache tables is most of
// the cost of creating a transform, so keep one around. Wasm is single-threaded,
// hence the `thread_local` rather than a lock.
thread_local! {
    static SRGB: Box<Profile> = {
        let mut profile = Profile::new_sRGB();
        profile.precache_output_transform();
        profile
    };
}

/// How many pixels are converted at a time. Small enough that the run-length
/// scratch buffers stay in L1 while the transform runs, big enough that the
/// per-chunk bookkeeping disappears.
const CHUNK: usize = 1024;

/// After a chunk that did not compact well, this many chunks are converted
/// without trying, so that a photographic image pays for the run-length pass
/// once every 32 chunks instead of on all of them.
const SKIP_AFTER_MISS: u32 = 31;

/// Compaction is abandoned for a while when fewer than this fraction of a
/// chunk's pixels repeated the one before them.
///
/// These are measured break-even points, not estimates: sweeping the repeat
/// ratio, compaction starts paying at 5% of pixels for CMYK and at 40% for the
/// matrix/TRC RGB path, which is five times cheaper per pixel and so needs far
/// more repeats to earn the scan back. Setting the gate anywhere below those
/// leaves a band in which compaction is switched on and loses -- an input sitting
/// exactly on a gate of 1/32 and 1/4 measured 0.90x and 0.86x against converting
/// straight through.
fn min_repeat_ratio(bytes_per_pixel: usize) -> (usize, usize) {
    if bytes_per_pixel == 4 {
        (1, 16)
    } else {
        (2, 5)
    }
}

/// Scratch space for the run-length path: the distinct pixels of one chunk,
/// their converted colors, and how many times each one repeats.
struct Scratch {
    uniq: Vec<u8>,
    rgb: Vec<u8>,
    runs: Vec<u16>,
}

impl Scratch {
    fn new() -> Self {
        Self {
            uniq: vec![0; CHUNK * 4],
            rgb: vec![0; CHUNK * 3],
            runs: vec![0; CHUNK],
        }
    }
}

pub struct Transformer {
    transform: Transform,
    bytes_per_pixel: usize,
    scratch: Scratch,
    skip_compaction: u32,
    /// Single-component input has only 256 possible pixels, so the whole
    /// transform collapses into a table. Alpha is pre-filled, so an RGBA
    /// destination is one 4-byte store per pixel.
    gray_lut: Option<Box<[[u8; 4]; 256]>>,
}

/// Collapses runs of identical pixels into `uniq`, recording each one's length
/// in `runs`. Returns how many distinct pixels were written.
///
/// This is exact, not an approximation: every qcms transform maps a pixel to a
/// color independently of its neighbours, so converting one pixel of a run and
/// repeating the result is the same as converting all of them. Images that come
/// out of a PDF are full of such runs -- flat fills, white margins, screened
/// artwork -- and a CMYK pixel costs about 30ns to convert but 1ns to compare.
///
/// Everything here is written in terms of `[u8; N]` rather than slices on
/// purpose: a slice copy of a length LLVM cannot see through becomes a call to
/// `memcpy`, which for three or four bytes costs more than the comparison it is
/// meant to serve.
fn compact<const BPP: usize>(src: &[u8], uniq: &mut [u8], runs: &mut [u16]) -> usize {
    let (src, _) = src.as_chunks::<BPP>();
    let (uniq, _) = uniq.as_chunks_mut::<BPP>();
    let Some((&first, rest)) = src.split_first() else {
        return 0;
    };
    uniq[0] = first;
    runs[0] = 1;
    let mut count = 1;
    let mut prev = first;
    for &pixel in rest {
        if pixel == prev {
            runs[count - 1] += 1;
            continue;
        }
        uniq[count] = pixel;
        runs[count] = 1;
        prev = pixel;
        count += 1;
    }
    count
}

/// Writes `dest` from `rgb`, repeating each color `runs` times. `OUT` is the
/// number of bytes per destination pixel: 3 for RGB, or 4 for RGBA, in which
/// case the alpha byte is set to 255.
fn expand<const OUT: usize>(rgb: &[u8], runs: &[u16], dest: &mut [u8]) {
    let (rgb, _) = rgb.as_chunks::<3>();
    let (dest, _) = dest.as_chunks_mut::<OUT>();
    let mut at = 0;
    for (pixel, &run) in rgb.iter().zip(runs) {
        let mut out = [255u8; OUT];
        out[0] = pixel[0];
        out[1] = pixel[1];
        out[2] = pixel[2];
        let run = run as usize;
        if run == 1 {
            dest[at] = out;
        } else {
            dest[at..at + run].fill(out);
        }
        at += run;
    }
}

/// Widens packed RGB to RGBA, for the chunks that were converted without
/// compaction.
fn widen(rgb: &[u8], dest: &mut [u8]) {
    let (rgb, _) = rgb.as_chunks::<3>();
    let (dest, _) = dest.as_chunks_mut::<4>();
    for (pixel, out) in rgb.iter().zip(dest) {
        *out = [pixel[0], pixel[1], pixel[2], 255];
    }
}

/// The single-component path: one table lookup per pixel.
fn lookup<const OUT: usize>(lut: &[[u8; 4]; 256], src: &[u8], dest: &mut [u8]) {
    let (dest, _) = dest.as_chunks_mut::<OUT>();
    for (&value, out) in src.iter().zip(dest) {
        let pixel = lut[value as usize];
        let mut converted = [255u8; OUT];
        converted[0] = pixel[0];
        converted[1] = pixel[1];
        converted[2] = pixel[2];
        *out = converted;
    }
}

impl Transformer {
    fn new(in_profile: &[u8], in_type: DataType, intent: Intent) -> Option<Self> {
        let in_profile = Profile::new_from_slice(in_profile, false)?;
        let transform = SRGB.with(|out_profile| {
            Transform::new_to(
                &in_profile,
                out_profile,
                to_datatype(&in_type),
                qcms::DataType::RGB8,
                to_intent(intent),
            )
        })?;
        let bytes_per_pixel = in_type.bytes_per_pixel();
        let gray_lut = (bytes_per_pixel == 1).then(|| {
            let mut gray = [0u8; 256];
            for (value, slot) in gray.iter_mut().enumerate() {
                *slot = value as u8;
            }
            let mut rgb = [0u8; 256 * 3];
            transform.convert(&gray, &mut rgb);
            let mut lut = Box::new([[255u8; 4]; 256]);
            for (pixel, out) in rgb.chunks_exact(3).zip(lut.iter_mut()) {
                out[..3].copy_from_slice(pixel);
            }
            lut
        });
        Some(Self {
            transform,
            bytes_per_pixel,
            scratch: Scratch::new(),
            skip_compaction: 0,
            gray_lut,
        })
    }

    fn convert<const BPP: usize>(&mut self, src: &[u8], dest: &mut [u8], out_bpp: usize) {
        if let Some(lut) = &self.gray_lut {
            if out_bpp == 3 {
                lookup::<3>(lut, src, dest);
            } else {
                lookup::<4>(lut, src, dest);
            }
            return;
        }
        // Reset per call, so that what one buffer costs does not depend on the
        // ones converted before it.
        self.skip_compaction = 0;
        let Self {
            transform,
            scratch,
            skip_compaction,
            ..
        } = self;
        let chunks = src
            .chunks(CHUNK * BPP)
            .zip(dest.chunks_mut(CHUNK * out_bpp));
        for (src, dest) in chunks {
            let pixels = src.len() / BPP;
            if *skip_compaction > 0 {
                *skip_compaction -= 1;
                if out_bpp == 3 {
                    transform.convert(src, dest);
                } else {
                    let rgb = &mut scratch.rgb[..pixels * 3];
                    transform.convert(src, rgb);
                    widen(rgb, dest);
                }
                continue;
            }
            let count = compact::<BPP>(src, &mut scratch.uniq, &mut scratch.runs);
            let (numer, denom) = min_repeat_ratio(BPP);
            if (pixels - count) * denom < pixels * numer {
                // Too few repeats to pay for the scan; stop trying for a while.
                *skip_compaction = SKIP_AFTER_MISS;
            }
            let rgb = &mut scratch.rgb[..count * 3];
            transform.convert(&scratch.uniq[..count * BPP], rgb);
            let runs = &scratch.runs[..count];
            if out_bpp == 3 {
                expand::<3>(rgb, runs, dest);
            } else {
                expand::<4>(rgb, runs, dest);
            }
        }
    }
}

#[wasm_bindgen]
/// Converts `src` and hands the result to `copy_result`, laid out as RGB, or as
/// RGBA with an opaque alpha when `add_alpha` is set.
///
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_array(transformer: *mut Transformer, src: &[u8], add_alpha: bool) {
    let transformer = &mut *transformer;
    let bpp = transformer.bytes_per_pixel;
    // A short final pixel is dropped rather than converted from whatever
    // follows it in memory.
    let pixels = src.len() / bpp;
    let src = &src[..pixels * bpp];
    let out_bpp = if add_alpha { 4 } else { 3 };
    let len = pixels * out_bpp;

    // Overaligned so that the JS side can always merge whole pixels at a time.
    let mut dest: Vec<u64> = vec![0; len.div_ceil(8)];
    let dest_bytes = unsafe { std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, len) };

    match bpp {
        1 => transformer.convert::<1>(src, dest_bytes, out_bpp),
        2 => transformer.convert::<2>(src, dest_bytes, out_bpp),
        3 => transformer.convert::<3>(src, dest_bytes, out_bpp),
        _ => transformer.convert::<4>(src, dest_bytes, out_bpp),
    }
    copy_result(dest_bytes.as_ptr(), len);
}

/// The three scalar entry points return `0x00RRGGBB`, which is cheaper than
/// calling back into JS and lets the caller pick between a buffer write and a
/// CSS color without a second code path in here.
fn convert_one(transform: &Transform, src: &[u8]) -> u32 {
    let mut dest = [0u8; 3];
    transform.convert(src, &mut dest);
    u32::from(dest[0]) << 16 | u32::from(dest[1]) << 8 | u32::from(dest[2])
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_one(transformer: *const Transformer, src: u8) -> u32 {
    convert_one(&(*transformer).transform, &[src])
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_three(
    transformer: *const Transformer,
    src1: u8,
    src2: u8,
    src3: u8,
) -> u32 {
    convert_one(&(*transformer).transform, &[src1, src2, src3])
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_four(
    transformer: *const Transformer,
    src1: u8,
    src2: u8,
    src3: u8,
    src4: u8,
) -> u32 {
    convert_one(&(*transformer).transform, &[src1, src2, src3, src4])
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_transformer_from_memory(
    mem: &[u8],
    in_type: DataType,
    intent: Intent,
) -> *mut Transformer {
    Transformer::new(mem, in_type, intent).map_or_else(null_mut, |t| Box::into_raw(Box::new(t)))
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_drop_transformer(transformer: *mut Transformer) {
    drop(Box::from_raw(transformer));
}
