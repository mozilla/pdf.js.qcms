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

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { test } = require("node:test");

const LOCK = `version = 4

[[package]]
name = "qcms"
version = "0.3.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "wasm-bindgen"
version = "0.2.130"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "qcms",
]

[[package]]
name = "wasm-bindgen-shared"
version = "0.2.129"
`;

// A module with a type section, so that the custom section isn't the only one.
const WASM = Uint8Array.of(
  ...[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00],
  ...[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
);

function setup(t, lock = LOCK) {
  const dir = mkdtempSync(join(tmpdir(), "qcms-metadata-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  // Keep the wasm file out of the repository, where it would be untracked.
  const repo = join(dir, "repo");
  mkdirSync(repo);
  const lockFile = join(repo, "Cargo.lock");
  const wasmFile = join(dir, "qcms_bg.wasm");
  writeFileSync(lockFile, lock);
  writeFileSync(wasmFile, WASM);
  const env = { ...process.env, REPO_REVISION: "" };
  delete env.GIT_INDEX_FILE;
  const addMetadata = (extraEnv = {}) =>
    execFileSync(
      process.execPath,
      [
        require.resolve("../build_metadata.js"),
        wasmFile,
        lockFile,
        repo,
        "1.100.0",
        "0.15.1",
      ],
      { cwd: repo, env: { ...env, ...extraEnv }, stdio: "pipe" },
    );
  return { repo, env, lockFile, wasmFile, addMetadata };
}

test("build metadata records the versions and local changes", (t) => {
  const { repo, env, lockFile, wasmFile, addMetadata } = setup(t);
  const git = (...args) =>
    execFileSync(
      "git",
      ["-c", "user.name=Test", "-c", "user.email=test@example.com", ...args],
      { cwd: repo, encoding: "utf8", env },
    ).trim();

  git("init", "--quiet");
  git("add", ".");
  git("commit", "--quiet", "--no-gpg-sign", "-m", "Initial commit");

  const revision = git("rev-parse", "HEAD");
  const readMetadata = (extraEnv) => {
    writeFileSync(wasmFile, WASM);
    addMetadata(extraEnv);
    const wasm = new WebAssembly.Module(readFileSync(wasmFile));
    const sections = WebAssembly.Module.customSections(wasm, "build_info");
    assert.equal(sections.length, 1);
    return JSON.parse(new TextDecoder().decode(sections[0]));
  };

  assert.deepEqual(readMetadata(), {
    qcms: "0.3.0",
    repo: revision,
    rustc: "1.100.0",
    wasmBindgen: "0.2.130",
    wasmPack: "0.15.1",
  });

  assert.equal(readMetadata({ REPO_REVISION: "abc" }).repo, "abc");

  git("config", "status.showUntrackedFiles", "no");
  const untracked = join(repo, "untracked.js");
  writeFileSync(untracked, "untracked\n");
  assert.equal(readMetadata().repo, `${revision}-dirty`);
  rmSync(untracked);
  assert.equal(readMetadata().repo, revision);

  writeFileSync(lockFile, `${LOCK}\n`);
  assert.equal(readMetadata().repo, `${revision}-dirty`);

  writeFileSync(join(repo, ".git", "index"), "invalid index\n");
  assert.equal(readMetadata().repo, "unknown");
});

test("build metadata isn't added twice", (t) => {
  const { addMetadata } = setup(t);
  addMetadata();
  assert.throws(addMetadata, /already has a build_info section/);
});

test("build metadata requires a single version of a crate", (t) => {
  const lock = `${LOCK}\n[[package]]\nname = "qcms"\nversion = "0.2.0"\n`;
  const { wasmFile, addMetadata } = setup(t, lock);
  assert.throws(addMetadata, /Expected exactly one version of qcms/);
  assert.deepEqual(readFileSync(wasmFile), Buffer.from(WASM));
});
