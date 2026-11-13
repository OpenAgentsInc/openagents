// Tooling only: instantiate the real C/Wasm units without a browser or audio.
const fs = require("node:fs");
(async () => {
  const module = await WebAssembly.compile(fs.readFileSync(process.argv[2]));
  const memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
  const imports = { env: { memory } };
  const inventory = WebAssembly.Module.imports(module);
  for (const item of inventory) {
    if (item.kind !== "function") continue;
    (imports[item.module] ||= {})[item.name] = () => {
      throw Error(`unexpected host effect ${item.module}.${item.name}`);
    };
  }
  const instance = await WebAssembly.instantiate(module, imports);
  const e = instance.exports;
  e._initialize?.();
  e.follower_init();
  const bytes = fs.readFileSync(process.argv[3]);
  if (bytes.length % 12) throw Error("partial operation record");
  const rows = [];
  for (let at = 0; at < bytes.length; at += 12) {
    e.follower_apply(bytes.readInt32LE(at), bytes.readFloatLE(at + 4), bytes.readInt32LE(at + 8));
    rows.push([...new Uint32Array(memory.buffer, e.follower_snapshot(), 61)]);
  }
  process.stdout.write(JSON.stringify({ rows, imports: inventory }));
})().catch(error => { console.error(error); process.exitCode = 1; });
