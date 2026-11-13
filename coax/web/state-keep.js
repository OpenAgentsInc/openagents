// Firm reason: 1 (browser glue: host.js carries a page's state across a development engine swap)
// A code reload keeps the game's state (#13411, metaverse-engine/book/ch082-change-code-while-it-runs.md).
//
// The field table is the text `coax layout` prints (coax/src/emit_c.cpp layout_report), the same
// text `coax drive` reads fields through (coax/web/drive.coax). A development build carries it in
// the module's "coaxial.state" custom section (coax/web/wasm.sh, wasm-size state). When a new
// engine's table differs from the running one's, host.js copies the old state out, starts the new
// state from its defaults (coaxial_state_init), and applies the plan made here:
//
//   same name, same type          copied
//   a struct                      field by field, by name; its order and offsets may change
//   [N] T to [M] T                the first min(N, M) elements, each by these rules
//   a wider number                converted: u8 < u16 < u32 < u64, s8 < s16 < s32 < s64 (int),
//                                 unsigned to a wider signed, f32 to f64
//   an enum whose members moved   by member name
//   a new field                   its default
//   a removed field               dropped
//   any other change of type      its default
//   a string or a view ([] T)     its default: it points into memory, which a swap may move
//
// The plan is a function of the two tables alone, so the same edit always keeps the same fields.

export const STATE_SECTION = "coaxial.state";

// The module's field table, or "" (a release module has none).
export function stateTable(module) {
  const parts = WebAssembly.Module.customSections(module, STATE_SECTION);
  return parts.length ? new TextDecoder().decode(parts[0]) : "";
}

