#!/usr/bin/env bash
# Builds the JavaScript/TypeScript package (WebAssembly) into pkg/ and runs its tests.
#
# Target: Node.js (CommonJS, loads synchronously).
set -euo pipefail
cd "$(dirname "$0")/.."

wasm-pack build --target nodejs --release --out-dir pkg -- --features wasm

# The public API is js/index.cjs (+ its types); the generated bindings are internal.
cp js/index.cjs js/index.d.ts pkg/

node -e '
const fs = require("fs");
const p = JSON.parse(fs.readFileSync("pkg/package.json", "utf8"));
p.author = { name: "PokerInfra", url: "https://www.pokerinfra.com" };
p.contributors = [{ name: "Vikas Sood", url: "https://github.com/vikassood11" }];
p.bugs = { url: "https://github.com/pokerinfra/poker-eval/issues" };
p.repository = { type: "git", url: "git+https://github.com/pokerinfra/poker-eval.git" };
p.main = "index.cjs";
p.types = "index.d.ts";
p.files = ["index.cjs", "index.d.ts", "pi_poker_eval.js", "pi_poker_eval_bg.wasm"];
fs.writeFileSync("pkg/package.json", JSON.stringify(p, null, 2) + "\n");
'

node --test js-test/
