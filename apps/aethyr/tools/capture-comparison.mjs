/** Export the owned MT-01 presentation from two actual, independent scene renders. */
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { chromium } from "playwright";

const checkout = resolve(fileURLToPath(new URL("..", import.meta.url)));
const output = resolve(process.env.AF_CAPTURE_DIR || "");
const time = Number(process.env.AF_COMPARISON_TIME ?? 3);
assert.ok(
  process.env.AF_CAPTURE_DIR &&
    output !== checkout &&
    !output.startsWith(checkout + "/"),
  "Choose a private output directory outside this repository.",
);
assert.ok(
  Number.isFinite(time) && time >= 0 && time <= 86400,
  "Choose a comparison age from zero to 24 hours.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert.ok(endpoint, "Run this tool through openagents browser run.");
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
});
const report = {
  kind: "aethyrforge.owned-day-night-comparison.v1",
  time,
  views: [],
  errors: [],
  ok: false,
};
try {
  const page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(error.message));
  page.on("console", (message) => {
    if (
      message.type() === "error" &&
      /validation|shader|pipeline|GPUDevice|WebGPU/i.test(message.text())
    )
      report.errors.push(message.text());
  });
  await page.goto("http://127.0.0.1:55304/");
  await page.waitForFunction(
    () =>
      window.aethyrforge?.snapshot().status.phase === "ready" &&
      window.aethyrforge.snapshot().status.frame > 2,
    null,
    { timeout: 120000 },
  );
  for (const [shot, label] of [
    ["MT-11", "Night"],
    ["MT-02", "Day"],
  ]) {
    await page.getByLabel("Camera composition").selectOption(shot);
    await page.waitForFunction(
      (shot) =>
        window.aethyrforge
          .snapshot()
          .receipts.some(
            (receipt) => receipt.ok && receipt.message === `Camera ${shot}`,
          ),
      shot,
    );
    const downloaded = page.waitForEvent("download");
    await page.evaluate((time) => window.aethyrforge.capture(time), time);
    const download = await downloaded;
    const path = resolve(output, `${shot}.png`);
    await download.saveAs(path);
    const png = await readFile(path);
    assert.equal(png.readUInt32BE(16), 1280);
    assert.equal(png.readUInt32BE(20), 720);
    report.views.push({
      shot,
      label,
      path,
      settings: (await page.evaluate(() => window.aethyrforge.snapshot()))
        .environment,
    });
  }
  // Only this isolated profile can fetch these temporary in-memory image routes.
  const imageRoot = `/__aethyrforge-comparison/${randomUUID()}`;
  for (const view of report.views)
    await context.route(`**${imageRoot}/${view.shot}.png`, (route) =>
      route.fulfill({ contentType: "image/png", path: view.path }),
    );
  const downloaded = page.waitForEvent("download");
  await page.evaluate(
    async ({ views, imageRoot, time }) => {
      const images = [];
      try {
        for (const view of views)
          images.push(
            await createImageBitmap(
              await (await fetch(`${imageRoot}/${view.shot}.png`)).blob(),
            ),
          );
        // Keep each 1280x720 render intact: no cropping, grading, or resampling.
        const canvas = document.createElement("canvas");
        canvas.width = 2560;
        canvas.height = 784;
        const draw = canvas.getContext("2d");
        if (!draw) throw Error("A comparison image could not be created.");
        draw.fillStyle = "#111820";
        draw.fillRect(0, 0, canvas.width, canvas.height);
        images.forEach((image, i) => draw.drawImage(image, i * 1280, 0));
        draw.fillStyle = "#f3f2ed";
        draw.font = "24px system-ui, sans-serif";
        views.forEach((view, i) =>
          draw.fillText(view.label, i * 1280 + 24, 760),
        );
        draw.fillStyle = "#aeb9c5";
        draw.font = "18px system-ui, sans-serif";
        draw.textAlign = "right";
        draw.fillText(`Aethyrforge · Medieval Town · ${time}s`, 2536, 760);
        const blob = await new Promise((accept, reject) =>
          canvas.toBlob(
            (value) =>
              value
                ? accept(value)
                : reject(Error("The comparison image could not be saved.")),
            "image/png",
          ),
        );
        const href = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = href;
        link.download = "MT-01-comparison.png";
        link.click();
        setTimeout(() => URL.revokeObjectURL(href), 5000);
      } finally {
        images.forEach((image) => image.close());
      }
    },
    {
      views: report.views.map(({ shot, label }) => ({ shot, label })),
      imageRoot,
      time,
    },
  );
  const comparison = await downloaded;
  const path = resolve(output, "MT-01-comparison.png");
  await comparison.saveAs(path);
  const png = await readFile(path);
  assert.equal(png.readUInt32BE(16), 2560);
  assert.equal(png.readUInt32BE(20), 784);
  assert.deepEqual(report.errors, []);
  report.comparison = {
    path,
    width: 2560,
    height: 784,
    unmodifiedConstituentRenders: true,
  };
  report.ok = true;
  console.log("Saved the day/night comparison and both original renders.");
} catch (error) {
  report.failure = String(error.stack || error);
  process.exitCode = 1;
} finally {
  await writeFile(
    resolve(output, "verification.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await context.close();
  await browser.close();
}
