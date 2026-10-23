import { AssetSourceStore } from "../src/asset-source-store.ts";
import { AssetImportClient, AssetImportSession } from "../src/asset-import-client.ts";
import { AssetRuntimeAdapter } from "../src/asset-runtime-adapter.ts";
import { MAX_COMPILED_CACHE_BYTES, MAX_SOURCE_COUNT, MAX_SOURCE_STORE_BYTES } from "../src/asset-import-protocol.ts";
import { sourceBytesHash } from "../src/protocol.ts";
import { initialProject } from "../src/project.ts";
import { houseGeometry } from "../src/scene.ts";

const assert = (condition, message) => { if (!condition) throw new Error(message); };
const equal = (actual, expected, message) => assert(JSON.stringify(actual) === JSON.stringify(expected),
  `${message}: ${JSON.stringify(actual).slice(0, 240)} !== ${JSON.stringify(expected).slice(0, 240)}`);
const arrayEqual = (actual, expected, message) => {
  assert(actual.length === expected.length, `${message}: length ${actual.length} !== ${expected.length}`);
  for (let index = 0; index < actual.length; index++) assert(actual[index] === expected[index],
    `${message}: item ${index}, ${actual[index]} !== ${expected[index]}`);
};
const bytesOf = text => Uint8Array.from(atob(text), character => character.charCodeAt(0));
const status = document.getElementById("status");
const report = { checks: [], imports: [], gpuErrors: [], workerErrors: [], stages: [], passed: false };
window.assetImportAcceptanceReport = report;
let device, environment, canvasContext, projectId;
const native = { buffers: new Map(), textures: new Map(), workers: new Set(),
  createdBuffers: 0, destroyedBuffers: 0, createdTextures: 0, destroyedTextures: 0, createdWorkers: 0, terminatedWorkers: 0 };
function nativeCounts() {
  return { buffers: native.buffers.size, textures: native.textures.size, workers: native.workers.size };
}
function observeDevice() {
  for (const [method, handles, created, destroyed] of [
    ["createBuffer", native.buffers, "createdBuffers", "destroyedBuffers"],
    ["createTexture", native.textures, "createdTextures", "destroyedTextures"],
  ]) {
    const create = device[method].bind(device);
    device[method] = descriptor => {
      const handle = create(descriptor), destroy = handle.destroy.bind(handle);
      handles.set(handle, { label: descriptor.label ?? "", ...(method === "createBuffer" ? { bytes: descriptor.size } : {}) });
      native[created]++;
      handle.destroy = () => {
        const result = destroy();
        if (handles.delete(handle)) native[destroyed]++;
        return result;
      };
      return handle;
    };
  }
}
function disposed(name) {
  equal(environment.runtime.stats(), { assets: 0, buffers: 0, textures: 0 }, `${name}: logical resource counts`);
  equal(nativeCounts(), { buffers: 0, textures: 0, workers: 0 }, `${name}: actual destroy and terminate calls`);
  report.checks.push(`${name}: all created GPU handles destroyed and the real worker terminated`);
}

