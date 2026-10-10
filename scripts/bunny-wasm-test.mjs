// Runs a Rust test binary built for wasm32-wasip1 under Node's WASI, so
// `cargo test --target wasm32-wasip1` runs the same tests as wasm. Grow
// Little Bunny uses it to check that its rules end a run in the same state
// on native and wasm32 (`scripts/bunny-wasm-test.sh`).
//
// Usage (as cargo's runner): node scripts/bunny-wasm-test.mjs TEST.wasm [ARGS]
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";
import { argv, env, exit } from "node:process";

const [file, ...args] = argv.slice(2);
const wasi = new WASI({
  version: "preview1",
  args: [file, ...args],
  env: { RUST_BACKTRACE: env.RUST_BACKTRACE ?? "0" },
  returnOnExit: true,
});
const module = await WebAssembly.compile(await readFile(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
exit(wasi.start(instance));
