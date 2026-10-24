/** Run the owned boids comparison through openagents browser run. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  realpath,
  writeFile,
} from "node:fs/promises";
import { createServer as createHTTPServer } from "node:http";
import { cpus, platform, release } from "node:os";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep captures and raw reports outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const durationMs = Number(process.env.AF_BOIDS_BENCHMARK_MS ?? 30000);
const warmupMs = Number(process.env.AF_BOIDS_WARMUP_MS ?? 5000);
const production = process.env.AF_BOIDS_PRODUCTION === "1";
const buildRoot = resolve(root, "dist/boids");
assert(
  Number.isSafeInteger(durationMs) &&
    durationMs >= 1000 &&
    durationMs <= 120000,
  "Choose a benchmark duration from 1000 to 120000 ms.",
);
assert(
  Number.isSafeInteger(warmupMs) && warmupMs >= 0 && warmupMs <= 30000,
  "Choose a warm-up from 0 to 30000 ms.",
);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const git = (...args) =>
  execFileSync("git", args, { cwd: root }).toString("utf8");
async function sourcePin() {
  const names = [
    ...new Set(
      git("ls-files", "--cached", "--others", "--exclude-standard", "-z")
        .split("\0")
        .filter(Boolean),
    ),
  ];
  const selected = names.filter(
    (path) =>
      /boids/i.test(path) ||
      [
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "vite.config.ts",
      ].includes(path),
  );
  const files = {};
  const available = new Set(names);
  const pending = [...selected];
  for (let i = 0; i < pending.length; i++) {
    const path = pending[i];
    if (files[path]) continue;
    const bytes = await readFile(resolve(root, path));
    files[path] = hash(bytes);
    // Include relative shared helpers, such as camera matrices, in the pin.
    if (/\.[cm]?[jt]sx?$/.test(path)) {
      for (const match of bytes
        .toString("utf8")
        .matchAll(/(?:from\s*|import\s*)["'](\.[^"']+)["']/g)) {
        const imported = fileURLToPath(
          new URL(match[1], new URL(path, `file://${root}/`)),
        ).slice(root.length + 1);
        const found = [
          imported,
          `${imported}.ts`,
          `${imported}.mjs`,
          `${imported}/index.ts`,
        ].find((candidate) => available.has(candidate));
        if (found && !files[found]) pending.push(found);
      }
    }
  }
  const ordered = Object.fromEntries(
    Object.entries(files).sort(([a], [b]) => a.localeCompare(b)),
  );
  return {
    revision: git("rev-parse", "HEAD").trim(),
    dirty: !!git("status", "--porcelain").trim(),
    files: ordered,
    sha256: hash(JSON.stringify(ordered)),
  };
}
async function buildPin() {
  const files = {};
  async function walk(directory, prefix = "") {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const name = prefix + entry.name;
      if (entry.isDirectory())
        await walk(resolve(directory, entry.name), name + "/");
      else {
        assert(
          entry.isFile(),
          "The built player must contain only regular files and directories.",
        );
        const bytes = await readFile(resolve(directory, entry.name));
        files[name] = { bytes: bytes.length, sha256: hash(bytes) };
      }
    }
  }
  await walk(buildRoot);
  assert(
    files["boids.html"],
    "Build Boids Lab before verifying production artifacts.",
  );
  const ordered = Object.fromEntries(
    Object.entries(files).sort(([a], [b]) => a.localeCompare(b)),
  );
  return {
    files: ordered,
    sha256: hash(JSON.stringify(ordered)),
    totalBytes: Object.values(files).reduce((sum, item) => sum + item.bytes, 0),
  };
}
async function productionServer() {
  const mime = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".json": "application/json",
    ".wasm": "application/wasm",
    ".png": "image/png",
  };
  const http = createHTTPServer(async (request, response) => {
    try {
      const pathname = decodeURIComponent(
        new URL(request.url, "http://localhost").pathname,
      );
      const path = resolve(buildRoot, "." + pathname);
      if (!path.startsWith(buildRoot + sep)) {
        response.writeHead(403);
        response.end();
        return;
      }
      const bytes = await readFile(path);
      response.writeHead(200, {
        "Content-Type": mime[extname(path)] ?? "application/octet-stream",
        "Content-Length": bytes.length,
        "Cache-Control": "no-store",
      });
      response.end(bytes);
    } catch {
      response.writeHead(404);
      response.end();
    }
  });
  await new Promise((resolve, reject) => {
    http.once("error", reject);
    http.listen(0, "127.0.0.1", resolve);
  });
  const base = `http://127.0.0.1:${http.address().port}/`;
  return {
    resolvedUrls: { local: [base] },
    close: () =>
      new Promise((resolve, reject) =>
        http.close((error) => (error ? reject(error) : resolve())),
      ),
  };
}
const report = {
  schema: "aethyrforge.boids-browser.v1",
  ok: false,
  startedAt: new Date().toISOString(),
  completedAt: null,
  options: {
    count: 100000,
    durationMs,
    warmupMs,
    production,
    modes: ["gpu", "cpu", "split"],
    shortPreflight: durationMs !== 30000 || warmupMs !== 5000,
  },
  scope:
    "Two owned Aethyrforge implementations with an original procedural fish mesh. The external Unity and Kaigen demos are reference links only. Single-view windows run sequentially before the split view; split views share a GPU. Frame delivery is application submission/completion evidence, not physical display presentation. No external demo is loaded by this check.",
  source: await sourcePin(),
  sourceAfter: null,
  built: production ? await buildPin() : null,
  builtAfter: null,
  machine: {
    os: `${platform()} ${release()}`,
    cpuModel: cpus()[0]?.model ?? null,
    logicalCpus: cpus().length,
    runtime: process.version,
  },
  browser: null,
  checks: [],
  captures: [],
  benchmarks: [],
  errors: [],
  warnings: [],
  failure: null,
  cleanupErrors: [],
};
const save = () =>
  writeFile(
    resolve(output, "boids-raw.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
let browser, context, page, server;
const check = (name, detail = {}) => {
  report.checks.push({ name, ...detail });
  console.log(`PASS ${name}`);
};
const status = () => page.evaluate(() => window.aethyrforgeBoids.status());
function hasRendered(value, count, mode) {
  return (
    value.count === count &&
    value.mode === mode &&
    value.width > 0 &&
    value.height > 0 &&
    value.metrics.trianglesPerView?.length === (mode === "split" ? 2 : 1) &&
    value.metrics.trianglesPerView.every(
      (triangles) => triangles === count * 310,
    )
  );
}
async function waitStatus(predicate, message) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    const value = await status();
    if (predicate(value)) return value;
    await page.waitForTimeout(50);
  }
  throw Error(`${message}: ${JSON.stringify(await status())}`);
}
function assertStatus(value, { count, mode }) {
  assert.equal(
    value.count,
    count,
    "The live population differs from the requested population.",
  );
  assert.equal(
    value.mode,
    mode,
    "The live mode differs from the requested mode.",
  );
  assert(
    value.width > 0 && value.height > 0,
    "The canvas has no backing pixels.",
  );
  assert.equal(
    value.errors.length,
    0,
    "The application reported a device or runtime error.",
  );
  assert.equal(
    value.metrics.lastError,
    null,
    "The renderer reported an error.",
  );
  assert.deepEqual(
    value.metrics.trianglesPerView,
    Array(mode === "split" ? 2 : 1).fill(count * 310),
    "Submitted geometry does not match 310 triangles per fish in every view.",
  );
  assert(
    value.metrics.inFlight <= 2,
    "More than two render submissions are in flight.",
  );
}
// Decode a browser screenshot into a separate 2D canvas. This observes compositor
// pixels rather than trusting a render counter or a shader-generated reference.
async function pixels(png) {
  return page.evaluate(async (base64) => {
    const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
    const image = await createImageBitmap(
      new Blob([bytes], { type: "image/png" }),
    );
    const width = image.width,
      height = image.height;
    const canvas = document.createElement("canvas");
    canvas.width = Math.min(480, width);
    canvas.height = Math.round((height * canvas.width) / width);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
    image.close();
    const rgba = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let sum = 0,
      sum2 = 0,
      edges = 0,
      opaque = 0,
      nonblack = 0;
    const colors = new Set();
    for (let y = 0; y < canvas.height; y += 2)
      for (let x = 0; x < canvas.width; x += 2) {
        const i = (y * canvas.width + x) * 4,
          l = (rgba[i] + rgba[i + 1] + rgba[i + 2]) / 3;
        sum += l;
        sum2 += l * l;
        opaque += rgba[i + 3] >= 250;
        nonblack += l > 3;
        colors.add(
          (rgba[i] >> 3) |
            ((rgba[i + 1] >> 3) << 5) |
            ((rgba[i + 2] >> 3) << 10),
        );
        if (x + 2 < canvas.width) {
          const j = i + 8;
          if (
            Math.abs(rgba[i] - rgba[j]) +
              Math.abs(rgba[i + 1] - rgba[j + 1]) +
              Math.abs(rgba[i + 2] - rgba[j + 2]) >
            35
          )
            edges++;
        }
      }
    const count = Math.ceil(canvas.width / 2) * Math.ceil(canvas.height / 2),
      mean = sum / count;
    return {
      width,
      height,
      sampleWidth: canvas.width,
      sampleHeight: canvas.height,
      mean,
      standardDeviation: Math.sqrt(Math.max(0, sum2 / count - mean * mean)),
      quantizedColors: colors.size,
      edgeFraction: edges / count,
      opaqueFraction: opaque / count,
      nonblackFraction: nonblack / count,
      rgba: Array.from(rgba),
    };
  }, png.toString("base64"));
}
async function capture(name) {
  const canvases = page.locator("canvas:visible");
  assert.equal(
    await canvases.count(),
    1,
    "The comparison should have one visible shared canvas.",
  );
  const png = await canvases.screenshot({
    type: "png",
    animations: "disabled",
  });
  const file = `${name}.png`;
  await writeFile(resolve(output, file), png);
  const sample = await pixels(png),
    { rgba, ...coverage } = sample;
  report.captures.push({
    name,
    file,
    pngSha256: hash(png),
    coverage,
    status: await status(),
  });
  await save();
  assert(
    coverage.width >= 320 && coverage.height >= 180,
    "The world capture is too small.",
  );
  assert(
    coverage.nonblackFraction > 0.8 &&
      coverage.standardDeviation > 4 &&
      coverage.quantizedColors > 24 &&
      coverage.edgeFraction > 0.0001,
    "The world capture is blank or lacks visible scene detail.",
  );
  return sample;
}
function comparePixels(a, b) {
  assert.equal(a.width, b.width);
  assert.equal(a.height, b.height);
  let changed = 0,
    absolute = 0;
  for (let i = 0; i < a.rgba.length; i += 4) {
    let delta = 0;
    for (let c = 0; c < 3; c++)
      delta += Math.abs(a.rgba[i + c] - b.rgba[i + c]);
    changed += delta > 15;
    absolute += delta;
  }
  return {
    changedFraction: changed / (a.sampleWidth * a.sampleHeight),
    meanAbsoluteChannelDifference:
      absolute / (a.sampleWidth * a.sampleHeight * 3),
  };
}
try {
  server = production
    ? await productionServer()
    : await createServer({
        root,
        configFile: false,
        logLevel: "error",
        cacheDir: `node_modules/.vite-boids-${process.pid}`,
        server: { host: "127.0.0.1", port: 0, strictPort: false },
      });
  if (!production) await server.listen();
  browser = await chromium.connectOverCDP(endpoint);
  const cdp = await browser.newBrowserCDPSession();
  report.browser = {
    version: await browser.version(),
    commandLine: await cdp
      .send("Browser.getBrowserCommandLine")
      .catch((error) => ({ unavailable: String(error) })),
    info: await cdp.send("Browser.getVersion"),
  };
  await cdp.detach();
  context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
  });
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["error", "warning"].includes(message.type()) &&
      report.warnings.length < 100
    )
      report.warnings.push({
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  await page.goto(new URL("/boids.html", server.resolvedUrls.local[0]).href, {
    waitUntil: "domcontentloaded",
  });
  await page.bringToFront();
  await page.waitForFunction(
    () => window.aethyrforgeBoids?.ready === true,
    null,
    { timeout: 90000 },
  );
  await page.evaluate(async () => {
    await window.aethyrforgeBoids.setOptions({ count: 100000, mode: "split" });
    window.aethyrforgeBoids.pause(true);
    await window.aethyrforgeBoids.reset();
  });
  assertStatus(
    await waitStatus(
      (s) => hasRendered(s, 100000, "split"),
      "The initial scene did not draw the full population",
    ),
    { count: 100000, mode: "split" },
  );
  check("100,000 live fish and 31 million submitted triangles per view", {
    totalTriangles: 62000000,
  });
  const verification = await page.evaluate(() =>
    window.aethyrforgeBoids.verifySimulation(),
  );
  report.simulation = verification;
  await save();
  assert.equal(
    verification.ok,
    true,
    "The CPU/GPU simulation fixtures failed.",
  );
  assert(
    Array.isArray(verification.cases) &&
      verification.cases.length >= 3 &&
      verification.cases.every((item) => item.ok === true),
    "The simulation check omitted independent passing cases.",
  );
  check("Small CPU/GPU fixtures preserve finite state and shared workload", {
    cases: verification.cases,
  });

  await page.getByTestId("boids-pause").click();
  await waitStatus(
    (s) => s.paused === false,
    "The pause control did not resume the world",
  );
  await waitStatus(
    (s) => s.simulationTime > 0,
    "Resuming did not advance the simulation",
  );
  await page.getByTestId("boids-pause").click();
  await waitStatus(
    (s) => s.paused === true,
    "The pause control did not pause the world",
  );
  const frozen = await status();
  await page.waitForTimeout(250);
  assert.equal(
    (await status()).simulationTime,
    frozen.simulationTime,
    "The paused simulation kept advancing.",
  );
  await page.getByTestId("boids-reset").click();
  await waitStatus(
    (s) =>
      s.resetGeneration > frozen.resetGeneration &&
      s.simulationTime === 0 &&
      hasRendered(s, 100000, "split"),
    "Reset did not restore the paused simulation",
  );
  check("Pause, resume, and reset controls respond to browser clicks");
  const many = await capture("split-100000");
  await page.getByTestId("boids-count").selectOption("1000");
  await waitStatus(
    (s) => s.count === 1000,
    "The count control did not change the population",
  );
  await page.getByTestId("boids-reset").click();
  assertStatus(
    await waitStatus(
      (s) => hasRendered(s, 1000, "split"),
      "The smaller population did not render",
    ),
    { count: 1000, mode: "split" },
  );
  const few = await capture("split-1000");
  const difference = comparePixels(many, few);
  report.populationPixelDifference = difference;
  await save();
  assert(
    difference.changedFraction > 0.01 &&
      difference.meanAbsoluteChannelDifference > 0.1,
    "Changing the fish population did not visibly change the world.",
  );
  check(
    "Independent screenshot pixels respond to population changes",
    difference,
  );
  await page.getByTestId("boids-count").selectOption("100000");
  await waitStatus((s) => s.count === 100000, "The population did not restore");
  await page.getByTestId("boids-mode").selectOption("gpu");
  await waitStatus(
    (s) => hasRendered(s, 100000, "gpu"),
    "The mode control did not switch views",
  );
  assertStatus(await status(), { count: 100000, mode: "gpu" });
  await page.setViewportSize({ width: 960, height: 540 });
  await page.waitForTimeout(200);
  await capture("gpu-resized");
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.waitForTimeout(200);
  const chromeHeight = await page.evaluate(
    () => innerHeight - document.querySelector("canvas").clientHeight,
  );
  await page.setViewportSize({ width: 1280, height: 720 + chromeHeight });
  await waitStatus(
    (s) => s.width === 1280 && s.height === 720,
    "The benchmark canvas did not reach 1280 × 720 backing pixels",
  );
  report.benchmarkViewport = {
    viewport: page.viewportSize(),
    canvas: { width: 1280, height: 720 },
    chromeHeight,
  };
  check("View selection and resize preserve visible geometry");

  for (const mode of report.options.modes) {
    console.log(
      `MEASURE ${mode}: ${warmupMs} ms warm-up, ${durationMs} ms recorded`,
    );
    const result = await page.evaluate(
      async (options) => {
        const deadline = new Promise((_, reject) =>
          setTimeout(
            () => reject(Error("The benchmark exceeded its bounded duration.")),
            options.durationMs + options.warmupMs + 30000,
          ),
        );
        return Promise.race([
          window.aethyrforgeBoids.benchmark(options),
          deadline,
        ]);
      },
      { durationMs, warmupMs, mode, count: 100000 },
    );
    report.benchmarks.push({ mode, ...result });
    await save();
    assert.equal(result.ok, true, `The ${mode} benchmark failed.`);
    assert(
      result.durationMs >= durationMs * 0.9 &&
        result.frameCount > 0 &&
        result.statistics,
      "The benchmark omitted actual frame measurements.",
    );
    assertStatus(await status(), { count: 100000, mode });
    await page.evaluate(() => window.aethyrforgeBoids.pause(true));
    await capture(`${mode}-measured`);
    check(`${mode} measurement retained`, {
      frames: result.frameCount,
      durationMs: result.durationMs,
    });
  }
  report.finalStatus = await status();
  report.sourceAfter = await sourcePin();
  if (production) {
    report.builtAfter = await buildPin();
    assert.deepEqual(
      report.builtAfter,
      report.built,
      "The built artifact closure changed during measurement.",
    );
  }
  assert.deepEqual(
    report.sourceAfter.files,
    report.source.files,
    "Boids source changed during measurement.",
  );
  assert.equal(
    report.errors.length,
    0,
    "The browser reported an uncaught error.",
  );
  assert.equal(
    report.finalStatus.errors.length,
    0,
    "The application reported an error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error);
  if (page) {
    report.finalStatus = await status().catch((problem) => ({
      unavailable: String(problem),
    }));
    await page
      .screenshot({ path: resolve(output, "failure.png"), fullPage: true })
      .catch(() => {});
  }
  process.exitCode = 1;
} finally {
  if (page)
    await page
      .evaluate(() => window.aethyrforgeBoids?.pause(true))
      .catch(() => {});
  if (page) {
    try {
      const png = await page.screenshot({
        fullPage: true,
        animations: "disabled",
      });
      await writeFile(resolve(output, "boids-full-page.png"), png);
      report.fullPageCapture = {
        file: "boids-full-page.png",
        sha256: hash(png),
      };
    } catch (error) {
      report.cleanupErrors.push({
        name: "full-page capture",
        error: String(error),
      });
    }
  }
  for (const [name, close] of [
    ["context", () => context?.close()],
    ["browser", () => browser?.close()],
    ["server", () => server?.close()],
  ]) {
    try {
      await close();
    } catch (error) {
      report.cleanupErrors.push({ name, error: String(error) });
      process.exitCode = 1;
    }
  }
  report.completedAt = new Date().toISOString();
  if (report.cleanupErrors.length) {
    report.ok = false;
    process.exitCode = 1;
  }
  await save();
  console.log(
    JSON.stringify({
      ok: report.ok,
      report: resolve(output, "boids-raw.json"),
      failure: report.failure,
      checks: report.checks.length,
      windows: report.benchmarks.length,
    }),
  );
}