function check(name, condition) {
  assert(condition, name);
  report.checks.push(name);
}
function stage(name) {
  report.stages.push({ name, elapsedMs: performance.now() - report.started });
  status.textContent = `${name}\n${report.checks.length} checks passed`;
}
function accepted(outcome, name) {
  assert(outcome.type === "accepted", `${name}: ${outcome.type}${outcome.message ? `: ${outcome.message}` : ""}`);
  return outcome;
}
async function imported(id, kind, path, bytes, name) {
  const began = performance.now();
  const result = await environment.session.importAsset(id, kind, path, bytes);
  report.imports.push({ name, type: result.type, elapsedMs: performance.now() - began,
    ...(result.metrics ? { metrics: result.metrics } : {}),
    ...(result.message ? { message: result.message, source: result.source } : {}),
    rebuilds: result.rebuilds?.map(item => ({ id: item.id, type: item.type, metrics: item.metrics, message: item.message })) ?? [] });
  return result;
}
function openEnvironment(store) {
  const worker = new Worker(new URL("../src/asset-import-worker.ts", import.meta.url), { type: "module" });
  native.workers.add(worker); native.createdWorkers++;
  const terminate = worker.terminate.bind(worker);
  worker.terminate = () => {
    const result = terminate();
    if (native.workers.delete(worker)) native.terminatedWorkers++;
    return result;
  };
  const transfers = { messages: 0, uploads: 0, transferredBytes: 0, detachedBuffers: 0, compiled: 0 };
  const control = { afterCompile: undefined };
  const post = worker.postMessage.bind(worker);
  worker.postMessage = (message, transfer) => {
    const buffers = (transfer ?? []).filter(value => value instanceof ArrayBuffer);
    const sizes = buffers.map(buffer => buffer.byteLength);
    transfers.messages++;
    if (message.type === "put") transfers.uploads++;
    transfers.transferredBytes += sizes.reduce((total, size) => total + size, 0);
    post(message, transfer);
    transfers.detachedBuffers += buffers.filter(buffer => buffer.byteLength === 0).length;
    assert(buffers.every(buffer => buffer.byteLength === 0), "Real worker uploads transfer their copied buffers.");
    if (message.type === "compile" && control.afterCompile) {
      const action = control.afterCompile;
      control.afterCompile = undefined;
      queueMicrotask(action);
    }
  };
  worker.addEventListener("message", event => { if (event.data.type === "compiled") transfers.compiled++; });
  worker.addEventListener("error", event => report.workerErrors.push(event.message));
  const client = new AssetImportClient(worker);
  const runtime = new AssetRuntimeAdapter(device);
  const session = new AssetImportSession(store, client, runtime);
  return { worker, client, runtime, session, store, transfers, control };
}
async function idle() {
  for (let count = 0; count < 100; count++) {
    const value = await environment.client.stats();
    if (value.active === 0) return value;
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  throw new Error("The real import worker did not finish its cancelled work.");
}
async function bufferBytes(buffer, size) {
  const readback = device.createBuffer({ size, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  try {
    const encoder = device.createCommandEncoder();
    encoder.copyBufferToBuffer(buffer, 0, readback, 0, size);
    device.queue.submit([encoder.finish()]);
    await readback.mapAsync(GPUMapMode.READ);
    return new Uint8Array(readback.getMappedRange()).slice();
  } finally { readback.unmap(); readback.destroy(); }
}
async function imageBytes(image) {
  const bytesPerRow = Math.ceil(image.width * 4 / 256) * 256;
  const readback = device.createBuffer({ size: bytesPerRow * image.height,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  try {
    const encoder = device.createCommandEncoder();
    encoder.copyTextureToBuffer({ texture: image.texture }, { buffer: readback, bytesPerRow, rowsPerImage: image.height },
      [image.width, image.height]);
    device.queue.submit([encoder.finish()]);
    await readback.mapAsync(GPUMapMode.READ);
    const mapped = new Uint8Array(readback.getMappedRange()), result = new Uint8Array(image.width * image.height * 4);
    for (let row = 0; row < image.height; row++) result.set(mapped.subarray(row * bytesPerRow, row * bytesPerRow + image.width * 4), row * image.width * 4);
    return result;
  } finally { readback.unmap(); readback.destroy(); }
}
async function meshMatches(id, positions) {
  const mesh = environment.runtime.resources(id)?.meshes[0];
  assert(mesh, `${id}: imported GPU mesh is present.`);
  const vertex = await bufferBytes(mesh.vertex, positions.length * 4);
  equal([...new Float32Array(vertex.buffer)], positions, `${id}: actual GPU vertex readback`);
  const index = await bufferBytes(mesh.index, 12);
  equal([...new Uint32Array(index.buffer)], [0, 1, 2], `${id}: actual GPU index readback`);
  check(`${id}: GPU mesh bytes match`, mesh.indexCount === 3);
}
async function imageMatches(id, pixels) {
  const image = environment.runtime.resources(id)?.images[0];
  assert(image, `${id}: imported GPU image is present.`);
  equal([image.width, image.height], [2, 2], `${id}: image size`);
  equal([...await imageBytes(image)], pixels, `${id}: actual GPU texture readback`);
  check(`${id}: GPU texture bytes match`, true);
}
function sameSelected(id, before, name) {
  const after = environment.runtime.resources(id);
  check(name, after?.revision === before.revision && after.meshes.length === before.meshes.length &&
    after.images.length === before.images.length && after.meshes.every((mesh, index) => mesh.vertex === before.meshes[index].vertex &&
      mesh.index === before.meshes[index].index && Object.entries(mesh.attributes).every(([name, buffer]) =>
        buffer === before.meshes[index].attributes[name])) && after.images.every((image, index) => image.texture === before.images[index].texture));
}
function selectedResources(ids) {
  const expected = { assets: ids.length, buffers: 0, textures: 0 };
  for (const id of ids) {
    const description = environment.runtime.get(id)?.description;
    assert(description, `${id}: selected description exists.`);
    expected.textures += description.images.length;
    for (const mesh of description.meshes) expected.buffers += 2 + [mesh.normals, mesh.colors, mesh.glow, mesh.uvs].filter(Boolean).length;
  }
  equal(environment.runtime.stats(), expected, "Selected assets account for every tracked GPU resource");
  equal(nativeCounts(), { buffers: expected.buffers, textures: expected.textures, workers: 1 },
    "Selected resources match independently observed native GPU handles and the real worker");
}
async function ownedGeometryMatches(reference, from, to) {
  const description = environment.runtime.get("owned-geometry").description;
  const resources = environment.runtime.resources("owned-geometry");
  const replaced = from.map(Math.fround), replacement = to.map(Math.fround);
  let cursor = 0, changed = 0, glowing = 0;
  for (let index = 0; index < description.meshes.length; index++) {
    const mesh = description.meshes[index], gpu = resources.meshes[index];
    assert(mesh.normals && mesh.colors && mesh.glow, "Haunt geometry includes normals, colors, and glow.");
    for (let at = 0; at < mesh.vertexCount; at++) {
      const raw = (cursor + at) * 10, rgb = reference.subarray(raw + 6, raw + 9);
      const color = rgb.every((value, axis) => value === replaced[axis]) ? replacement : rgb;
      if (color === replacement) changed++;
      if (reference[raw + 9] > 0) glowing++;
      for (let axis = 0; axis < 3; axis++) {
        assert(mesh.positions[at * 3 + axis] === reference[raw + axis], "Haunt positions survive conversion.");
        assert(mesh.normals[at * 3 + axis] === reference[raw + 3 + axis], "Haunt normals survive conversion.");
        assert(mesh.colors[at * 3 + axis] === color[axis], "Palette replacement preserves other Haunt colors.");
      }
      assert(mesh.glow[at] === reference[raw + 9], "Haunt glow survives conversion.");
    }
    for (const [name, values] of [["normal", mesh.normals], ["color", mesh.colors], ["glow", mesh.glow]]) {
      const bytes = await bufferBytes(gpu.attributes[name], values.byteLength);
      arrayEqual(new Float32Array(bytes.buffer), values, `Haunt ${name}: GPU attribute readback`);
    }
    const positions = await bufferBytes(gpu.vertex, mesh.positions.byteLength);
    arrayEqual(new Float32Array(positions.buffer), mesh.positions, "Haunt positions: GPU readback");
    const indices = await bufferBytes(gpu.index, mesh.indices.byteLength);
    const actualIndices = new Uint32Array(indices.buffer);
    arrayEqual(actualIndices, mesh.indices, "Haunt indices: GPU readback");
    for (let at = 0; at < actualIndices.length; at++) assert(actualIndices[at] === at, "Haunt triangle order is preserved.");
    cursor += mesh.vertexCount;
  }
  check("Haunt palette changes real floor colors and preserves nonzero glow", changed > 0 && glowing > 0 && cursor * 10 === reference.length);
  return { vertices: cursor, meshes: description.meshes.length, replacedVertices: changed, glowingVertices: glowing };
}
async function ownedCardMatches(pixels) {
  const description = environment.runtime.get("owned-card").description;
  const resources = environment.runtime.resources("owned-card"), mesh = description.meshes[0], gpu = resources.meshes[0];
  const material = description.materials?.find(item => item.id === mesh.material);
  check("Card material binds the decoded image", material?.baseColorTexture === description.images[0]?.id &&
    material.doubleSided && material.unlit && gpu.material === material.id);
  arrayEqual(mesh.positions, [-1, -1.5, 0, 1, -1.5, 0, 1, 1.5, 0, -1, 1.5, 0], "Card dimensions");
  arrayEqual(mesh.indices, [0, 1, 2, 0, 2, 3], "Card triangles");
  arrayEqual(mesh.uvs, [0, 1, 1, 1, 1, 0, 0, 0], "Card UV coordinates");
  arrayEqual(new Float32Array((await bufferBytes(gpu.vertex, mesh.positions.byteLength)).buffer), mesh.positions, "Card positions: GPU readback");
  arrayEqual(new Uint32Array((await bufferBytes(gpu.index, mesh.indices.byteLength)).buffer), mesh.indices, "Card indices: GPU readback");
  for (const [name, data] of [["normal", mesh.normals], ["uv", mesh.uvs]]) {
    const bytes = await bufferBytes(gpu.attributes[name], data.byteLength);
    arrayEqual(new Float32Array(bytes.buffer), data, `Card ${name}: GPU readback`);
  }
  await imageMatches("owned-card", pixels);
}
async function ownedSources(fixtures, blobs) {
  stage("Importing owned Haunt geometry and an image-backed card");
  const encoder = new TextEncoder(), project = initialProject();
  const reference = houseGeometry(project.entities, undefined, project.layout);
  const from = [0.29, 0.23, 0.22], first = [0.1, 0.8, 0.2], second = [0.8, 0.2, 0.7];
  const palette = to => encoder.encode(JSON.stringify({ version: 1, replacements: [{ from, to }] }));
  const recipe = value => encoder.encode(JSON.stringify({ asset: { version: "2.0" }, extensionsUsed: ["OPENAGENTS_haunt"], extensionsRequired: ["OPENAGENTS_haunt"],
    extensions: { OPENAGENTS_haunt: value } }));
  const geometryBytes = recipe({ version: 1, kind: "geometry", project, palette: { source: "../palettes/house.json" } });
  const cardBytes = recipe({ version: 1, kind: "card", image: "../images/card.png", width: 2, height: 3 });
  await environment.store.put("palette", "gltf", "palettes/house.json", palette(first));
  const geometry = accepted(await imported("owned-geometry", "gltf", "recipes/house.gltf", geometryBytes, "Haunt geometry recipe"), "Haunt geometry");
  equal(geometry.description.sources.map(source => source.id).sort(), ["owned-geometry", "palette"], "Only the geometry recipe and its palette are used");
  report.ownedGeometry = await ownedGeometryMatches(reference, from, first);
  accepted(await imported("card-image", "image", "images/card.png", blobs.image, "Card image"), "Card image");
  const card = accepted(await imported("owned-card", "gltf", "recipes/card.gltf", cardBytes, "Image card recipe"), "Image card");
  equal(card.description.sources.map(source => source.id).sort(), ["card-image", "owned-card"], "Only the card recipe and its image are used");
  await ownedCardMatches(fixtures.firstPixels);
  const unrelated = environment.runtime.resources("unrelated"), triangle = environment.runtime.resources("triangle");
  const beforeGeometry = environment.runtime.resources("owned-geometry"), beforeCard = environment.runtime.resources("owned-card");
  const paletteBegan = performance.now();
  const paletteEdit = await environment.session.updateSource("palette", "gltf", "palettes/house.json", palette(second));
  check("A saved palette edit rebuilds only its geometry user", paletteEdit.type === "stored" && paletteEdit.rebuilds.length === 1 &&
    paletteEdit.rebuilds[0].id === "owned-geometry" && paletteEdit.rebuilds[0].type === "accepted");
  report.ownedPaletteEdit = { mode: "AssetImportSession.updateSource", elapsedMs: performance.now() - paletteBegan,
    revision: paletteEdit.revision, rebuilds: paletteEdit.rebuilds.map(item => ({ id: item.id, type: item.type, metrics: item.metrics })),
    ...(await ownedGeometryMatches(reference, from, second)) };
  check("A palette edit replaces geometry resources", environment.runtime.resources("owned-geometry").meshes[0].vertex !== beforeGeometry.meshes[0].vertex);
  sameSelected("owned-card", beforeCard, "Palette edits keep the unrelated card GPU objects");
  const stableGeometry = environment.runtime.resources("owned-geometry");
  const edited = accepted(await imported("card-image", "image", "images/card.png", blobs.changedImage, "Card image dependency edit"), "Card image edit");
  check("Image edits rebuild only their card user", edited.rebuilds?.length === 1 && edited.rebuilds[0].id === "owned-card" && edited.rebuilds[0].type === "accepted");
  await ownedCardMatches(fixtures.secondPixels);
  sameSelected("owned-geometry", stableGeometry, "Card image edits keep Haunt geometry GPU objects");
  sameSelected("unrelated", unrelated, "Owned edits keep the unrelated mesh GPU objects");
  sameSelected("triangle", triangle, "Owned edits keep the unrelated GLB GPU objects");
  const goodCard = environment.runtime.resources("owned-card");
  const badImage = await imported("card-image", "image", "images/card.png", new Uint8Array([137, 80, 78, 71]), "Invalid card image draft");
  check("Invalid card image fails without promoting its dependent", badImage.type === "failed" &&
    badImage.rebuilds?.some(item => item.id === "owned-card" && item.type === "failed"));
  sameSelected("owned-card", goodCard, "Invalid card images keep the previous card GPU objects");
  const badRecipe = await imported("owned-geometry", "gltf", "recipes/house.gltf", recipe({ version: 2, kind: "geometry", project }), "Invalid geometry recipe draft");
  check("Unsupported owned recipe version is rejected", badRecipe.type === "failed");
  sameSelected("owned-geometry", stableGeometry, "Invalid geometry recipes keep previous attributes");
  const ids = ["unrelated", "image", "triangle", "card-image", "owned-geometry", "owned-card"];
  selectedResources(ids);
  const manifest = await environment.store.list();
  for (const [id, bytes] of [["owned-geometry", geometryBytes], ["owned-card", cardBytes], ["palette", palette(first)], ["card-image", blobs.image]]) {
    const source = manifest.find(item => item.record.id === id);
    arrayEqual(await environment.store.read(source.original), bytes, `${id}: encoded original is retained`);
  }
  report.ownedBeforeReopen = { sources: manifest, resources: environment.runtime.stats(), worker: await idle(), transfers: { ...environment.transfers } };
  environment.session.dispose();
  disposed("Owned session close before reopening");
  environment = openEnvironment(await AssetSourceStore.open(projectId));
  const restored = await environment.session.restore();
  check("Owned historical pins restore alongside ordinary assets", restored.length === ids.length && restored.every(item => item.type === "accepted"));
  equal(await environment.store.list(), manifest, "Owned restore does not overwrite bad latest drafts");
  await ownedGeometryMatches(reference, from, second); await ownedCardMatches(fixtures.secondPixels);
  selectedResources(ids);
  report.ownedReopened = { outcomes: restored.map(item => ({ id: item.id, type: item.type, revision: item.revision, metrics: item.metrics })),
    resources: environment.runtime.stats(), worker: await idle(), transfers: { ...environment.transfers } };
}
async function preview() {
  const canvas = document.getElementById("preview");
  canvasContext = canvas.getContext("webgpu");
  assert(canvasContext, "The fixture canvas supports WebGPU.");
  const format = navigator.gpu.getPreferredCanvasFormat();
  canvasContext.configure({ device, format, alphaMode: "opaque" });
  const shader = device.createShaderModule({ code: `
    @group(0) @binding(0) var image: texture_2d<f32>;
    @vertex fn vertex(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
      return vec4<f32>(position, 1.0);
    }
    @fragment fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
      return textureLoad(image, vec2<i32>(i32(position.x / 200.0), i32(position.y / 150.0)), 0);
    }` });
  const pipeline = await device.createRenderPipelineAsync({ layout: "auto",
    vertex: { module: shader, entryPoint: "vertex", buffers: [{ arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] }] },
    fragment: { module: shader, entryPoint: "fragment", targets: [{ format }] }, primitive: { topology: "triangle-list" } });
  const resources = environment.runtime.resources("triangle");
  const mesh = resources.meshes[0], image = resources.images[0];
  const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: image.texture.createView() }] });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({ colorAttachments: [{ view: canvasContext.getCurrentTexture().createView(),
    clearValue: [0.035, 0.04, 0.065, 1], loadOp: "clear", storeOp: "store" }] });
  pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.setVertexBuffer(0, mesh.vertex);
  pass.setIndexBuffer(mesh.index, "uint32"); pass.drawIndexed(mesh.indexCount); pass.end();
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
  report.preview = { width: canvas.width, height: canvas.height, scope: "Imported GPU position/index buffers and texture in a minimal draw, not the full editor renderer." };
}

