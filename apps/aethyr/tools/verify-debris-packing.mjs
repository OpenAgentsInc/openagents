/** Verify real WebGPU packing through an isolated browser, without loading a world. */
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
const report = {
  schema: "aethyrforge.debris-packing-browser.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  scope:
    "Real compute readback against synthetic geometry at existing Town slab capacities. This is correctness evidence, not scene FPS or physical presentation evidence.",
  source: {
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    files: await Promise.all(
      [
        "tools/fixtures/debris-packing.ts",
        "packages/renderer/debris-gpu-packing.ts",
        "packages/renderer/source-debris.ts",
      ].map(async (path) => ({
        path,
        sha256: hash(await readFile(resolve(root, path))),
      })),
    ),
  },
  errors: [],
};
const server = await createServer({
  root,
  configFile: false,
  server: { host: "127.0.0.1", port: 0 },
  plugins: [
    {
      name: "packing-probe-page",
      configureServer(server) {
        server.middlewares.use("/packing-probe.html", (_req, res) => {
          res.setHeader("Content-Type", "text/html");
          res.end(
            '<!doctype html><link rel="icon" href="data:,"><title>Debris packing verification</title>',
          );
        });
      },
    },
  ],
});
let browser, context;
try {
  await server.listen();
  const address = server.httpServer.address();
  assert(address && typeof address !== "string");
  browser = await chromium.connectOverCDP(endpoint);
  context = await browser.newContext({ serviceWorkers: "block" });
  const page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") report.errors.push(message.text());
  });
  await page.goto(`http://127.0.0.1:${address.port}/packing-probe.html`);
  report.browser = await browser.version();
  report.compilation = await page.evaluate(async () => {
    const { DEBRIS_PACKING_WGSL } =
      await import("/packages/renderer/debris-gpu-packing.ts");
    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter.requestDevice();
    try {
      const info = await device
        .createShaderModule({ code: DEBRIS_PACKING_WGSL })
        .getCompilationInfo();
      return Array.from(
        info.messages,
        ({ type, message, lineNum, linePos }) => ({
          type,
          message,
          lineNum,
          linePos,
        }),
      );
    } finally {
      device.destroy();
    }
  });
  assert(
    !report.compilation.some((message) => message.type === "error"),
    JSON.stringify(report.compilation),
  );
  report.result = await page.evaluate(async () =>
    (
      await import("/tools/fixtures/debris-packing.ts")
    ).runDebrisPackingGpuProbe(),
  );
  assert.equal(report.result.ok, true, JSON.stringify(report.result));
  assert.deepEqual(report.errors, []);
  report.ok = true;
  console.log(JSON.stringify(report.result));
} catch (error) {
  report.errors.push(error.stack || String(error));
  process.exitCode = 1;
} finally {
  await context?.close();
  await browser?.close();
  await server.close();
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "debris-packing-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
