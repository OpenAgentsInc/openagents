// Strict producer capture: raw exports and exact payload bytes, without count clipping.
// Derived from verse/native/wasm-run.ts; the same
// host steps in the same order (coax/web/host.js loadEngine, unpack and
// deliver, without the GPU), so the two can be compared line for line.
//   bun verse/native/wasm-run.ts REALM.wasm [realm-run options]
// Network imports are refused as realm-host.c refuses them;
// VERSE_NATIVE_CLOCK_MS fixes the clock for both.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
const sha = (b: Uint8Array) => createHash("sha256").update(b).digest("hex");

const [wasmPath, ...rest] = process.argv.slice(2);
const opt: Record<string, string> = {};
for (let i = 0; i < rest.length; i++) {
  const a = rest[i];
  if (a === "--golden") opt.golden = "1";
  else opt[a.slice(2)] = rest[++i];
}
const frames = Number(opt.frames ?? 120), every = Number(opt.every ?? 30);
const [w, h] = (opt.size ?? "1280x720").split("x").map(Number);
const scale = Number(opt.scale ?? 1);

// coax/web/wasm.sh links with --max-memory=67108864 (1024 pages); the module asks for at least its own data.
// As host.js: 128 pages, grown to the initial pages the module's memory import declares (a module
// with large static data, the homepage's, declares more), so the arena lands where the browser's does.
const memory = new WebAssembly.Memory({ initial: 128, maximum: 1024 });
function importedMemoryPages(b: Uint8Array): number {
  let i = 8;
  const leb = () => { let v = 0, shift = 0, c; do { c = b[i++]; v += (c & 0x7f) * 2 ** shift; shift += 7; } while (c & 0x80); return v; };
  const skipName = () => { const n = leb(); i += n; };
  while (i < b.length) {
    const id = b[i++], size = leb(), end = i + size;
    if (id === 2) {
      for (let n = leb(); n > 0; n--) {
        skipName(); skipName();
        const kind = b[i++];
        if (kind === 0) leb();
        else if (kind === 1) { i++; const f = leb(); leb(); if (f & 1) leb(); }
        else if (kind === 2) { leb(); return leb(); }
        else if (kind === 3) { i += 2; }
        else return 0;
      }
      return 0;
    }
    i = end;
  }
  return 0;
}
const clock = () => BigInt(opt.clock ?? 0);
const env: Record<string, unknown> = {
  memory,
  coaxial_now_ms: clock,
  coaxial_now_seconds: () => clock() / 1000n,
  coaxial_http: () => -1,
  coaxial_http_bounded: () => -1,
  coaxial_http_cancel: () => 0,
  host_line: () => {}, host_dot: () => {}, host_slot: () => {},
  coaxial_socket_open: () => -1, coaxial_socket_send: () => {}, coaxial_socket_close: () => {},
  // byte storage refused (UNAVAILABLE), as natively (realm-host.c)
  coaxial_storage_put: () => 2, coaxial_storage_get: () => 2, coaxial_storage_delete: () => 2, coaxial_storage_cancel: () => 0,
};
const wasmBytes = new Uint8Array(readFileSync(wasmPath));
const module = await WebAssembly.compile(wasmBytes);
{ const pages = importedMemoryPages(wasmBytes), have = memory.buffer.byteLength / 65536; if (pages > have) memory.grow(pages - have); }
const imports: Record<string, Record<string, unknown>> = { env };
for (const imp of WebAssembly.Module.imports(module)) {
  if (imp.kind !== "function") continue;
  const into = (imports[imp.module] ||= {});
  if (into[imp.name]) continue;
  if (imp.module === "wasi_snapshot_preview1" && ["fd_fdstat_get", "fd_seek", "fd_close", "clock_time_get"].includes(imp.name)) into[imp.name] = () => 52;
  else if (imp.module === "wasi_snapshot_preview1" && imp.name === "fd_write") into[imp.name] = () => 0;
  else into[imp.name] = () => { throw new Error(`wasm-run: ${imp.module}.${imp.name} is not provided`); };
}
const instance = await WebAssembly.instantiate(module, imports);
const x = instance.exports as Record<string, any>;
x._initialize?.();
const align = (n: number, a: number) => Math.ceil(n / a) * a;
const heapBase = x.__heap_base?.value ?? 0;
const floats = x.coaxial_vertex_floats(), maxVertices = x.coaxial_max_vertices(), uniformFloats = x.coaxial_uniform_floats();
const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
const stateCap = align(Math.max(x.coaxial_state_size(), 1024), 16);
const total = base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64;
if (total > memory.buffer.byteLength) memory.grow(Math.ceil((total - memory.buffer.byteLength) / 65536));
const state = base, vertices = base + stateCap, uniforms = vertices + maxVertices * floats * 4;
x.coaxial_state_init?.(state); // struct defaults, as the browser host applies them
if (opt.golden) x.coaxial_golden?.(1);
x.coaxial_page?.(state, Number(opt.page ?? 0)); // data-page (the galaxy: the verse app's 1), before the facts, as host.js

