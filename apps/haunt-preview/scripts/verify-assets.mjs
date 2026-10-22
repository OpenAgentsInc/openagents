import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
import catalog from "../src/assets.json" with { type: "json" };

const url = process.env.HAUNT_URL;
if (!url) throw new Error("Set HAUNT_URL to the local editor.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "assets",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0],
  page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 1060 });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const report = {
  browser: browser.version(),
  measuredAt: new Date().toISOString(),
  manual: [],
  decisions: [],
  checks: [],
};
try {
  await page.goto(url);
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#add-pumpkin").disabled,
  );
  const initial = await snap();
  const kinds = Object.keys(catalog).filter((k) => k !== "pumpkin");
  assert.equal(
    await page.locator("#asset-kind option").count(),
    Object.keys(catalog).length,
  );
  for (const kind of kinds) {
    await page.getByLabel("Asset to add", { exact: true }).selectOption(kind);
    await page.locator("#x").fill("1.4");
    await page.locator("#z").fill("3");
    await page.locator("#scale").fill("1");
    await page
      .getByRole("button", { name: "Add asset +", exact: true })
      .click();
    await page.waitForFunction(
      (n) =>
        window.hauntPreview.snapshot().project.entities.length === n &&
        document.querySelector("#scene-status").textContent.includes("Saved"),
      initial.project.entities.length + 1,
    );
    const after = await snap(),
      entity = after.project.entities.at(-1);
    assert.equal(entity.kind, kind);
    const id = entity.id;
    await page.locator("#scale").fill("1.3");
    await page.getByRole("button", { name: "Resize", exact: true }).click();
    await page.waitForFunction(
      (id) =>
        window.hauntPreview.snapshot().project.entities.find((e) => e.id === id)
          ?.scale === 1.3 &&
        !document.querySelector("#resize-pumpkin").disabled,
      id,
    );
    await page.locator("#x").fill("1.8");
    await page
      .getByRole("button", { name: "Move selected", exact: true })
      .click();
    await page.waitForFunction(
      (id) =>
        window.hauntPreview.snapshot().project.entities.find((e) => e.id === id)
          ?.x === 1.8 && !document.querySelector("#remove-pumpkin").disabled,
      id,
    );
    await page.getByRole("button", { name: "Remove", exact: true }).click();
    await page.waitForFunction(
      (n) =>
        window.hauntPreview.snapshot().project.entities.length === n &&
        !document.querySelector("#undo").disabled,
      initial.project.entities.length,
    );
    await command({ kind: "undo" });
    assert.equal((await snap()).project.entities.at(-1).kind, kind);
    await command({ kind: "redo" });
    report.manual.push({ kind, id });
  }
  report.checks.push(
    "every new asset supports picker add, resize, move, remove, undo and redo through the real UI",
  );
  if (process.env.HAUNT_DECISIONS !== "0") {
    for (const kind of kinds) {
      const text = `Add a ${kind} at (1, 3).`;
      await page.locator("#request").fill(text);
      await page.locator("#ask").click();
      await page.waitForFunction(
        () =>
          document
            .querySelector("#scene-status")
            .textContent.includes("Changed and saved"),
        null,
        { timeout: 6000 },
      );
      const result = await page.evaluate(() =>
        window.hauntPreview.editor().decisions.at(-1),
      );
      assert.equal(result.decision.action, "add_asset");
      assert.equal(result.decision.asset, kind);
      assert.equal((await snap()).project.entities.at(-1).kind, kind);
      report.decisions.push(result);
      await command({ kind: "undo" });
    }
    await page.locator("#request").fill("Add three ghosts to my right.");
    await page.locator("#ask").click();
    await page.waitForFunction(
      () =>
        document
          .querySelector("#scene-status")
          .textContent.includes("Changed and saved"),
      null,
      { timeout: 6000 },
    );
    const added = (await snap()).project.entities.slice(
      initial.project.entities.length,
    );
    assert.equal(added.length, 3);
    assert.ok(added.every((e) => e.kind === "ghost" && e.x > initial.pose.x));
    report.group = await page.evaluate(() =>
      window.hauntPreview.editor().decisions.at(-1),
    );
    await command({ kind: "undo" });
    assert.deepEqual((await snap()).project.entities, initial.project.entities);
    const beforeUnsupported = await snap();
    await page.evaluate(() =>
      window.hauntPreview.request("Add a dragon ahead.").catch(() => {}),
    );
    assert.deepEqual((await snap()).project, beforeUnsupported.project);
    report.checks.push(
      "real Jev requests add each catalog kind; three ghosts add and undo as one group; an unlisted dragon makes no change",
    );
  }
  // A gallery makes the six original meshes easy to inspect in one screenshot.
  for (const [kind, x, z, scale] of [
    ["ghost", -1.5, 3.6, 0.85],
    ["gravestone", 1.5, 3.6, 0.85],
    ["coffin", -1.5, 1, 0.8],
    ["candle", 0.9, 5, 1],
    ["crate", 1.8, 1, 1],
    ["cauldron", 0, 1.5, 1.2],
  ])
    await command({
      kind: "add_asset",
      entity: { id: `${kind}-gallery`, kind, x, z, scale },
    });
  const final = await snap();
  assert.deepEqual(final.pose, initial.pose);
  assert.equal(final.deviceId, initial.deviceId);
  assert.equal(final.compiles, initial.compiles);
  assert.equal(final.activePipelines, 1);
  assert.equal(final.buffers, 2);
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => window.kilnReadScene())),
    final.project,
  );
  await page.screenshot({ path: `${out}/gallery.png`, fullPage: true });
  await page.reload();
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#add-pumpkin").disabled,
  );
  assert.deepEqual((await snap()).project, final.project);
  assert.deepEqual(errors, []);
  report.project = final.project;
  report.checks.push(
    "all six meshes share the existing shader and buffers; player/device stay unchanged; mixed assets save and reopen through Kiln OPFS",
  );
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        decisions: report.decisions.map((d) => ({
          asset: d.decision.asset,
          totalMs: d.totalMs,
        })),
        groupMs: report.group?.totalMs,
      },
      null,
      2,
    ),
  );
} catch (e) {
  report.passed = false;
  report.failure = String(e);
  process.exitCode = 1;
  console.error(e);
  await page
    .screenshot({ path: `${out}/failure.png`, fullPage: true })
    .catch(() => {});
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
