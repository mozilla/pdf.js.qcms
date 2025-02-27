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

## Licensing

The code is released under [MIT](https://mit-license.org/).
