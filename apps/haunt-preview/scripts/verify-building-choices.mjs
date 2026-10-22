import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const base = process.env.HAUNT_URL;
if (!base) throw new Error("Set HAUNT_URL to the local creator server.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "building-choices",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 1440, height: 960 });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const ready = () =>
  page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
    null,
    { timeout: 30000 },
  );
const report = { checks: [], decisions: [] };
try {
  await page.goto(new URL("/medieval/", base).href);
  await ready();
  const initial = await snap();
  for (const text of [
    "add 2 buildings your choice anywhere u want",
    "place three buildings wherever there is room",
    "add a building, surprise me, you choose where",
    "add two cottages anywhere",
  ]) {
    const before = await snap(),
      previous = await page.evaluate(
        () => window.hauntPreview.editor().decisions.length,
      );
    await page.locator("#request").fill(text);
    await page.locator("#request").press("Enter");
    await page.waitForFunction(
      (n) => window.hauntPreview.editor().decisions.length > n,
      previous,
      { timeout: 6000 },
    );
    const decision = await page.evaluate(() =>
      window.hauntPreview.editor().decisions.at(-1),
    );
    const after = await snap(),
      added = after.project.entities.slice(before.project.entities.length);
    const count = text.includes("three")
      ? 3
      : text.includes("a building")
        ? 1
        : 2;
    assert.equal(added.length, count);
    assert.ok(added.every((e) => ["cottage", "townhouse"].includes(e.kind)));
    assert.equal(decision.decision.placement, "anywhere");
    assert.deepEqual(after.pose, before.pose);
    assert.equal(after.deviceId, before.deviceId);
    assert.equal(after.hash, before.hash);
    assert.ok(
      added.every((e) => Object.hasOwn(after.project.town.buildings, e.id)),
    );
    report.decisions.push(decision);
    if (report.decisions.length === 1) {
      assert.deepEqual(
        added.map((e) => e.kind),
        ["cottage", "townhouse"],
      );
      await page.screenshot({ path: `${out}/exact-request.png` });
    }
    await command({ kind: "undo" });
    assert.deepEqual((await snap()).project.entities, before.project.entities);
    await command({ kind: "redo" });
    assert.deepEqual((await snap()).project.entities, after.project.entities);
    await page.reload();
    await ready();
    assert.deepEqual((await snap()).project.entities, after.project.entities);
    for (const e of added) await command({ kind: "remove_asset", id: e.id });
  }
  report.checks.push(
    "Exact user request and three paraphrases add the literal count, choose supported styles and clear plots, preserve pose/device/shader, undo and redo as one edit, and reopen from Kiln.",
  );
  for (const text of [
    "do not add 2 buildings anywhere",
    "add 2 buildings and launch a spaceship",
    "add a building and a ghost",
  ]) {
    const response = await page.request.post(
      new URL("/api/medieval/decision", base).href,
      {
        data: { text, selected: false, lab: "medieval" },
        headers: { Origin: new URL(base).origin },
      },
    );
    const d = await response.json();
    assert.ok(
      d.action === "unsupported" || d.needs_review,
      JSON.stringify({ text, d }),
    );
  }
  report.checks.push(
    "Negated requests and unsupported later changes remain refused.",
  );
  assert.equal(
    (await snap()).project.entities.length,
    initial.project.entities.length,
  );
  assert.deepEqual(errors, []);
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        passed: report.passed,
        checks: report.checks,
        decisions: report.decisions.map((d) => ({
          text: d.text,
          totalMs: d.totalMs,
        })),
      },
      null,
      2,
    ),
  );
} catch (e) {
  report.error = String(e);
  report.state = await snap().catch(() => null);
  await page.screenshot({ path: `${out}/failure.png` }).catch(() => {});
  throw e;
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