window.cleanupAssetImportAcceptance = async () => {
  environment?.session.dispose();
  if (device) await device.queue.onSubmittedWorkDone().catch(() => {});
  const resources = environment?.runtime.stats() ?? { assets: 0, buffers: 0, textures: 0 };
  const observed = nativeCounts();
  report.nativeLifecycle = { createdBuffers: native.createdBuffers, destroyedBuffers: native.destroyedBuffers,
    createdTextures: native.createdTextures, destroyedTextures: native.destroyedTextures,
    createdWorkers: native.createdWorkers, terminatedWorkers: native.terminatedWorkers, beforeDeviceDestroy: observed,
    scope: "Observed native create/destroy/terminate calls, including readback buffers; not a VRAM measurement." };
  try {
    equal(observed, { buffers: 0, textures: 0, workers: 0 }, "Native GPU handles and real workers close before device.destroy");
  } finally { canvasContext?.unconfigure(); device?.destroy(); }
  if (projectId && navigator.storage?.getDirectory) {
    const directory = await (await navigator.storage.getDirectory()).getDirectoryHandle("openagents-asset-sources-v1").catch(() => undefined);
    await directory?.removeEntry(projectId, { recursive: true });
  }
  return { resources, native: observed, nativeLifecycle: report.nativeLifecycle, gpuErrors: report.gpuErrors, workerErrors: report.workerErrors };
};