const bytesAt = (p: number, n: number) => new Uint8Array(memory.buffer, p, n);
const after = () => { if (x.coaxial_action?.(state)) x.coaxial_action_done(state); };
const sayAfter = () => {
  const a = x.coaxial_action?.(state);
  if (!a) return;
  // Acknowledge platform requests without logging their payloads.
  x.coaxial_action_done(state);
};
const send = (kind: number, text: Uint8Array) => {
  if (text.length > x.coaxial_inbox_cap()) throw new Error("capture input exceeds the producer inbox");
  const b = text;
  bytesAt(x.coaxial_inbox(state), b.length).set(b);
  x.coaxial_message(state, kind, b.length);
  after();
};

// The pack, as realm-host.c rh_pack reads it (none without --pack, as a page that names none).
const pack = opt.pack ? readFileSync(opt.pack) : new Uint8Array(32);
const v = new DataView(pack.buffer, pack.byteOffset, pack.length);
const count = v.getUint32(4, true), table = v.getUint32(16, true);
const entries = Array.from({ length: count }, (_, k) => {
  const at = table + k * 48;
  if (at + 48 > pack.length) throw new Error("truncated pack table");
  const begin = v.getUint32(at + 4, true), length = v.getUint32(at + 8, true);
  if (begin + length > pack.length) throw new Error("truncated pack payload");
  return { kind: pack[at], bytes: pack.subarray(v.getUint32(at + 4, true), v.getUint32(at + 4, true) + v.getUint32(at + 8, true)) };
});
const of = (kind: number) => entries.find((e) => e.kind === kind);
const layers = entries.filter((e) => e.kind === 4).length;
// --fact K=V adds one fact to the file's (a realm's data-ruleset, say)
if (opt.facts) send(600, new TextEncoder().encode(readFileSync(opt.facts, "utf8").replace(/\n+$/, "") + (opt.fact ? `\n${opt.fact}` : "")));
send(606, new Uint8Array());
const first = opt.pack ? 4 : 0;
if (opt.pack) {
  send(605, new TextEncoder().encode(`${pack.length} ${pack.length}`));
  const catalog = of(1)!;
  const at = x.coaxial_asset(state, 0, catalog.bytes.length);
  if (at) bytesAt(at, catalog.bytes.length).set(catalog.bytes);
  [catalog.bytes.length, of(2)?.bytes.length ?? 0, of(3)?.bytes.length ?? 0, layers].forEach((n, i) => x.coaxial_asset_ready(state, i, n));
}
(opt.extra ? opt.extra.split(",").map(Number) : []).forEach((n, i) => x.coaxial_asset_ready?.(state, first + i, n));
console.log(`realm-run: ${count} entries in the pack (${layers} layers), ${floats} floats a vertex, ${uniformFloats} uniforms`);
// --hash PLACE: the page's location hash, as host.js sendHash delivers it once the engine is up
if (opt.hash && x.coaxial_hash) {
  const b = new TextEncoder().encode(opt.hash);
  if (b.length > x.coaxial_inbox_cap()) throw new Error("capture hash exceeds producer inbox");
  bytesAt(x.coaxial_inbox(state), b.length).set(b);
  x.coaxial_hash(state, b.length);
  after();
}

