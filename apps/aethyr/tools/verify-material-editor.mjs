/** Check editor-only materials on owned geometry through an isolated browser. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(process.env.AF_CAPTURE_DIR, { recursive: true });
const output = await realpath(process.env.AF_CAPTURE_DIR);
assert(output !== root && !output.startsWith(root + sep));
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const errorText = (error) => error?.stack || String(error);
const inventory = () =>
  JSON.parse(
    execFileSync(
      "python3",
      [
        "tools/assets/measure-runtime-assets.py",
        "--profile",
        "starter",
        "--source",
        resolve(root, "public/starter"),
      ],
      { cwd: root, encoding: "utf8", timeout: 30_000, maxBuffer: 4 << 20 },
    ),
  );
const report = {
  schema: "aethyrforge.material-editor-browser.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  scope:
    "Owned Starter geometry relabeled in memory; real published-Room editor hooks and Renderer material staging, remapping and restoration. No kernel execution, provider, public authoring UI, performance or physical presentation qualification.",
  source: {
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    patchSha256: hash(
      execFileSync("git", ["diff", "HEAD", "--binary"], { cwd: root }),
    ),
    files: [],
  },
  errors: [],
  consoleErrors: [],
  cleanup: { context: false, connection: false, server: false },
};
const sourcePaths = [
  "tools/verify-material-editor.mjs",
  "tools/fixtures/material-editor-fixture.ts",
  "tools/assets/measure-runtime-assets.py",
  "../../scripts/deploy/aethyr/package-assets.py",
  "packages/runtime/published-world.ts",
  "packages/runtime/world-loader.ts",
  "packages/runtime/player-artifact.ts",
  "packages/runtime/material-source.ts",
  "packages/runtime/cell-stream.ts",
  "packages/renderer/renderer.ts",
  "packages/renderer/material-editor.ts",
  "packages/renderer/features.ts",
  "packages/renderer/room-features.ts",
  "packages/renderer/feature-contract.ts",
  "packages/renderer/shaders.ts",
  "public/kernel.wasm",
  "package-lock.json",
];
const captureNames = [
  "before",
  "staged",
  "applied",
  "discarded",
  "failed",
  "scalar",
  "scalar-restored",
  "restored",
];
let server, browser, context;
try {
  report.source.files = await Promise.all(
    sourcePaths.map(async (path) => {
      const bytes = await readFile(resolve(root, path));
      return { path, bytes: bytes.byteLength, sha256: hash(bytes) };
    }),
  );
  report.assetsBefore = inventory();
  server = await createServer({
    root,
    configFile: false,
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "material-editor-probe-page",
        configureServer(server) {
          server.middlewares.use(
            "/material-editor-probe.html",
            (_request, response) => {
              response.setHeader("Content-Type", "text/html");
              response.end(
                '<!doctype html><link rel="icon" href="data:,"><title>Material editor verification</title>',
              );
            },
          );
        },
      },
    ],
  });
  await server.listen();
  const address = server.httpServer.address();
  assert(address && typeof address !== "string");
  browser = await chromium.connectOverCDP(endpoint, { timeout: 30_000 });
  context = await browser.newContext({
    viewport: { width: 800, height: 500 },
    deviceScaleFactor: 1,
    serviceWorkers: "block",
  });
  const page = await context.newPage();
  page.setDefaultTimeout(90_000);
  page.on("pageerror", (error) => {
    if (report.errors.length < 64) report.errors.push(errorText(error));
  });
  page.on("console", (message) => {
    if (message.type() === "error" && report.consoleErrors.length < 64)
      report.consoleErrors.push(message.text().slice(0, 8192));
  });
  await page.goto(
    `http://127.0.0.1:${address.port}/material-editor-probe.html`,
    {
      waitUntil: "load",
      timeout: 30_000,
    },
  );
  report.browser = await browser.version();
  report.userAgent = await page.evaluate(() => navigator.userAgent);
  await page.bringToFront();
  let timer;
  const result = await Promise.race([
    page.evaluate(async () =>
      (
        await import("/tools/fixtures/material-editor-fixture.ts")
      ).runMaterialEditorGpuProbe(),
    ),
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(Error("Material GPU fixture exceeded 90 seconds.")),
        90_000,
      );
    }),
  ]).finally(() => clearTimeout(timer));
  assert.equal(result?.ok, true);
  assert.equal(result.width, 480);
  assert.equal(result.height, 300);
  assert(Array.isArray(result.checks) && result.checks.length >= 10);
  assert(Array.isArray(result.captures));
  assert.deepEqual(
    result.captures.map(({ name }) => name),
    captureNames,
  );
  const captures = [];
  for (const capture of result.captures) {
    assert.equal(capture.width, 480);
    assert.equal(capture.height, 300);
    assert.equal(typeof capture.base64, "string");
    assert(capture.base64.length <= 480 * 300 * 4 * 2);
    const bytes = Buffer.from(capture.base64, "base64");
    assert.equal(bytes.byteLength, 480 * 300 * 4);
    assert.equal(hash(bytes), capture.sha256);
    const path = resolve(output, `material-${capture.name}.rgba`);
    await writeFile(path, bytes);
    captures.push({
      name: capture.name,
      path,
      width: 480,
      height: 300,
      format: "rgba8-tight",
      bytes: bytes.byteLength,
      sha256: capture.sha256,
    });
  }
  report.result = { ...result, captures };
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.consoleErrors, []);
  report.ok = true;
} catch (error) {
  report.errors.push(errorText(error));
} finally {
  // Close the GPU world before any file hashing or derived evidence processing.
  for (const [name, resource] of [
    ["context", context],
    ["connection", browser],
    ["server", server],
  ]) {
    try {
      await resource?.close();
      report.cleanup[name] = true;
    } catch (error) {
      report.ok = false;
      report.errors.push(`${name} cleanup: ${errorText(error)}`);
    }
  }
  try {
    report.assetsAfter = inventory();
    assert.deepEqual(report.assetsAfter, report.assetsBefore);
    report.assetInventoryUnchanged = true;
    for (const file of report.source.files)
      assert.equal(
        hash(await readFile(resolve(root, file.path))),
        file.sha256,
        `Source changed during verification: ${file.path}`,
      );
    report.sourceUnchanged = true;
  } catch (error) {
    report.ok = false;
    report.errors.push(errorText(error));
  }
  report.ok &&= report.errors.length === 0 && report.consoleErrors.length === 0;
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "material-editor-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  if (!report.ok) process.exitCode = 1;
  console.log(
    JSON.stringify({
      ok: report.ok,
      checks: report.result?.checks.length ?? 0,
      captures: report.result?.captures.length ?? 0,
      report: resolve(output, "material-editor-browser.json"),
      cleanup: report.cleanup,
      errors: report.errors,
      consoleErrors: report.consoleErrors,
    }),
  );
}
