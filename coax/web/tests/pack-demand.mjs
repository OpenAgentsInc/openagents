// Firm reason: 1 (drives the browser SDK module coax/web/pack-host.js: fetch with Range, DecompressionStream and the pack reactor's Wasm)
// The driver of metaverse-engine/host/check-pack-cells.coax (GitHub #13304): it opens a VPK3 file through
// pack-host.js's loadPaged with demand:true, over a fetch that answers Range requests from the
// file, runs the steps it is given and prints what Coax holds after each. It judges nothing: the
// Coax tool wrote the file, knows its pages and compares these lines with what the reader says.
//   bun coax/web/tests/pack-demand.mjs FILE.vpk3 host-packs.wasm STEP...
//     probe:AT,N[;AT,N...]   the image ranges whose readiness is printed after every step
//     cell:KEY               print the cell's digest and ranges, then fetch them (more)
//     more:AT,N[;AT,N...]    fetch these ranges
//     rest                   fetch everything left
import { readFileSync } from "node:fs";
const [file, wasm, ...steps] = process.argv.slice(2);
const pack = readFileSync(file), reactor = readFileSync(wasm);
let requests = 0;
globalThis.fetch = async (url, init = {}) => {
  if (String(url).endsWith("host-packs.wasm")) return new Response(reactor);
  const m = /^bytes=(\d+)-(\d+)$/.exec(init.headers?.Range || "");
  if (!m) return new Response(pack, { status: 200 });
  const from = Number(m[1]), to = Math.min(Number(m[2]), pack.length - 1);
  if (from > to) return new Response(null, { status: 416 });
  requests++;
  return new Response(pack.subarray(from, to + 1), { status: 206, headers: { "content-range": `bytes ${from}-${to}/${pack.length}`, etag: '"fixture"' } });
};
const pairs = text => text.split(";").filter(Boolean).map(p => p.split(",").map(Number));
const { openPackHost } = await import("../pack-host.js");
const host = await openPackHost();
const result = await host.loadPaged({ url: "http://fixture.invalid/fixture.vpk3" }, { owner: "check", demand: true });
let probes = [];
const held = what => console.log(`${what} held ${probes.map(([at, n]) => (result.ready({ at, bytes: { length: n } }) ? 1 : 0)).join("")} requests ${requests} complete ${result.complete ? "set" : "none"}`);
console.log(`open pages ${result.pages} image ${result.bytes} entries ${result.entries.length}`);
for (const step of steps) {
  const [verb, rest = ""] = [step.slice(0, step.indexOf(":") < 0 ? step.length : step.indexOf(":")), step.indexOf(":") < 0 ? "" : step.slice(step.indexOf(":") + 1)];
  if (verb === "probe") { probes = pairs(rest); held("probe"); continue; }
  if (verb === "cell") {
    const cell = result.cell(Number(rest));
    if (!cell) { console.log(`cell ${rest} none`); continue; }
    console.log(`cell ${rest} digest ${cell.digest} ranges ${cell.ranges.map(r => r.join(",")).join(";")}`);
    await result.more(cell.ranges); held(`cell ${rest}`); continue;
  }
  if (verb === "more") { try { await result.more(pairs(rest)); held("more"); } catch (error) { console.log(`more refused ${error.message}`); } continue; }
  if (verb === "rest") { await result.rest(); await result.complete; held("rest"); continue; }
  console.log(`unknown ${step}`);
}
result.close();
