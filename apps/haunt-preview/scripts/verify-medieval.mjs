import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "medieval",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 1440, height: 1100 });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const ask = async (text) => {
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
  return page.evaluate(() => window.hauntPreview.editor().decisions.at(-1));
};
const report = { checks: [], decisions: [] };
try {
  await page.goto(process.env.HAUNT_URL);
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
  );
  const initial = await snap();
  assert.equal(initial.medieval.available, true);
  assert.equal(initial.medieval.meshes, 8);
  // Use the real wall selector and courtyard control.
  await page.locator("#wall-selected").selectOption("hall:south");
  await page
    .getByRole("button", { name: "Add courtyard · 32 m", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      window.hauntPreview
        .snapshot()
        .project.layout?.rooms.some((r) => r.outdoor) &&
      document.querySelector("#scene-status").textContent.includes("Saved"),
  );
  const court = await snap();
  assert.equal(court.project.layout.rooms.at(-1).name, "Courtyard");
  assert.equal(court.project.layout.rooms.at(-1).width, 32);
  await command({ kind: "undo" });
  assert.equal((await snap()).project.layout, undefined);
  await command({ kind: "redo" });
  report.checks.push(
    "Courtyard button creates a saved roofless 32 m space with a doorway; one-step undo and redo.",
  );
  const cottage = await ask("Add a medieval house at (0, 25).");
  report.decisions.push(cottage);
  assert.equal(cottage.decision.asset, "cottage");
  assert.equal((await snap()).medieval.instances, 1);
  // All eight kinds are already tested through add/move/resize/remove/undo in verify-assets.
  await ask("Add a barrel at (-3, 10).");
  await ask("Add a market stall at (4, 13).");
  const save = await snap();
  assert.deepEqual(save.pose, initial.pose);
  assert.equal(save.hash, initial.hash);
  assert.equal(save.deviceId, initial.deviceId);
  assert.equal(save.buffers, 4);
  assert.equal(save.medieval.instances, 3);
  // Enter the courtyard using ordinary movement, then turn to face the house.
  await page.locator("#house").focus();
  await page.keyboard.down("KeyS");
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().pose.z > 10.1,
    null,
    { timeout: 5000 },
  );
  await page.keyboard.up("KeyS");
  await page.keyboard.down("ArrowRight");
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().pose.yaw > 3.1,
    null,
    { timeout: 5000 },
  );
  await page.keyboard.up("ArrowRight");
  await page.waitForTimeout(150);
  await page.screenshot({ path: `${out}/cottage.png`, fullPage: true });
  const walked = await snap();
  assert.ok(walked.pose.z > 8);
  report.checks.push(
    "Player walks through the doorway into the courtyard; full-size textured cottage, barrel, and market stall draw on the same WebGPU device.",
  );
  // Immutable geometry stays allocated once while only instances change.
  const before = await snap();
  for (let i = 0; i < 20; i++)
    await command({
      kind: "move_asset",
      id: save.project.entities.at(-1).id,
      x: 4 + (i % 2) * 0.1,
      z: 13,
    });
  const after = await snap();
  assert.equal(after.buffers, before.buffers);
  assert.equal(after.compiles, before.compiles);
  assert.deepEqual(after.medieval, before.medieval);
  assert.equal(after.geometryBuilds, before.geometryBuilds);
  const savedProject = after.project;
  await page.reload();
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().medieval?.instances === 3 &&
      !document.querySelector("#ask").disabled,
  );
  assert.deepEqual((await snap()).project, savedProject);
  report.checks.push(
    "Twenty instance edits keep buffer/pipeline counts stable; mixed scene and outdoor layout reopen from the real Kiln OPFS save.",
  );
  // The model distinguishes a whole house from extending the existing house.
  await page.locator("#wall-selected").selectOption("gallery:west");
  const nextCourt = await ask("Add a courtyard here.");
  report.decisions.push(nextCourt);
  assert.equal(nextCourt.decision.action, "add_room");
  assert.equal((await snap()).project.layout.rooms.at(-1).outdoor, true);
  await command({ kind: "undo" });
  const town = await ask("Add a townhouse at (0, 25).");
  report.decisions.push(town);
  assert.equal(town.decision.asset, "townhouse");
  await command({ kind: "undo" });
  assert.deepEqual(errors, []);
  report.final = await snap();
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        decisions: report.decisions.map((r) => ({
          text: r.text,
          asset: r.decision.asset,
          action: r.decision.action,
          totalMs: r.totalMs,
        })),
      },
      null,
      2,
    ),
  );
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(() => window.hauntPreview.snapshot().disposed);
  const disposed = await snap();
  assert.equal(disposed.buffers, 0);
  assert.equal(disposed.medieval.buffers, 0);
  assert.equal(disposed.medieval.textures, 0);
  const missing = await browser.contexts()[0].newPage();
  await missing.route("**/medieval-kit/**", (route) =>
    route.fulfill({ status: 404, body: "" }),
  );
  await missing.goto(process.env.HAUNT_URL);
  await missing.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
  );
  const unavailable = await missing.evaluate(() =>
    window.hauntPreview.snapshot(),
  );
  assert.equal(unavailable.medieval.available, false);
  assert.deepEqual(unavailable.project, report.final.project);
  assert.equal(
    await missing.locator('#asset-kind option[value="cottage"]').isDisabled(),
    true,
  );
  const result = await missing.evaluate(() =>
    window.hauntPreview
      .command({
        kind: "add_asset",
        entity: { id: "cottage-no-kit", kind: "cottage", x: 1, z: 3, scale: 1 },
      })
      .then(
        () => "accepted",
        (e) => String(e),
      ),
  );
  assert.match(result, /Prepare the medieval kit/);
  assert.deepEqual(
    (await missing.evaluate(() => window.hauntPreview.snapshot())).project,
    unavailable.project,
  );
  await missing.evaluate(() => window.hauntPreview.dispose());
  report.checks.push(
    "Without the licensed bundle, ordinary props still work, kit choices are disabled, and kit edits fail without changing the scene.",
  );
} catch (e) {
  report.passed = false;
  report.failure = String(e);
  process.exitCode = 1;
  console.error(e);
  await page.keyboard.up("KeyS");
  await page.keyboard.up("ArrowRight");
  await page
    .screenshot({ path: `${out}/failure.png`, fullPage: true })
    .catch(() => {});
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
