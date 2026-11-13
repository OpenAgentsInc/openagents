// Firm reason: 1 (it runs the page's script module as a browser does: WebAssembly.instantiate and byte copies; it holds no logic of its own)
// The page's half of coax/web/room/room-scripts-check.coax (GitHub #13467): loads the page's script module
// (script-host.wasm), hands it the modules and the frames the check wrote, and writes each tick's result.
//   node room-scripts-page.mjs DIR     reads DIR/script-host.wasm, DIR/modules.txt, DIR/frames.bin; writes DIR/page.bin
import { readFileSync, writeFileSync } from "node:fs";
const dir = process.argv[2];
const module = new WebAssembly.Module(readFileSync(`${dir}/script-host.wasm`));
const memory = new WebAssembly.Memory({ initial: 64, maximum: 1024 });
const imports = { env: { memory } };
for (const imp of WebAssembly.Module.imports(module)) if (imp.kind === "function") (imports[imp.module] ||= {})[imp.name] ||= () => 0;
const x = new WebAssembly.Instance(module, imports).exports;
x._initialize?.();
const state = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536;
memory.grow(Math.ceil((x.coaxial_state_size() + 64) / 65536));
x.coaxial_state_init(state);
const put = (bytes, at = 0) => new Uint8Array(memory.buffer, x.scripts_in() + at, bytes.length).set(bytes);
// modules.txt: one line a module, "PLACE NAME FILE FIELDS-JSON"
for (const line of readFileSync(`${dir}/modules.txt`, "utf8").split("\n")) {
  if (!line) continue;
  const [place, name, file, ...rest] = line.split(" ");
  const n = new TextEncoder().encode(name), b = readFileSync(`${dir}/${file}`), f = new TextEncoder().encode(rest.join(" "));
  put(n); put(b, n.length); put(f, n.length + b.length);
  const code = x.scripts_try(state, Number(place), n.length, b.length, f.length);
  if (code !== 0) { console.error(`the page's module refused ${name}: ${code}`); process.exit(1); }
}
// frames.bin and page.bin: each record a little-endian u32 length, then the bytes
const frames = readFileSync(`${dir}/frames.bin`), out = [];
for (let at = 0; at + 4 <= frames.length;) {
  const n = frames.readUInt32LE(at); at += 4;
  put(frames.subarray(at, at + n)); at += n;
  const rn = x.scripts_tick(state, n), head = Buffer.alloc(4);
  head.writeUInt32LE(rn);
  out.push(head, Buffer.from(new Uint8Array(memory.buffer, x.scripts_out(), rn)));
}
writeFileSync(`${dir}/page.bin`, Buffer.concat(out));