window.runAssetImportAcceptance = async fixtures => {
  report.started = performance.now();
  projectId = fixtures.projectId;
  const blobs = Object.fromEntries(Object.entries(fixtures.blobs).map(([key, value]) => [key, bytesOf(value)]));
  stage("Opening local files, a real worker, and WebGPU");
  assert(navigator.gpu, "WebGPU is required; no simulated GPU fallback is used.");
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter, "A WebGPU adapter is required.");
  report.adapter = { vendor: adapter.info?.vendor, architecture: adapter.info?.architecture,
    device: adapter.info?.device, description: adapter.info?.description, fallback: adapter.info?.isFallbackAdapter };
  device = await adapter.requestDevice();
  observeDevice();
  device.addEventListener("uncapturederror", event => report.gpuErrors.push(event.error.message));
  const store = await AssetSourceStore.open(projectId);
  environment = openEnvironment(store);
  accepted(await imported("unrelated", "glb", "unrelated.glb", blobs.plain, "Plain indexed triangle"), "Plain triangle baseline");
  await meshMatches("unrelated", fixtures.positions);
  const unrelated = environment.runtime.resources("unrelated");
  accepted(await imported("image", "image", "models/tile.png", blobs.image, "Standalone PNG"), "Standalone PNG");
  await imageMatches("image", fixtures.firstPixels);
  const textured = accepted(await imported("triangle", "glb", "models/triangle.glb", blobs.triangle, "Textured GLB"), "Textured GLB");
  check("GLB records the image it actually uses", textured.description.sources.some(source => source.id === "image") &&
    !textured.description.sources.some(source => source.id === "unrelated"));
  await meshMatches("triangle", fixtures.positions);
  await imageMatches("triangle", fixtures.firstPixels);
  for (const [id, encoded] of [["unrelated", blobs.plain], ["image", blobs.image], ["triangle", blobs.triangle]]) {
    const source = (await store.list()).find(item => item.record.id === id);
    equal([...await store.read(source.original)], [...encoded], `${id}: original encoded bytes`);
    check(`${id}: caller bytes were not detached`, encoded.byteLength > 0);
  }
  stage("Editing an image rebuilds only its users");
  const priorTriangle = environment.runtime.resources("triangle");
  const edited = accepted(await imported("image", "image", "models/tile.png", blobs.changedImage, "PNG dependency edit"), "PNG edit");
  check("Image edit rebuilds its dependent GLB", edited.rebuilds?.some(item => item.id === "triangle" && item.type === "accepted"));
  check("Image edit skips an unrelated asset", !edited.rebuilds?.some(item => item.id === "unrelated"));
  sameSelected("unrelated", unrelated, "Unrelated GPU buffers keep their identity");
  check("Dependent GPU resources are replaced", environment.runtime.resources("triangle").meshes[0].vertex !== priorTriangle.meshes[0].vertex);
  await imageMatches("image", fixtures.secondPixels); await imageMatches("triangle", fixtures.secondPixels);
  const sources = await store.list(), root = sources.find(source => source.record.id === "triangle");
  const inputs = await Promise.all(sources.map(async source => ({ record: source.record, bytes: await store.read(source.record) })));
  const cached = await environment.client.compile(inputs.find(input => input.record.id === "triangle"), inputs, root.revision);
  check("Warm conversion uses the real worker cache without uploads", cached.metrics.cacheHit && cached.metrics.uploads === 0);
  report.warmCache = cached.metrics;
  stage("Cancelling and superseding actual worker requests");
  const beforeCancel = environment.runtime.resources("triangle");
  environment.control.afterCompile = () => environment.session.cancel("triangle");
  const cancelled = await imported("triangle", "glb", "models/triangle.glb", blobs.cancelled, "Real cancellation");
  check("Cancellation wins after the actual compile request is sent", cancelled.type === "cancelled");
  sameSelected("triangle", beforeCancel, "Cancellation preserves the last working GPU objects");
  await idle();
  let replacement;
  environment.control.afterCompile = () => { replacement = imported("triangle", "glb", "models/triangle.glb", blobs.latest, "Latest replacement"); };
  const stale = await imported("triangle", "glb", "models/triangle.glb", blobs.stale, "Superseded request");
  check("The superseded actual request is stale", stale.type === "stale");
  accepted(await replacement, "Latest replacement");
  await idle();
  await meshMatches("triangle", fixtures.latestPositions); await imageMatches("triangle", fixtures.secondPixels);
  stage("Checking bounded cache eviction with real PNG conversions");
  const cacheBegan = performance.now(), cacheInputs = [];
  for (let index = 0; index < fixtures.cacheImages.length; index++) {
    const bytes = bytesOf(fixtures.cacheImages[index]);
    const record = { id: "cache-image", kind: "image", path: "cache-image.png", bytes: bytes.byteLength, hash: await sourceBytesHash(bytes) };
    const input = { record, bytes };
    cacheInputs.push(input);
    const result = await environment.client.compile(input, [], index + 1);
    equal([...result.description.images[0].bytes], Array.from({ length: 4 }, () => [index, 255 - index, 127, 255]).flat(), "Cache control decoded pixels");
  }
  const cold = await environment.client.compile(cacheInputs[0], [], 100);
  const warm = await environment.client.compile(cacheInputs[0], [], 100);
  check("An evicted compiled version is rebuilt, then cached", !cold.metrics.cacheHit && warm.metrics.cacheHit);
  const stats = await idle();
  check("Worker cache and source versions stay within their bounds", stats.cacheEntries <= MAX_SOURCE_COUNT &&
    stats.cacheBytes <= MAX_COMPILED_CACHE_BYTES && stats.sourceBytes <= MAX_SOURCE_STORE_BYTES && stats.sourceCount <= MAX_SOURCE_COUNT * 4);
  report.cacheEviction = { versions: cacheInputs.length, elapsedMs: performance.now() - cacheBegan, cold: cold.metrics, warm: warm.metrics, stats };
  stage("Keeping working resources after bad source edits");
  const beforeUnsupported = environment.runtime.resources("triangle");
  const unsupported = await imported("triangle", "glb", "models/triangle.glb", blobs.unsupported, "Unsupported primitive draft");
  check("Unsupported geometry is rejected while its image is valid", unsupported.type === "failed");
  sameSelected("triangle", beforeUnsupported, "Unsupported geometry keeps the last working GPU resources");
  accepted(await imported("triangle", "glb", "models/triangle.glb", blobs.latest, "Valid GLB after unsupported draft"), "Recovery from unsupported geometry");
  const goodTriangle = environment.runtime.resources("triangle"), goodImage = environment.runtime.resources("image");
  const invalidImage = await imported("image", "image", "models/tile.png", new Uint8Array([137, 80, 78, 71]), "Invalid PNG draft");
  check("Invalid image reports its source and dependent failure", invalidImage.type === "failed" && invalidImage.source === "models/tile.png" &&
    invalidImage.rebuilds?.some(item => item.id === "triangle" && item.type === "failed"));
  sameSelected("image", goodImage, "Invalid image keeps the previous texture");
  sameSelected("triangle", goodTriangle, "Invalid dependency keeps the previous mesh and texture");
  const truncated = await imported("triangle", "glb", "models/triangle.glb", blobs.latest.slice(0, -4), "Truncated GLB draft");
  check("Truncated GLB is rejected with a source-specific diagnostic", truncated.type === "failed" && truncated.source === "models/triangle.glb");
  sameSelected("triangle", goodTriangle, "Truncation keeps the last working GPU resources");
  const saved = await store.list();
  for (const [id, encoded] of [["image", blobs.image], ["triangle", blobs.triangle]]) {
    const source = saved.find(item => item.record.id === id);
    equal([...await store.read(source.original)], [...encoded], `${id}: encoded original survives later drafts`);
    check(`${id}: invalid latest differs from its accepted source`, source.record.hash !== source.accepted.sources.find(pin => pin.id === id).hash);
  }
  report.beforeReopen = { sources: saved, resources: environment.runtime.stats(), worker: await idle(), transfers: { ...environment.transfers } };
  selectedResources(["unrelated", "image", "triangle"]);
  check("Actual source transfers detach only the worker-owned copies", environment.transfers.uploads > 0 && environment.transfers.detachedBuffers === environment.transfers.uploads);
  environment.session.dispose();
  disposed("First session close before reopening");
  stage("Reopening accepted historical files from OPFS");
  const reopened = await AssetSourceStore.open(projectId);
  equal(await reopened.list(), saved, "OPFS manifest survives a new store instance");
  environment = openEnvironment(reopened);
  const restored = await environment.session.restore();
  check("All saved working assets restore despite invalid latest drafts", restored.length === 3 && restored.every(item => item.type === "accepted"));
  equal(await reopened.list(), saved, "Restore does not accept or overwrite newer invalid drafts");
  await meshMatches("unrelated", fixtures.positions); await meshMatches("triangle", fixtures.latestPositions);
  await imageMatches("image", fixtures.secondPixels); await imageMatches("triangle", fixtures.secondPixels);
  report.reopened = { outcomes: restored.map(item => ({ id: item.id, type: item.type, revision: item.revision, metrics: item.metrics })),
    resources: environment.runtime.stats(), worker: await idle(), transfers: { ...environment.transfers } };
  selectedResources(["unrelated", "image", "triangle"]);
  await ownedSources(fixtures, blobs);
  await preview();
  check("No uncaptured GPU or worker errors", report.gpuErrors.length === 0 && report.workerErrors.length === 0);
  report.elapsedMs = performance.now() - report.started;
  report.memory = performance.memory ? { usedJsHeapBytes: performance.memory.usedJSHeapSize, scope: "Browser JS heap estimate, not GPU memory." } : undefined;
  report.passed = true;
  stage("Passed: real worker, local saved files, GPU readback, and reopen");
  return report;
};
