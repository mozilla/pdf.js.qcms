/*
 * MIT License
 *
 * Copyright (c) 2026 Mozilla Foundation
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

"use strict";

const { execFileSync } = require("child_process");
const fs = require("fs");

const SECTION_NAME = "build_info";

function git(dir, args) {
  return execFileSync("git", args, {
    cwd: dir,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
}

function getRepoRevision(dir, gitOptions = []) {
  try {
    const revision = git(dir, [...gitOptions, "rev-parse", "HEAD"]);
    const status = git(dir, [
      ...gitOptions,
      "status",
      "--porcelain",
      "--untracked-files=normal",
    ]);
    return status ? `${revision}-dirty` : revision;
  } catch {
    return "unknown";
  }
}

// Get the version of a crate from Cargo.lock, which pins the whole build.
function getLockedVersion(lockFile, name) {
  const lock = fs.readFileSync(lockFile, "utf8");
  const versions = new Set();
  for (const pkg of lock.split(/^\[\[package\]\]$/m).slice(1)) {
    if (pkg.match(/^name = "(.*)"$/m)?.[1] === name) {
      versions.add(pkg.match(/^version = "(.*)"$/m)?.[1]);
    }
  }
  if (versions.size !== 1) {
    throw new Error(`Expected exactly one version of ${name} in ${lockFile}`);
  }
  return [...versions][0];
}

function encodeU32(value) {
  const bytes = [];
  do {
    let byte = value & 0x7f;
    value >>>= 7;
    if (value) {
      byte |= 0x80;
    }
    bytes.push(byte);
  } while (value);
  return Buffer.from(bytes);
}

function addCustomSection(wasm, name, payload) {
  const encodedName = Buffer.from(name, "utf8");
  const content = Buffer.concat([
    encodeU32(encodedName.length),
    encodedName,
    payload,
  ]);
  // A custom section has the id 0 and can be anywhere after the header.
  return Buffer.concat([
    wasm,
    Buffer.from([0]),
    encodeU32(content.length),
    content,
  ]);
}

function getBuildInfoSection(wasm) {
  const module = new WebAssembly.Module(wasm);
  const [section] = WebAssembly.Module.customSections(module, SECTION_NAME);
  return section;
}

function readBuildMetadata(wasmFile) {
  const section = getBuildInfoSection(fs.readFileSync(wasmFile));
  return section ? JSON.parse(new TextDecoder().decode(section)) : undefined;
}

function addBuildMetadata(wasmFile, lockFile, sourceDir, rustc, wasmPack) {
  // The mounted checkout can belong to a different user inside Docker.
  const repo =
    process.env.REPO_REVISION ||
    getRepoRevision(sourceDir, ["-c", "safe.directory=*"]);
  const info = {
    qcms: getLockedVersion(lockFile, "qcms"),
    repo,
    rustc,
    wasmBindgen: getLockedVersion(lockFile, "wasm-bindgen"),
    wasmPack,
  };

  const wasm = fs.readFileSync(wasmFile);
  if (getBuildInfoSection(wasm)) {
    throw new Error(`${wasmFile} already has a ${SECTION_NAME} section`);
  }
  const payload = Buffer.from(JSON.stringify(info), "utf8");
  fs.writeFileSync(wasmFile, addCustomSection(wasm, SECTION_NAME, payload));
  return info;
}

if (require.main === module) {
  const [wasmFile, lockFile, sourceDir, rustc, wasmPack] =
    process.argv.slice(2);
  const info = addBuildMetadata(wasmFile, lockFile, sourceDir, rustc, wasmPack);
  console.log(`Added ${JSON.stringify(info)} to ${wasmFile}`);
}

module.exports = { getRepoRevision, readBuildMetadata };
