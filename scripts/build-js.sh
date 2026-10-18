#!/usr/bin/env bash
# Builds the JavaScript/TypeScript package (WebAssembly) into pkg/ and runs its tests.
#
# Target: Node.js (CommonJS, loads synchronously).
# The package is marked "private" until the first release, so an accidental
# `npm publish` in pkg/ is refused.
set -euo pipefail
cd "$(dirname "$0")/.."

wasm-pack build --target nodejs --release --out-dir pkg -- --features wasm

node -e '
const fs = require("fs");
const p = JSON.parse(fs.readFileSync("pkg/package.json", "utf8"));
p.private = true;
fs.writeFileSync("pkg/package.json", JSON.stringify(p, null, 2) + "\n");
'

node --test js-test/
