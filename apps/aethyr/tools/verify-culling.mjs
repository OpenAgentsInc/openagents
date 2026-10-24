import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";
const checkout = resolve(new URL("..", import.meta.url).pathname);
const output = resolve(process.env.AF_CAPTURE_DIR || "");
assert.ok(
  process.env.AF_CAPTURE_DIR &&
    output !== checkout &&
    !output.startsWith(checkout + "/"),
);
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
});
const report = { errors: [], captures: [], ok: false };
try {
  const page = await context.newPage();
  page.on("pageerror", (e) => report.errors.push(e.message));
  await page.goto("http://127.0.0.1:55304/");
  await page.waitForFunction(
    () =>
      window.aethyrforge?.snapshot().status.phase === "ready" &&
      window.aethyrforge.snapshot().status.frame > 2,
    null,
    { timeout: 120000 },
  );
  const previousFrame = await page.evaluate(
    () => window.aethyrforge.snapshot().status.frame,
  );
  await page.getByLabel("Camera composition").selectOption("MT-09");
  await page.waitForFunction(() =>
    window.aethyrforge
      .snapshot()
      .receipts.some((r) => r.ok && r.message === "Camera MT-09"),
  );
  await page.waitForFunction(
    (frame) => window.aethyrforge.snapshot().status.frame > frame + 3,
    previousFrame,
  );
  // Custom source conservatively disables prepared lighting as well as bounds
  // culling. Keep both paths on the same live generated-sky profile here.
  await page.evaluate(() =>
    window.aethyrforge.environment({
      ...window.aethyrforge.snapshot().environment,
      skySource: "generated",
    }),
  );
  await page.waitForFunction(
    () => window.aethyrforge.snapshot().environment.skySource === "generated",
  );
  const capture = async (name) => {
    const pending = page.waitForEvent("download");
    await page.evaluate(() => window.aethyrforge.capture(0));
    const download = await pending;
    const path = resolve(output, name + ".png");
    await download.saveAs(path);
    const png = await readFile(path);
    report.captures.push({ path, bytes: png.length, time: 0 });
    return png;
  };
  const culled = await capture("culled");
  const id = await page.evaluate(async () => {
    const { MATERIAL } = await import("/packages/renderer/shaders.ts");
    return window.aethyrforge.shader(
      MATERIAL +
        "\n// Unchanged vertex shader; custom-source mode disables source bounds culling.\n",
    );
  });
  await page.waitForFunction(
    (id) =>
      window.aethyrforge
        .snapshot()
        .receipts.some((r) => r.requestId === id && r.ok),
    id,
    { timeout: 30000 },
  );
  const unculled = await capture("unculled");
  const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
  report.hashes = { culled: hash(culled), unculled: hash(unculled) };
  assert.equal(
    report.hashes.culled,
    report.hashes.unculled,
    "Fixed-time culled and unculled renders must match exactly.",
  );
  assert.deepEqual(report.errors, []);
  report.ok = true;
  console.log("PASS fixed-time culling comparison is byte-identical");
} catch (error) {
  report.failure = String(error.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
} finally {
  await writeFile(
    resolve(output, "verification.json"),
    JSON.stringify(report, null, 2),
  );
  await context.close();
  await browser.close();
}