const HEAD_SUM = /^(.+?)  sum size (\d+)  align \d+  tag u32@0  payload@(\d+)$/;
const HEAD_ENUM = /^(.+?)  enum (\S+)  size (\d+)$/;
const HEAD_STRUCT = /^(.+?)  size (\d+)  align \d+$/;
const FIELD = /^ +(\d+) +(\d+)  ([^:(][^:]*): (.+)$/;
const MEMBER = /^  (\S+) = (-?\d+)$/;
const VARIANT = /^  \d+: (.+)$/;

// `coax layout`'s text as tables: structs (size and fields), enums (their members) and sums.
export function parseLayout(text) {
  const layout = { structs: new Map(), enums: new Map(), sums: new Map() };
  let struct = null, en = null, sum = null, m;
  for (const line of text.split("\n")) {
    if (!line) continue;
    if (line[0] !== " ") {
      struct = en = sum = null;
      if ((m = HEAD_SUM.exec(line))) { sum = { size: +m[2], payload: +m[3], variants: [] }; if (!layout.sums.has(m[1])) layout.sums.set(m[1], sum); }
      else if ((m = HEAD_ENUM.exec(line))) { en = { base: m[2], size: +m[3], members: new Map() }; if (!layout.enums.has(m[1])) layout.enums.set(m[1], en); }
      else if ((m = HEAD_STRUCT.exec(line))) { struct = { size: +m[2], fields: [] }; if (!layout.structs.has(m[1])) layout.structs.set(m[1], struct); }
      continue;
    }
    if (struct && (m = FIELD.exec(line))) struct.fields.push({ name: m[3], offset: +m[1], size: +m[2], type: m[4] });
    else if (en && (m = MEMBER.exec(line))) en.members.set(m[1], BigInt(m[2]));
    else if (sum && (m = VARIANT.exec(line))) sum.variants.push(m[1]);
  }
  return layout;
}

// The state's struct: the one of the state's size (coaxial_state_size), as `coax drive` picks it
// (drive.coax dv_root). Of several, `prefer` if it is one, else the one with most fields; "" when
// none has that size or two tie, since a wrong root would copy bytes to the wrong fields.
export function stateRoot(layout, size, prefer = "") {
  let best = "", most = -1, tie = false;
  for (const [name, s] of layout.structs) {
    if (s.size !== size) continue;
    if (name === prefer) return name;
    if (s.fields.length > most) { best = name; most = s.fields.length; tie = false; }
    else if (s.fields.length === most) tie = true;
  }
  return tie ? "" : best;
}

const NUMBERS = { bool: [1, "b"], u8: [1, "u"], u16: [2, "u"], u32: [4, "u"], u64: [8, "u"], uint: [8, "u"],
  s8: [1, "s"], s16: [2, "s"], s32: [4, "s"], s64: [8, "s"], int: [8, "s"], f32: [4, "f"], f64: [8, "f"] };
const ARRAY = /^\[(\d+)\] (.+)$/;

function kind(layout, type) {
  if (NUMBERS[type]) return "number";
  if (type === "string" || type.startsWith("[] ")) return "pointer";
  if (ARRAY.test(type)) return "array";
  if (layout.sums.has(type)) return "sum";
  if (layout.structs.has(type)) return "struct";
  if (layout.enums.has(type)) return "enum";
  return type === "void" ? "void" : "other";
}
// May a number of type `from` become one of type `to` with its value unchanged?
function widens(from, to) {
  const [fs, fk] = NUMBERS[from], [ts, tk] = NUMBERS[to];
  if (fk === "b" || tk === "b") return false;
  if (fk === "f" || tk === "f") return fk === tk && ts > fs;
  if (fk === tk) return ts > fs;
  return fk === "u" && tk === "s" && ts > fs;
}

// Is `type` laid out the same in both tables, all the way down? Without `pointers`, a type that
// holds a string or a view is not: only a value with nothing that points into memory is copied as
// one run of bytes, its padding with it.
function sameAllDown(a, b, type, memo, pointers = false) {
  if (memo.has(type)) return memo.get(type);
  memo.set(type, true);   // a type can't hold itself by value: this only stops a broken table looping
  const ka = kind(a, type), kb = kind(b, type);
  let same = ka === kb && (pointers || ka !== "pointer");
  if (same && ka === "array") same = sameAllDown(a, b, ARRAY.exec(type)[2], memo, pointers);
  else if (same && ka === "struct") {
    const x = a.structs.get(type), y = b.structs.get(type);
    same = x.size === y.size && x.fields.length === y.fields.length &&
      x.fields.every((f, i) => { const g = y.fields[i]; return f.name === g.name && f.offset === g.offset && f.size === g.size && f.type === g.type && sameAllDown(a, b, f.type, memo, pointers); });
  } else if (same && ka === "enum") {
    const x = a.enums.get(type), y = b.enums.get(type);
    same = x.base === y.base && x.size === y.size && x.members.size === y.members.size && [...x.members].every(([n, v]) => y.members.get(n) === v);
  } else if (same && ka === "sum") {
    const x = a.sums.get(type), y = b.sums.get(type);
    same = x.size === y.size && x.payload === y.payload && x.variants.length === y.variants.length && x.variants.every((v, i) => v === y.variants[i] && sameAllDown(a, b, v, memo, pointers));
  }
  memo.set(type, same);
  return same;
}
// How many fields a value of `type` holds (an array's elements are one field each of the element's).
function leaves(layout, type, memo) {
  if (memo.has(type)) return memo.get(type);
  let n = 1; const k = kind(layout, type);
  if (k === "array") n = leaves(layout, ARRAY.exec(type)[2], memo);
  else if (k === "struct") { n = 0; for (const f of layout.structs.get(type).fields) n += leaves(layout, f.type, memo); }
  memo.set(type, n);
  return n;
}

// The plan to carry a state from one table to another: what to copy and what the reload reports.
//   ops      [kind, from, to, a, b]: 0 copy a bytes; 1 number of type a to type b; 2 enum a by map b
//   kept     fields carried; added, dropped, reset (a changed type, with why), resized: their paths
//   pointers strings and views, which take their defaults on every carry
//   same     the state's layout is unchanged: the old bytes are the new state where they are
// null when either table has no struct of its state's size.
export function planState(oldText, oldSize, newText, newSize) {
  const a = parseLayout(oldText), b = parseLayout(newText);
  const from = stateRoot(a, oldSize), to = stateRoot(b, newSize, from);
  if (!from || !to) return null;
  const plan = { root: to, size: newSize, ops: [], maps: [], kept: 0, added: [], dropped: [], reset: [], resized: [], pointers: [] };
  const same = new Map(), count = new Map(), said = new Set();
  const say = (list, text) => { if (!said.has(text)) { said.add(text); list.push(text); } };
  const copy = (src, dst, n) => {
    const last = plan.ops[plan.ops.length - 1];
    if (last && last[0] === 0 && last[1] + last[3] === src && last[2] + last[3] === dst) last[3] += n;
    else plan.ops.push([0, src, dst, n, 0]);
  };
  // `first`: the first element of every array on the way here, the one whose fields are counted and named
  const walk = (ta, tb, sa, sb, src, dst, path, first) => {
    const ka = kind(a, ta), kb = kind(b, tb);
    if (ta === tb && sa === sb && sameAllDown(a, b, ta, same)) { copy(src, dst, sa); if (first) plan.kept += leaves(b, tb, count); return; }
    if (ka === "pointer" || kb === "pointer") { say(plan.pointers, path); return; }
    if (ka === "array" && kb === "array") {
      const [, na, ea] = ARRAY.exec(ta), [, nb, eb] = ARRAY.exec(tb);
      const wa = sa / na, wb = sb / nb, n = Math.min(+na, +nb);
      if (na !== nb) say(plan.resized, `${path}: [${na}] to [${nb}]`);
      if (ea === eb && wa === wb && sameAllDown(a, b, ea, same)) { copy(src, dst, n * wa); if (first) plan.kept += leaves(b, eb, count); return; }
      for (let i = 0; i < n; i++) walk(ea, eb, wa, wb, src + i * wa, dst + i * wb, +na === 1 && +nb === 1 ? path : path + "[]", first && i === 0);
      return;
    }
    if (ka === "struct" && kb === "struct" && ta === tb) {
      const olds = new Map(a.structs.get(ta).fields.map((f) => [f.name, f]));
      for (const f of b.structs.get(tb).fields) {
        const o = olds.get(f.name), p = path ? `${path}.${f.name}` : f.name;
        if (o) walk(o.type, f.type, o.size, f.size, src + o.offset, dst + f.offset, p, first);
        else say(plan.added, p);
        olds.delete(f.name);
      }
      for (const name of olds.keys()) say(plan.dropped, path ? `${path}.${name}` : name);
      return;
    }
    if (ka === "number" && kb === "number") {
      const equal = NUMBERS[ta][0] === NUMBERS[tb][0] && NUMBERS[ta][1] === NUMBERS[tb][1];   // int is s64, uint u64
      if (equal) copy(src, dst, sa);
      else if (widens(ta, tb)) plan.ops.push([1, src, dst, ta, tb]);
      else { say(plan.reset, `${path}: ${ta} to ${tb}`); return; }
      if (first) plan.kept += 1;
      return;
    }
    if (ka === "enum" && kb === "enum" && ta === tb) {
      const x = a.enums.get(ta), y = b.enums.get(tb), map = new Map();
      for (const [name, v] of x.members) if (y.members.has(name)) map.set(v, y.members.get(name));
      plan.maps.push(map); plan.ops.push([2, src, dst, x.base, y.base, plan.maps.length - 1]);
      if (first) plan.kept += 1;
      return;
    }
    say(plan.reset, ta === tb ? `${path}: ${tb} (it holds a pointer, or its layout changed)` : `${path}: ${ta} to ${tb}`);
  };
  walk(from, to, oldSize, newSize, 0, 0, "", true);
  plan.same = from === to && oldSize === newSize && sameAllDown(a, b, from, new Map(), true);
  return plan;
}

function read(view, at, type) {
  const [size, k] = NUMBERS[type];
  if (k === "f") return size === 4 ? view.getFloat32(at, true) : view.getFloat64(at, true);
  if (size === 8) return k === "s" ? view.getBigInt64(at, true) : view.getBigUint64(at, true);
  if (k === "s") return BigInt(size === 1 ? view.getInt8(at) : size === 2 ? view.getInt16(at, true) : view.getInt32(at, true));
  return BigInt(size === 1 ? view.getUint8(at) : size === 2 ? view.getUint16(at, true) : view.getUint32(at, true));
}
function write(view, at, type, value) {
  const [size, k] = NUMBERS[type];
  if (k === "f") { if (size === 4) view.setFloat32(at, value, true); else view.setFloat64(at, value, true); return; }
  if (size === 8) { if (k === "s") view.setBigInt64(at, value, true); else view.setBigUint64(at, value, true); return; }
  const n = Number(value);
  if (size === 1) view.setUint8(at, n & 0xff); else if (size === 2) view.setUint16(at, n & 0xffff, true); else view.setUint32(at, n >>> 0, true);
}

// Carries `old` (the old state's bytes, copied out before the new engine was instantiated) into
// the new state at `base` of `buffer`, which already holds its defaults.
export function applyState(plan, old, buffer, base) {
  const from = new DataView(old.buffer, old.byteOffset, old.byteLength), to = new DataView(buffer, base, plan.size);
  const bytes = new Uint8Array(buffer, base, plan.size);
  for (const [op, src, dst, x, y, map] of plan.ops) {
    if (op === 0) bytes.set(old.subarray(src, src + x), dst);
    else if (op === 1) write(to, dst, y, read(from, src, x));
    else { const v = plan.maps[map].get(read(from, src, x)); if (v !== undefined) write(to, dst, y, v); }
  }
}

// The reload's report as one line for the dev note.
export function stateNote(plan) {
  const list = (what, items) => (items.length ? `; ${what} ${items.slice(0, 6).join(", ")}${items.length > 6 ? ` and ${items.length - 6} more` : ""}` : "");
  return `state kept: ${plan.kept} field${plan.kept === 1 ? "" : "s"}` + list("new", plan.added) + list("dropped", plan.dropped) + list("reset", plan.reset) + list("resized", plan.resized) +
    (plan.pointers.length ? `; ${plan.pointers.length} pointer field${plan.pointers.length === 1 ? "" : "s"} at ${plan.pointers.length === 1 ? "its default" : "their defaults"}` : "");
}
