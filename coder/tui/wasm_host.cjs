// Firm reason: 1 (Coax has no WebAssembly engine; node's WebAssembly is the bounded sandbox a Wasm plugin runs in)
// Runs one export of a Coder Wasm plugin (Coder book chapter 15) with only
// the host functions its plugin was admitted, in a worker thread that is
// ended after SECONDS. Coder already refused the module at admission if it
// imports anything else or has no memory maximum; this host refuses it again.
// WASI functions, which the Coax runtime links for its failure path, end the
// plugin when called.
//   node coder/tui/wasm_host.cjs MODULE.wasm EXPORT ALLOWED,IMPORTS SECONDS
// Host functions, in module "env":
//   coder_print(s: [] u8) -> int   a line of output
//   coder_now_ms() -> int          the wall clock, in milliseconds
'use strict';
const { Worker, isMainThread, workerData, parentPort } = require('node:worker_threads');

if (isMainThread) {
  const [file, exp, allowed, secs] = process.argv.slice(2);
  const seconds = Math.min(Math.max(Number(secs) || 10, 1), 60);
  const w = new Worker(__filename, {
    workerData: { file, exp, allowed: (allowed || '').split(',').filter(Boolean) },
    resourceLimits: { maxOldGenerationSizeMb: 64, maxYoungGenerationSizeMb: 16 },
  });
  const timer = setTimeout(() => {
    process.stderr.write(`wasm plugin: stopped after ${seconds}s\n`);
    process.exitCode = 124;
    w.terminate();
  }, seconds * 1000);
  w.on('message', (m) => process.stdout.write(m));
  w.on('error', (e) => { process.stderr.write(`wasm plugin: ${e.message}\n`); if (!process.exitCode) process.exitCode = 1; });
  w.on('exit', () => clearTimeout(timer));
} else {
  const fs = require('node:fs');
  const mod = new WebAssembly.Module(fs.readFileSync(workerData.file));
  let memory = null;
  const host = {
    coder_print: (view) => {
      // a Coax [] u8 passed indirectly: {data: i32 at +0, count: i64 at +8}
      const v = new DataView(memory.buffer);
      const at = v.getUint32(view, true);
      const n = Number(v.getBigInt64(view + 8, true));
      if (n < 0 || at + n > memory.buffer.byteLength) throw Error('coder_print: a view outside the memory');
      parentPort.postMessage(Buffer.from(memory.buffer, at, n).toString('utf8') + '\n');
      return 0n;
    },
    coder_now_ms: () => BigInt(Date.now()),
  };
  const env = {};
  // the Coax runtime's failure path (a bounds check that fails) links WASI's
  // stdio; each one ends the plugin instead of writing anywhere
  const wasi = {};
  const failed = () => { throw Error('a runtime check failed'); };
  for (const i of WebAssembly.Module.imports(mod)) {
    if (i.module === 'wasi_snapshot_preview1' && i.kind === 'function') { wasi[i.name] = failed; continue; }
    if (i.module !== 'env' || !workerData.allowed.includes(i.name) || !host[i.name]) {
      throw Error(`it imports ${i.module}.${i.name}, which it was not admitted`);
    }
    env[i.name] = host[i.name];
  }
  const inst = new WebAssembly.Instance(mod, { env, wasi_snapshot_preview1: wasi });
  memory = inst.exports.memory;
  if (inst.exports._initialize) inst.exports._initialize();
  const f = inst.exports[workerData.exp];
  if (typeof f !== 'function') throw Error(`it has no export ${workerData.exp}`);
  const r = f();
  if (typeof r === 'bigint' && r !== 0n) throw Error(`${workerData.exp} returned ${r}`);
}
