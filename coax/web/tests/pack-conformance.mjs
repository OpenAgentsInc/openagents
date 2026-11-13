// Firm reason: 1 (drives the browser SDK module coax/web/pack-host.js: fetch with Range, DecompressionStream and the pack reactor's Wasm)
// The browser's driver of the host core's conformance suite (metaverse-engine/host/core/conformance.coax,
// GitHub #13304): the browser's pack module reads the suite's pack as it is (openPackTable) and
// through its paged file (loadPaged), and prints each entry and the two cells in the lines the C
// core's driver prints. It judges nothing: the Coax tool compares.
//   bun coax/web/tests/pack-conformance.mjs conformance.pack conformance.vpk3 host-packs.wasm KEY OTHER_KEY
import { readFileSync } from "node:fs";
const [plain, paged, wasm, key, other] = process.argv.slice(2);
const pack = readFileSync(plain), file = readFileSync(paged), reactor = readFileSync(wasm);
globalThis.fetch = async (url, init = {}) => {
  if (String(url).endsWith("host-packs.wasm")) return new Response(reactor);
  const m = /^bytes=(\d+)-(\d+)$/.exec(init.headers?.Range || "");
  if (!m) return new Response(file, { status: 200 });
  const from = Number(m[1]), to = Math.min(Number(m[2]), file.length - 1);
  if (from > to) return new Response(null, { status: 416 });
  return new Response(file.subarray(from, to + 1), { status: 206, headers: { "content-range": `bytes ${from}-${to}/${file.length}`, etag: '"fixture"' } });
};
const fnv = bytes => { let h = 2166136261; for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0; return h >>> 0; };
const say = entries => entries.forEach((e, k) => console.log(`entry ${k} kind ${e.kind} bytes ${e.bytes.length} hash ${fnv(e.bytes)}`));
const { openPackHost, openPackTable } = await import("../pack-host.js");
say((await openPackTable()).table(new Uint8Array(pack)));
const host = await openPackHost();
try {
  const result = await host.loadPaged({ url: "http://fixture.invalid/conformance.vpk3" }, { owner: "check", demand: true });
  await result.rest(); await result.complete;
  say(result.entries);
  for (const k of [key, other]) {
    const cell = result.cell(Number(k));
    console.log(cell ? `cell ${k} digest ${cell.digest} ranges ${cell.ranges.map(r => r.join(",")).join(" ")}` : `cell ${k} none`);
  }
  result.close();
} catch (error) {
  // the full admission (pack_read, in the reactor) refused the image
  console.log("paged refused");
}
