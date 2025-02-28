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
use std::{ptr::null_mut, vec};
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
    fn copy_rgb(ptr: *const u8);
}

pub struct Transformer {
    src1: [u8; 1],
    src3: [u8; 3],
    src4: [u8; 4],
    dest: [u8; 3],
    transform: Transform,
    in_type: DataType,
}

impl Transformer {
    fn new(
        in_profile: &[u8],
        in_type: DataType,
        intent: Intent,
    ) -> Option<Self> {
        let out_profile = Profile::new_sRGB();
        let in_profile = Profile::new_from_slice(in_profile, false)?;
        let transform = Transform::new_to(
            &in_profile,
            &out_profile,
            to_datatype(&in_type),
            qcms::DataType::RGB8,
            to_intent(intent),
        )?;
        Some(Self {
            src1: [0; 1],
            src3: [0; 3],
            src4: [0; 4],
            dest: [0; 3],
            transform,
            in_type,
        })
    }
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_array(
    transformer: *const Transformer,
    src: Vec<u8>
) {
    let transformer = &*transformer;
    let len = src.len();
    let output_len = match transformer.in_type {
        DataType::RGB8 => len,
        DataType::RGBA8 | DataType::BGRA8 => 3 * len / 4,
        DataType::Gray8 => 3 * len,
        DataType::GrayA8 => 3 * len / 2,
        DataType::CMYK => 3 * len / 4,
    };
    let mut dest = vec![0; output_len];
    transformer.transform.convert(&src, &mut dest);
    copy_result(dest.as_ptr(), dest.len());
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_one(transformer: *mut Transformer, src: u8) {
    let transformer = &mut *transformer;
    transformer.src1[0] = src;
    let dest = &mut transformer.dest;
    transformer
        .transform
        .convert(&transformer.src1, dest);
    copy_rgb(dest.as_ptr());
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_three(transformer: *mut Transformer, src1: u8, src2: u8, src3: u8) {
    let transformer = &mut *transformer;
    transformer.src3[0] = src1;
    transformer.src3[1] = src2;
    transformer.src3[2] = src3;
    let dest = &mut transformer.dest;
    transformer
        .transform
        .convert(&transformer.src3, dest);
    copy_rgb(dest.as_ptr());
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_convert_four(transformer: *mut Transformer, src1: u8, src2: u8, src3: u8, src4: u8) {
    let transformer = &mut *transformer;
    transformer.src4[0] = src1;
    transformer.src4[1] = src2;
    transformer.src4[2] = src3;
    transformer.src4[3] = src4;
    let dest = &mut transformer.dest;
    transformer
        .transform
        .convert(&transformer.src4, dest);
    copy_rgb(dest.as_ptr());
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_transformer_from_memory(mem: &[u8], in_type: DataType, intent: Intent) -> *mut Transformer {
    Transformer::new(mem, in_type, intent)
        .map_or_else(null_mut, |t| Box::into_raw(Box::new(t)))
}

#[wasm_bindgen]
/// # Safety
///
/// This function is called directly from JavaScript.
pub unsafe fn qcms_drop_transformer(transformer: *mut Transformer) {
    drop(Box::from_raw(transformer));
}
