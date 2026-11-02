// C-only Grid link qualification. No scene frame, GPU, browser, network or audio.
// Usage: bun aethyr/tests/check_grid_memory.ts FILE.wasm RECEIPT.json
import { createHash } from "node:crypto";
import { dirname, join } from "node:path";
import { mkdir, readFile, writeFile } from "node:fs/promises";

const [file, receipt] = process.argv.slice(2);
if (!file || !receipt) throw new Error("usage: check_grid_memory.ts FILE.wasm RECEIPT.json");
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const bytes = new Uint8Array(await readFile(file));
const driver = new Uint8Array(await readFile(import.meta.path));
const reader = (data: Uint8Array) => {
  let at = 0;
  const byte = () => { if (at >= data.length) throw new Error("truncated Wasm"); return data[at++]; };
  const uint = () => { let value = 0, shift = 0; for (let n = 0; n < 5; n++) { const b = byte(); value += (b & 127) * 2 ** shift; if (!(b & 128)) return value; shift += 7; } throw new Error("wide Wasm unsigned LEB"); };
  const signed = () => { let value = 0, shift = 0, b = 0; do { b = byte(); value += (b & 127) * 2 ** shift; shift += 7; } while (b & 128); return b & 64 ? value - 2 ** shift : value; };
  const raw = (n: number) => { if (at + n > data.length) throw new Error("short Wasm section"); const b = data.slice(at, at + n); at += n; return b; };
  const text = () => new TextDecoder().decode(raw(uint()));
  return { byte, uint, signed, raw, text, remaining: () => data.length - at };
};
const wasm = reader(bytes);
const magic = [...wasm.raw(8)];
if (magic.join() !== "0,97,115,109,1,0,0,0") throw new Error("not Wasm v1");
let minimum = -1, maximum = -1, stackPointer = -1;
while (wasm.remaining()) {
  const kind = wasm.byte(), section = reader(wasm.raw(wasm.uint()));
  if (kind === 2) {
    const count = section.uint();
    for (let i = 0; i < count; i++) {
      section.text(); section.text();
      const item = section.byte();
      if (item === 0) section.uint();
      else if (item === 2) {
        const flags = section.uint();
        minimum = section.uint(); maximum = flags & 1 ? section.uint() : -1;
        if (flags !== 1) throw new Error("expected bounded unshared wasm32 memory");
      } else throw new Error("unexpected nonfunction/nonmemory import");
    }
  }
  if (kind === 6) {
    const count = section.uint();
    if (count < 1 || section.byte() !== 0x7f || section.byte() !== 1 || section.byte() !== 0x41) throw new Error("expected mutable i32 stack pointer first");
    stackPointer = section.signed();
    if (section.byte() !== 0x0b) throw new Error("invalid stack initializer");
  }
}
if (minimum < 0 || maximum < minimum) throw new Error("missing bounded imported memory");
const module = new WebAssembly.Module(bytes);
const memory = new WebAssembly.Memory({ initial: Math.max(128, minimum), maximum });
const effects: string[] = [];
const imports: Record<string, Record<string, unknown>> = {};
for (const entry of WebAssembly.Module.imports(module)) {
  const bank = imports[entry.module] ||= {};
  if (entry.kind === "memory") bank[entry.name] = memory;
  else if (entry.kind === "function") bank[entry.name] = () => { effects.push(entry.module + "." + entry.name); throw new Error("unexpected platform effect during scalar measurement"); };
  else throw new Error("unsupported import kind");
}
const instance = new WebAssembly.Instance(module, imports as WebAssembly.Imports);
const exports = instance.exports as Record<string, any>;
exports._initialize?.();
const heapBase = Number(exports.__heap_base.value);
const stateSize = Number(exports.coaxial_state_size());
const vertices = Number(exports.coaxial_max_vertices());
const stride = Number(exports.coaxial_vertex_floats());
const uniformFloats = Number(exports.coaxial_uniform_floats());
const align = (n: number, a: number) => Math.ceil(n / a) * a;
// Independent reproduction of the production host's existing arena endpoints.
// The actual memory.grow below exercises the originally failing platform effect.
const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
const stateBytes = align(Math.max(stateSize, 1024), 16);
const need = base + stateBytes + vertices * stride * 4 + uniformFloats * 4 + 64;
const pages = Math.ceil(need / 65536);
let growthError: string | null = null;
try { memory.grow(Math.max(0, pages - memory.buffer.byteLength / 65536)); }
catch (error) { growthError = String(error); }
const failures = [
  ...(stackPointer === 4194304 ? [] : ["Grid stack reservation is not 4 MiB"]),
  ...(maximum === 1024 ? [] : ["Wasm memory maximum changed from 64 MiB"]),
  ...(vertices === 400000 && stride === 12 && uniformFloats === 112 ? [] : ["Grid producer capacities changed"]),
  ...(growthError ? ["startup arena memory.grow refused"] : []),
  ...(effects.length ? ["scalar probe called platform effects"] : []),
];
const result = {
  schema: "aethyr.grid.memory-link-check.v1", status: failures.length ? "failed" : "passed",
  scope: "Real Wasm instantiation, scalar getters and arena memory.grow; no producer frame or GPU image claim.",
  wasm: file, wasm_sha256: hash(bytes), driver_sha256: hash(driver),
  minimum_pages: minimum, maximum_pages: maximum, stack_pointer_initial: stackPointer,
  heap_base: heapBase, state_size: stateSize, max_vertices: vertices, vertex_floats: stride,
  uniform_floats: uniformFloats, arena_base: base, arena_need: need, arena_pages: pages,
  final_memory_bytes: memory.buffer.byteLength, growth_error: growthError, effects, failures,
};
await mkdir(dirname(receipt), { recursive: true });
await writeFile(join(dirname(receipt), "driver.ts"), driver);
await writeFile(receipt, JSON.stringify(result, null, 2) + "\n");
if (hash(new Uint8Array(await readFile(file))) !== result.wasm_sha256) throw new Error("Wasm changed during probe");
console.log(JSON.stringify(result));
process.exitCode = failures.length ? 1 : 0;
