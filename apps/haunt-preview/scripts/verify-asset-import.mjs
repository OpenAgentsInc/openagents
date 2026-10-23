import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { PNG } from "pngjs";
import { createServer } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scratch = process.env.HAUNT_EVIDENCE || process.env.OPENAGENTS_SCRATCH ||
  execFileSync("openagents", ["scratch"], { encoding: "utf8" }).trim();
const out = resolve(scratch, "asset-import");
await mkdir(out, { recursive: true });
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const positions = [-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0];

function glb(scale = 1, image = false, mode = 4) {
  const bin = Buffer.alloc(image ? 68 : 44);
  positions.forEach((value, index) => bin.writeFloatLE(value * scale, index * 4));
  [0, 1, 2].forEach((value, index) => bin.writeUInt16LE(value, 36 + index * 2));
  if (image) [0, 1, 1, 1, 0.5, 0].forEach((value, index) => bin.writeFloatLE(value, 44 + index * 4));
  const json = Buffer.from(JSON.stringify({
    asset: { version: "2.0" }, buffers: [{ byteLength: bin.length }],
    bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: 36 }, { buffer: 0, byteOffset: 36, byteLength: 6 },
      ...(image ? [{ buffer: 0, byteOffset: 44, byteLength: 24 }] : [])],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [-0.5 * scale, -0.5 * scale, 0], max: [0.5 * scale, 0.5 * scale, 0] },
      { bufferView: 1, componentType: 5123, count: 3, type: "SCALAR" },
      ...(image ? [{ bufferView: 2, componentType: 5126, count: 3, type: "VEC2" }] : []),
    ],
    meshes: [{ primitives: [{ attributes: { POSITION: 0, ...(image ? { TEXCOORD_0: 2 } : {}) }, indices: 1, mode,
      ...(image ? { material: 0 } : {}) }] }],
    nodes: [{ mesh: 0 }], scenes: [{ nodes: [0] }], scene: 0,
    ...(image ? { images: [{ uri: "tile.png" }], textures: [{ source: 0 }],
      materials: [{ pbrMetallicRoughness: { baseColorTexture: { index: 0 } } }] } : {}),
  }));
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4, 0x20);
  json.copy(padded);
  const result = Buffer.alloc(12 + 8 + padded.length + 8 + bin.length);
  result.writeUInt32LE(0x46546c67, 0); result.writeUInt32LE(2, 4); result.writeUInt32LE(result.length, 8);
  result.writeUInt32LE(padded.length, 12); result.writeUInt32LE(0x4e4f534a, 16); padded.copy(result, 20);
  result.writeUInt32LE(bin.length, 20 + padded.length); result.writeUInt32LE(0x004e4942, 24 + padded.length);
  bin.copy(result, 28 + padded.length);
  return result;
}
function png(pixels) {
  const image = new PNG({ width: 2, height: 2 });
  image.data.set(pixels);
  return PNG.sync.write(image);
}
const firstPixels = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
const secondPixels = [32, 64, 128, 255, 192, 32, 64, 255, 64, 192, 32, 255, 128, 64, 192, 255];
const blobs = {
  plain: glb(), triangle: glb(1, true), cancelled: glb(0.9, true), stale: glb(0.8, true), latest: glb(0.75, true),
  unsupported: glb(1, true, 1), image: png(firstPixels), changedImage: png(secondPixels),
};
const fixtures = {
  projectId: `asset-acceptance-${randomUUID()}`, positions, latestPositions: positions.map(value => value * 0.75),
  firstPixels, secondPixels, blobs: Object.fromEntries(Object.entries(blobs).map(([key, value]) => [key, value.toString("base64")])),
  cacheImages: Array.from({ length: 65 }, (_, index) => png(Array.from({ length: 4 }, () => [index, 255 - index, 127, 255]).flat()).toString("base64")),
};
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Asset import acceptance</title>
<link rel="icon" href="data:,"><style>body{margin:24px;background:#15151c;color:#eee;font:16px system-ui}canvas{width:400px;height:300px;border:1px solid #555}pre{white-space:pre-wrap;max-width:850px}</style>
<h1>Asset import acceptance</h1><p>Owned fixtures, real worker, local files, and WebGPU. No model requests.</p>
<canvas id="preview" width="400" height="300"></canvas><pre id="status">Starting…</pre>
<script type="module" src="/scripts/asset-import-fixture.mjs"></script></html>`;
const server = await createServer({
  root, configFile: false, appType: "custom", logLevel: "error", clearScreen: false,
  cacheDir: resolve(out, "vite-cache"), server: { host: "127.0.0.1", port: 0, hmr: false },
  plugins: [{ name: "asset-import-acceptance", configureServer(vite) {
    vite.middlewares.use((request, response, next) => {
      if (request.url?.split("?")[0] !== "/asset-import-acceptance") return next();
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.setHeader("Cache-Control", "no-store");
      response.end(html);
    });
  } }],
});
let browser, context, page;
const report = {
  schema: "openagents.asset-import-browser.v1", measuredAt: new Date().toISOString(),
  scope: "Minimal runtime adapter acceptance; not the full editor/renderer or a Maya inference test.",
  sources: {}, fixtures: Object.fromEntries(Object.entries(blobs).map(([key, bytes]) => [key, { bytes: bytes.length, sha256: sha256(bytes) }])),
  blockedRequests: [], consoleErrors: [], pageErrors: [], requestFailures: [], passed: false,
};
try {
  for (const path of ["src/asset-compiler.ts", "src/asset-owned-source-adapter.ts", "src/asset-import-protocol.ts", "src/asset-import-worker.ts", "src/asset-import-client.ts", "src/asset-runtime-adapter.ts", "src/asset-source-store.ts", "src/project.ts", "src/scene.ts", "src/layout.ts", "src/lab.ts", "scripts/asset-import-fixture.mjs", "scripts/verify-asset-import.mjs"]) {
    report.sources[path] = sha256(await readFile(resolve(root, path)));
  }
  await server.listen();
  const address = server.httpServer.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;
  report.origin = origin;
  const endpoint = process.env.OPENAGENTS_CHROME_WS || (process.env.OPENAGENTS_CHROME_PORT && `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
  if (endpoint) browser = await chromium.connectOverCDP(endpoint);
  else browser = await chromium.launch({ headless: true, args: ["--use-mock-keychain", "--password-store=basic", "--disable-background-networking"] });
  report.browser = browser.version();
  context = await browser.newContext({ viewport: { width: 1000, height: 850 }, serviceWorkers: "block" });
  await context.route("**/*", async route => {
    const request = route.request(), url = new URL(request.url());
    if (url.origin !== origin || request.method() !== "GET" || /\/(?:api|v1)(?:\/|$)|decision|systemone/i.test(url.pathname)) {
      report.blockedRequests.push({ method: request.method(), url: request.url() });
      return route.abort("blockedbyclient");
    }
    return route.continue();
  });
  await context.routeWebSocket("**/*", socket => {
    report.blockedRequests.push({ method: "WebSocket", url: socket.url() });
    socket.close();
  });
  page = await context.newPage();
  page.on("console", message => { if (message.type() === "error") report.consoleErrors.push(message.text()); });
  page.on("pageerror", error => report.pageErrors.push(String(error)));
  page.on("requestfailed", request => report.requestFailures.push({ url: request.url(), error: request.failure()?.errorText }));
  await page.goto(`${origin}/asset-import-acceptance`, { waitUntil: "networkidle", timeout: 30_000 });
  await page.waitForFunction(() => typeof window.runAssetImportAcceptance === "function", null, { timeout: 30_000 });
  let timer;
  try {
    report.acceptance = await Promise.race([page.evaluate(data => window.runAssetImportAcceptance(data), fixtures),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("Asset import acceptance exceeded 120 seconds.")), 120_000); })]);
  } finally { clearTimeout(timer); }
  await page.screenshot({ path: resolve(out, "asset-import.png"), fullPage: true });
  report.finalCleanup = await page.evaluate(() => window.cleanupAssetImportAcceptance());
  assert.deepEqual(report.finalCleanup.resources, { assets: 0, buffers: 0, textures: 0 });
  assert.deepEqual(report.finalCleanup.native, { buffers: 0, textures: 0, workers: 0 });
  assert.deepEqual(report.finalCleanup.gpuErrors, []); assert.deepEqual(report.finalCleanup.workerErrors, []);
  assert.deepEqual(report.blockedRequests, [], "The fixture attempted a foreign, API, or decision request.");
  assert.deepEqual(report.consoleErrors, []); assert.deepEqual(report.pageErrors, []);
  assert.deepEqual(report.requestFailures, []);
  for (const [path, hash] of Object.entries(report.sources)) {
    assert.equal(sha256(await readFile(resolve(root, path))), hash, `${path} changed during acceptance.`);
  }
  report.passed = true;
} catch (error) {
  report.error = String(error?.stack || error);
  if (page) {
    report.acceptance = await page.evaluate(() => window.assetImportAcceptanceReport).catch(() => undefined);
    await page.screenshot({ path: resolve(out, "asset-import-failure.png"), fullPage: true }).catch(() => {});
    report.finalCleanup = await page.evaluate(() => window.cleanupAssetImportAcceptance?.()).catch(() => undefined);
  }
  process.exitCode = 1;
} finally {
  await writeFile(resolve(out, "receipt.json"), `${JSON.stringify(report, null, 2)}\n`);
  await context?.close().catch(() => {});
  await browser?.close().catch(() => {});
  await server.close();
}
console.log(JSON.stringify({ passed: report.passed, receipt: resolve(out, "receipt.json"), ...(report.error ? { error: report.error } : {}) }));
