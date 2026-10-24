import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";

// Captures contain licensed assets; choose a private output outside the checkout.
const output = resolve(process.env.AF_CAPTURE_DIR || "");
const checkout = resolve(new URL("..", import.meta.url).pathname);
assert.ok(
  process.env.AF_CAPTURE_DIR &&
    output !== checkout &&
    !output.startsWith(checkout + "/"),
);
const captureTime = Number(process.env.AF_CAPTURE_TIME ?? 0);
assert.ok(
  Number.isFinite(captureTime) && captureTime >= 0 && captureTime <= 86400,
  "Use a capture time between zero and 86400 seconds.",
);
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
});
const report = { captures: [], errors: [], console: [], ok: false };
try {
  const page = await context.newPage();
  page.on("pageerror", (e) => report.errors.push(e.message));
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type())) report.console.push(m.text());
  });
  await page.goto("http://127.0.0.1:55304/");
  await page.waitForFunction(
    () =>
      (window.aethyrforge?.snapshot().status.phase === "ready" &&
        window.aethyrforge.snapshot().status.frame > 2) ||
      ["error", "lost"].includes(window.aethyrforge?.snapshot().status.phase),
    null,
    { timeout: 120000 },
  );
  report.startup = await page.evaluate(
    () => window.aethyrforge.snapshot().status,
  );
  assert.equal(report.startup.phase, "ready", report.startup.message);
  const ids = (
    process.env.AF_GALLERY_SHOTS ||
    "02,03,05,06,07,08,09,10,11,12,13,14,15,16,17,18"
  )
    .split(",")
    .map((id) => `MT-${id}`);
  for (const id of ids) {
    const frame = await page.evaluate(
      () => window.aethyrforge.snapshot().status.frame,
    );
    await page.getByLabel("Camera composition").selectOption(id);
    await page.waitForFunction(
      ({ id, frame }) => {
        const s = window.aethyrforge.snapshot();
        return (
          s.status.frame > frame + 3 &&
          s.receipts.some((r) => r.ok && r.message === `Camera ${id}`)
        );
      },
      { id, frame },
      { timeout: 30000 },
    );
    const pending = page.waitForEvent("download", { timeout: 30000 });
    await page.evaluate(
      (time) => window.aethyrforge.capture(time),
      captureTime,
    );
    const download = await pending;
    const path = resolve(output, `${id}.png`);
    await download.saveAs(path);
    const png = await readFile(path);
    assert.equal(png.readUInt32BE(16), 1280);
    assert.equal(png.readUInt32BE(20), 720);
    report.captures.push({
      id,
      path,
      animationTime: captureTime,
      bytes: png.length,
      snapshot: await page.evaluate(() => window.aethyrforge.snapshot()),
    });
    if (process.env.AF_UI_CAPTURE === "1")
      await page.screenshot({ path: resolve(output, `${id}-editor.png`) });
    console.log(`CAPTURE ${id} ${png.length}`);
  }
  assert.deepEqual(report.errors, []);
  assert.ok(
    !report.console.some((m) =>
      /validation|shader|pipeline|graphics device|webgpu.*error/i.test(m),
    ),
    report.console.join("\n"),
  );
  report.ok = true;
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
