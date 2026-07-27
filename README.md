# pdf.js.qcms

Provide basic bindings for [qcms](https://github.com/FirefoxGraphics/qcms) library.

## Build

Run:

```sh
node build.js --compile --output my_output_dir
```

it will create a Docker image with emsdk and then run it. The generated `qcms.js` and `qcms.wasm` will be in `my_output_dir`.

## Update

In order to update qcms to a specific version, change the version in Cargo.toml and then run:
```sh
node build.js --create
```
to create a new docker image and then
```sh
node build.js --compile --output my_output_dir
```
to compile. The short version is:
```sh
node build.js -Cco my_output_dir
```

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
