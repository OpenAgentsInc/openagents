// Firm reason: 2 (independent effect oracle for the actual browser texture upload scheduler).
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const host = fs.readFileSync('coax/web/host.js', 'utf8');
const start = host.indexOf('async function layerTexture(images)');
const end = host.indexOf('function sceneLayout()', start);
assert(start >= 0 && end > start);
async function observe(fail = false) {
  let active = 0, peak = 0, decoded = 0, closed = 0, destroyed = false;
  const copies = [], images = Array.from({length: 5}, (_, i) => new Uint8Array([i]));
  const tick = () => new Promise(resolve => setImmediate(resolve));
  const image = (id, source = true) => { let done = false; return {id, width: 4, height: 4, close() { if (!done) {done = true;if (source) {active--;closed++;}}}}; };
  const context = vm.createContext({
    Uint8Array, Math, Promise, Error,
    GPUTextureUsage: {TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4},
    engine: {exports: {}}, arena: {state: 0}, loading: {textures() {}},
    gpu: {device: {
      createTexture: () => ({destroy() {destroyed = true;}}),
      queue: {copyExternalImageToTexture: ({source}, target, size) => copies.push([source.id, target.origin[2], target.mipLevel, ...size]), onSubmittedWorkDone: tick},
    }},
    layerBitmap: async bytes => { await tick(); if (fail && bytes[0] === 2) throw Error('decode refused'); active++;peak = Math.max(peak, active);decoded++;return image(bytes[0]); },
    createImageBitmap: async source => {await tick();return image(source.id, false);},
  });
  vm.runInContext(host.slice(start, end), context);
  let error;
  try {await context.layerTexture(images);} catch (e) {error = e;}
  assert.equal(active, 0, 'all source bitmaps close before settling');
  assert.equal(decoded, closed);
  return {peak, destroyed, copies, error};
}
(async () => {
  const ok = await observe();assert.equal(ok.error, undefined);assert.equal(ok.destroyed, false);
  assert.equal(ok.peak, 2, 'pipeline decoding with at most two live atlases');
  assert.deepEqual(ok.copies.sort((a,b)=>a[1]-b[1]||a[2]-b[2]), Array.from({length:5}, (_,layer)=>[0,1,2].map(mip=>[layer,layer,mip,4>>mip,4>>mip])).flat());
  const refused = await observe(true);assert.match(refused.error.message, /decode refused/);assert.equal(refused.destroyed, true);assert(refused.peak <= 2);
  console.log('PASS bounded atlas pipeline: two live sources, every layer/mip unchanged, cleanup before refusal');
})().catch(e => {console.error(e);process.exitCode = 1;});
