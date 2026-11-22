# pdf.js.qcms

Provide basic bindings for [qcms](https://github.com/FirefoxGraphics/qcms) library.

## Build

Run:

```sh
node build.js --compile --output my_output_dir
```

This builds the Docker image using Docker's layer cache, then writes `qcms.js`, `qcms_bg.wasm` and `qcms_utils.js` to `my_output_dir`. The image pins the Rust toolchain, `wasm-pack` and `wasm-bindgen-cli`, and the crates are built from `Cargo.lock`.

### Build metadata

`qcms_bg.wasm` contains a `build_info` custom section with a JSON object describing the build: the qcms version (`qcms`), the revision of this repository (`repo`, suffixed with `-dirty` if there were local changes), the Rust version (`rustc`), the wasm-bindgen version (`wasmBindgen`) and the wasm-pack version (`wasmPack`). It can be read with:

```js
const module = await WebAssembly.compile(bytes);
const [section] = WebAssembly.Module.customSections(module, "build_info");
const buildInfo = JSON.parse(new TextDecoder().decode(section));
```

The tests for the metadata can be run with `node --test test/build_metadata.test.js`.

## Update

To update qcms, change its version in `Cargo.toml` if needed, then run:

```sh
cargo update qcms
node build.js --compile --output my_output_dir
```

When updating `wasm-bindgen` in `Cargo.lock`, `wasm-bindgen-cli` in `Dockerfile` must be updated to the same version.

Use `node build.js --create` to build only the Docker image.

## Release

Pushing a version tag (e.g. `v1.2.3`) runs the `Release` workflow: it builds the bindings and opens a pull request in [pdf.js](https://github.com/mozilla/pdf.js) updating `external/qcms/`, titled "Update qcms bindings to version 1.2.3":
```sh
git tag v1.2.3
git push origin v1.2.3
```

The workflow refuses to replace bindings in pdf.js that were built from a revision which isn't an ancestor of the tagged one, so that retrying an old tag can't revert a newer release.

The workflow runs in the `release` environment, which must provide the `CLIENT_ID` and `APP_PRIVATE_KEY` secrets of a GitHub App installed on `mozilla/pdf.js` with read and write access to contents and pull requests.

## API

`qcms_transformer_from_memory(profile, inType, intent)` returns a pointer, or 0
if the profile cannot be used. Only `RGB8`, `Gray8` and `CMYK` input types
actually work: the transform always targets sRGB, and qcms will not convert
`RGBA8`, `BGRA8` or `GrayA8` into a three-channel destination.

`qcms_convert_one` / `_three` / `_four` return the colour as `0x00RRGGBB`. Their
components are `u8`, so the JS→Wasm boundary truncates toward zero; a caller
passing pre-scaled floats gets the same result as one storing them into a
`Uint8Array` first, which is what makes the bulk and per-colour paths agree.

`qcms_convert_array(transformer, src, addAlpha)` converts a buffer and calls
`copy_result` in `qcms_utils.js`, which copies it to wherever `QCMS._destBuffer`
and `QCMS._destOffset` point. The Wasm side writes 3 bytes per pixel, or 4 with
an opaque alpha when `addAlpha` is set, so `copy_result` is a single bulk copy.
Set `QCMS._keepAlpha` when the destination is RGBA and its alpha channel already
holds something that must survive — an image with an `/SMask` has its alpha
decoded before its colours — and the bytes are merged rather than overwritten.

The caller must set `QCMS._memory` to the module's memory before converting
anything.

## Licensing

The code is released under [MIT](https://mit-license.org/).