// Observe store scheduling once, in the same place as the browser upload loop.
// A zero update keeps the previously initialized GPU bytes; it does not empty a resource.
const storeCount=x.coaxial_stores?.()??0;
const stores=Array.from({length:storeCount},(_,index)=>{
  const capacity=x.coaxial_store_cap(index);
  return {index,capacity,physical_bytes:align(Math.max(capacity,16),16),data:new Uint8Array(align(Math.max(capacity,16),16)),highwater:0,version:undefined as unknown,last_update_bytes:0,last_positive:new Uint8Array(),updates:[] as unknown[]};
});
const particleCap=x.coaxial_particle_cap?.()??0;
const puffs={data:new Uint8Array(particleCap),highwater:0,last_update_bytes:0,last_positive:new Uint8Array(),updates:[] as unknown[]};
const keys=(opt.keys??'').split(',').filter(Boolean).map(k=>k.split(':').map(Number));
const clicks=(opt.pointer??'').split(',').filter(Boolean).map(k=>k.split(':').map(Number));
const rawFrames:unknown[]=[];
let hud=0;
for(let f=1;f<=frames;f++) {
  for(const[kf,key,down]of keys)if(kf===f){(down?x.coaxial_key:x.coaxial_key_up)(state,key,0);sayAfter();}
  for(const[cf,cx,cy]of clicks)if(cf===f)for(const phase of[0,2]){x.coaxial_pointer(state,cx,cy,phase,0,1);sayAfter();}
  hud=x.coaxial_frame(state,vertices,uniforms,w,h,scale,1/60,0);
  if(hud<0||hud>maxVertices)throw new Error('raw HUD count exceeds producer allocation');
  sayAfter();
  const counts={frame:f,hud,draws:x.coaxial_draw_count?.(state)??0,water:x.coaxial_water_draws?.(state)??0,grass:x.coaxial_grass_instances?.(state)??0,particles:x.coaxial_particle_instances?.(state)??0,surface:x.coaxial_surface_vertices?.(state)??0,caster_vertices:x.coaxial_caster_vertices?.(state)??0,caster_instances:x.coaxial_caster_instances?.(state)??0};
  if(counts.draws>0)for(const store of stores){
    const version=x.coaxial_store_version?.(state,store.index);
    if(version!==undefined&&version===store.version)continue;
    const bytes=x.coaxial_store_bytes(state,store.index);
    store.last_update_bytes=bytes;
    if(bytes<0||bytes>store.capacity)throw new Error('raw store update exceeds producer allocation');
    store.updates.push({frame:f,bytes,version:version===undefined?null:String(version)});
    if(bytes){
      const copied=bytesAt(x.coaxial_store(state,store.index),bytes).slice();
      store.data.set(copied);store.last_positive=copied;
      store.highwater=Math.max(store.highwater,bytes);store.version=version;
    }
  }
  if(counts.particles>0&&x.coaxial_particle_bytes){
    const bytes=x.coaxial_particle_bytes(state);puffs.last_update_bytes=bytes;
    if(bytes<0||bytes>particleCap)throw new Error('raw particle update exceeds producer allocation');
    puffs.updates.push({frame:f,bytes});
    if(bytes){const copied=bytesAt(x.coaxial_particles(state),bytes).slice();puffs.data.set(copied);puffs.last_positive=copied;puffs.highwater=Math.max(puffs.highwater,bytes);}
  }
  rawFrames.push(counts);
  if(f%every===0||f===frames)console.log(JSON.stringify(counts));
}
if(opt.dump){
  mkdirSync(opt.dump,{recursive:true});
  const files:Record<string,unknown>={};
  const write=(name:string,b:Uint8Array)=>{writeFileSync(`${opt.dump}/${name}`,b);files[name]={bytes:b.length,sha256:sha(b)};};
  const last=rawFrames.at(-1)as any;
  write('u.bin',bytesAt(uniforms,uniformFloats*4));
  write('hud.bin',bytesAt(vertices,hud*floats*4));
  write('draws.bin',last.draws?bytesAt(x.coaxial_draws(state),last.draws*16):new Uint8Array());
  writeFileSync(`${opt.dump}/draws.txt`,`${last.draws} ${last.water} ${last.grass} ${last.particles} ${hud} ${floats} ${last.surface} ${last.caster_vertices} ${last.caster_instances}\n`);
  for(const store of stores){write(`s${store.index}.bin`,store.data);write(`s${store.index}.upload.bin`,store.last_positive);}
  if(particleCap){write('puffs.bin',puffs.data);write('puffs.upload.bin',puffs.last_positive);}
  if(of(2))write('vertices.bin',of(2)!.bytes);
  if(of(3))write('index.bin',of(3)!.bytes);
  for(const[option,file]of[['index','index.bin'],['vertices','vertices.bin'],['mesh','mesh.bin']])if(opt[option])write(file,readFileSync(opt[option]));
  const images=entries.filter(e=>e.kind===4);images.forEach((e,i)=>write(`layer-${i}.img`,e.bytes));
  writeFileSync(`${opt.dump}/layers.bin.list`,images.map((_,i)=>`layer-${i}.img\n`).join(''));
  const inputs:Record<string,unknown>={};
  for(const k of['pack','facts','index','vertices','mesh'])if(opt[k]){const b=readFileSync(opt[k]);inputs[k]={path:opt[k],bytes:b.length,sha256:sha(b)};}
  const storeMeta=stores.map(({data,last_positive,version,...metadata})=>({...metadata,retained_bytes:data.length,last_positive_bytes:last_positive.length,version:version===undefined?null:String(version),file:`s${metadata.index}.bin`}));
  const{data:pd,last_positive:pp,...pm}=puffs;
  writeFileSync(`${opt.dump}/capture.json`,JSON.stringify({schema:'aethyr.render.producer-capture.v1',status:'captured-unadmitted',wasm:{path:wasmPath,bytes:wasmBytes.length,sha256:sha(wasmBytes)},tool:{path:process.argv[1],sha256:sha(readFileSync(process.argv[1]))},options:opt,inputs,counts:last,frames:rawFrames,stores:storeMeta,particles:{...pm,capacity:particleCap,retained_bytes:pd.length,last_positive_bytes:pp.length},uniform_floats:uniformFloats,vertex_stride:floats,max_vertices:maxVertices,memory_bytes:memory.buffer.byteLength,files},null,2)+'\n');
}
