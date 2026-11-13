// Firm reason: 2 (independent oracle: runs a realm script's module in V8's own WebAssembly engine, so it
// shares nothing with our interpreter, metaverse-engine/script/interp.coax).
//
// Runs the torture script's module (metaverse-engine/script/examples/torture.coax, built by
// coax/web/script.sh) with WebAssembly.instantiate, makes the calls that
// metaverse-engine/tools/script-oracle.coax makes through our host, and prints the same lines: for each
// call, the tick, the handler, the thing, the thing's state record as hex and the commands it queued.
// The imports answer by the host's rules (metaverse-engine/script/host.coax) for the few calls the
// script makes, over the same fixed frame. script-oracle.coax compares the two outputs byte for byte.
//   node metaverse-engine/tests/script-oracle.mjs MODULE.wasm TICKS
import { readFileSync } from "node:fs";

const [wasmPath, ticksText] = process.argv.slice(2);
const ticks = Number(ticksText);
if (!wasmPath || !Number.isInteger(ticks) || ticks < 1) {
  console.error("usage: node metaverse-engine/tests/script-oracle.mjs MODULE.wasm TICKS");
  process.exit(2);
}

const THINGS = 3;
const f32 = Math.fround;
const f32Bits = (x) => { const v = new DataView(new ArrayBuffer(4)); v.setFloat32(0, x, true); return v.getUint32(0, true); };
const hex = (v, digits) => BigInt.asUintN(digits * 4, BigInt(v)).toString(16).padStart(digits, "0");

// The frame of tick t, as script-oracle.coax's so_frame writes it.
const frameOf = (t) => ({
  dt: f32((t % 4 + 1) / 8),
  time: f32(t / 4),
  world: (k) => (k >= 0 && k < 8 ? f32(k * 1.5) : 0),
  things: [0, 1, 2].map((i) => ({ id: BigInt(101 + i), x: f32(3 * i + t / 2), z: -2 * i, speed: f32(1.5 + i), health: 10 - i })),
});

let frame = frameOf(0);
let me = 0n;
let commands = [];
let failed = "";
const thing = (id) => frame.things.find((t) => t.id === id);
const finite = (x) => Number.isFinite(x) && x > -1e9 && x < 1e9;
const write = (id) => { if (id !== me) failed = "not-yours"; };

// The host's answers (host.coax, sh_import) for the calls the torture script makes. A command is
// [op, thing, target, i, a, b, c]; ops are API row numbers (api-table.coax): move 26, set_var 29.
const env = {
  sa_trap: (line) => { failed = `trap ${line}`; },
  sa_count: () => THINGS,
  sa_x: (id) => thing(id)?.x ?? 0,
  sa_z: (id) => thing(id)?.z ?? 0,
  sa_speed: (id) => thing(id)?.speed ?? 0,
  sa_health: (id) => thing(id)?.health ?? 0,
  sa_time: () => frame.time,
  sa_world: (k) => frame.world(k),
  sa_move: (id, x, z, speed) => {
    write(id);
    if (!finite(x) || !finite(z) || !finite(speed)) failed = "bad-argument";
    let c = speed; const top = thing(id)?.speed ?? 0;
    if (c > top) c = top;
    if (c < 0) c = 0;
    commands.push([26, id, 0n, 0, x, z, c]);
  },
  sa_set_var: (id, slot, value) => {
    write(id);
    if (slot < 0 || slot >= 4 || !finite(value)) failed = "bad-argument";
    commands.push([29, id, 0n, slot, value, 0, 0]);
  },
  sa_timer: (id, timer, seconds) => {
    write(id);
    if (timer < 0 || timer >= 4 || !finite(seconds)) failed = "bad-argument";
  },
};

const module = new WebAssembly.Module(readFileSync(wasmPath));
for (const imp of WebAssembly.Module.imports(module)) {
  if (imp.module !== "env" || !(imp.name in env)) { console.error(`script-oracle.mjs: no answer for ${imp.module}.${imp.name}`); process.exit(2); }
}
const { exports } = new WebAssembly.Instance(module, { env });
const stateAt = exports.rs_state();
const stateSize = exports.rs_state_size();

const lines = [];
const call = (t, name, slot, ...rest) => {
  me = BigInt(101 + slot);
  commands = []; failed = "";
  exports[`rs_${name}`](me, slot, ...rest);
  if (failed) { console.error(`script-oracle.mjs: tick ${t} ${name} thing ${me}: ${failed}`); process.exit(1); }
  const bytes = new Uint8Array(exports.memory.buffer, stateAt + slot * stateSize, stateSize);
  let line = `${hex(t, 4)} ${name} ${hex(me, 16)} ${Array.from(bytes, (b) => hex(b, 2)).join("")}`;
  for (const [op, id, target, i, a, b, c] of commands) {
    line += ` ${hex(op, 2)}:${hex(id, 16)}:${hex(target, 16)}:${hex(i, 8)}:${hex(f32Bits(a), 8)}:${hex(f32Bits(b), 8)}:${hex(f32Bits(c), 8)}`;
  }
  lines.push(line);
};
// The calls of tick t, as script-oracle.coax's so_tick makes them.
for (let t = 0; t < ticks; t++) {
  frame = frameOf(t);
  for (let i = 0; i < THINGS; i++) call(t, "on_tick", i, frame.dt);
  if (t % 3 === 1) call(t, "on_hit", 0, 102n, f32(t / 2));
  if (t % 4 === 2) call(t, "on_timer", 1, t % 4);
  if (t % 5 === 3) call(t, "on_enter", 2, 101n);
}
process.stdout.write(lines.join("\n") + "\n");
