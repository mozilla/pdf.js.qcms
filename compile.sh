#!/bin/sh

# MIT License
#
# Copyright (c) 2025 Mozilla Foundation
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

set -e

INPUT=${INPUT:=.}
OUTPUT=${OUTPUT:=.}
OUTPUT_NAME=${OUTPUT_NAME:=qcms}
RUSTC_VERSION=$(rustc -vV | sed -n 's/^release: //p')
WASM_PACK_VERSION=$(wasm-pack --version | sed 's/^wasm-pack //')

wasm-pack build \
          --target web \
          --release \
          --no-typescript \
          --no-pack \
          --out-name "${OUTPUT_NAME}" \
          --out-dir "${OUTPUT}" \
          -- --locked

node "$(dirname "$0")/build_metadata.js" \
     "${OUTPUT}/${OUTPUT_NAME}_bg.wasm" Cargo.lock "${INPUT}" \
     "${RUSTC_VERSION}" "${WASM_PACK_VERSION}"

sed -i "1 i\/* THIS FILE IS GENERATED - DO NOT EDIT */" "${OUTPUT}/${OUTPUT_NAME}.js"
cp ./js/qcms_utils.js "${OUTPUT}"
