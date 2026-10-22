import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";

const url = process.env.HAUNT_URL;
if (!url) throw new Error("Set HAUNT_URL to the local editor.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "layout",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0],
  page = await context.newPage();
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
const report = {
  browser: browser.version(),
  measuredAt: new Date().toISOString(),
  checks: [],
  decisions: [],
};
try {
  await page.goto(url);
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
  );
  const initial = await snap();
  report.initial = {
    project: initial.project,
    pose: initial.pose,
    hash: initial.hash,
  };
  assert.equal(await page.locator("#room-count").textContent(), "3 rooms");
  assert.equal(initial.aimedWall.id, "gallery:north");
  const removed = await ask("knock down this wall");
  report.decisions.push(removed);
  const open = await snap();
  assert.ok(open.project.layout.removedWalls.includes("gallery:north"));
  assert.equal(open.hash, initial.hash);
  assert.equal(open.deviceId, initial.deviceId);
  assert.deepEqual(open.pose, initial.pose);
  await command({ kind: "undo" });
  assert.equal((await snap()).project.layout, undefined);
  await page.locator("#wall-selected").selectOption("hall:east");
  report.decisions.push(await ask("add a room"));
  const room = await snap();
  assert.equal(room.project.layout.rooms.length, 4);
  assert.equal(room.project.layout.rooms.at(-1).x, 6.5);
  assert.equal(await page.locator("#room-count").textContent(), "4 rooms");
  assert.equal(room.hash, initial.hash);
  assert.deepEqual(room.pose, initial.pose);
  await command({ kind: "undo" });
  await page.locator("#wall-selected").selectOption("aim");
  report.decisions.push(await ask("add a wing to my right"));
  const wing = await snap();
  assert.equal(wing.project.layout.rooms.length, 6);
  assert.equal(wing.project.revision, room.project.revision + 2);
  assert.deepEqual(wing.pose, initial.pose);
  assert.equal(wing.compiles, initial.compiles);
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => window.kilnReadScene())),
    wing.project,
  );
  await command({ kind: "undo" });
  assert.equal((await snap()).project.layout, undefined);
  await command({ kind: "redo" });
  assert.deepEqual((await snap()).project.layout, wing.project.layout);
  report.checks.push(
    "real exact text requests knock down an aimed wall, add a selected-wall room, and add a three-room wing to the right; one-step undo/redo, pose/device/shader preservation, and real Kiln saves",
  );
  // Enter all three rooms through their doorways at z=6.0.
  const surface = page.locator("#house");
  await surface.focus();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(300);
  await page.keyboard.up("KeyW");
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().pose.z < 6.25,
  );
  await surface.focus();
  await page.keyboard.down("KeyD");
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().pose.x > 4.5,
    null,
    { timeout: 5000 },
  );
  await page.keyboard.up("KeyD");
  const entered = await snap();
  assert.ok(entered.pose.x > 3.5);
  await page.waitForFunction(
    (frame) => window.hauntPreview.snapshot().frame > frame + 5,
    entered.frame,
  );
  const beforeUndo = await snap();
  const rejection = await page.evaluate(() =>
    window.hauntPreview.command({ kind: "undo" }).then(
      () => "unexpected acceptance",
      (e) => String(e),
    ),
  );
  assert.match(rejection, /Step back/);
  assert.deepEqual((await snap()).project, beforeUndo.project);
  assert.deepEqual((await snap()).pose, beforeUndo.pose);
  // Place and animate an asset beyond the original house bounds.
  await command({
    kind: "add_asset",
    entity: {
      id: "ghost-wing-proof",
      kind: "ghost",
      x: 6.5,
      z: 6,
      scale: 1,
      behavior: "float",
    },
  });
  await page.waitForFunction(
    () =>
      window.hauntPreview
        .snapshot()
        .actors.find((a) => a.id === "ghost-wing-proof")?.y > 0.02,
  );
  report.checks.push(
    "player walks through the new doorway; undo refuses to remove floor beneath the player; assets and ghost behavior work inside added rooms",
  );
  const beforeInvalid = await snap();
  assert.ok(
    await page.evaluate(async () => {
      try {
        await window.hauntPreview.command({
          kind: "add_room",
          wall: "missing",
          size: 6,
        });
        return false;
      } catch {
        return true;
      }
    }),
  );
  assert.deepEqual((await snap()).project, beforeInvalid.project);
  const shader = await page.evaluate(() => window.hauntPreview.original());
  assert.equal(
    (await page.evaluate(() => window.hauntPreview.save("broken WGSL"))).type,
    "rejected",
  );
  assert.equal((await snap()).hash, beforeInvalid.hash);
  assert.deepEqual((await snap()).project, beforeInvalid.project);
  await page.evaluate((s) => window.hauntPreview.save(s), shader);
  const saved = await snap();
  const reopened = await context.newPage();
  await reopened.goto(url);
  await reopened.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
  );
  assert.deepEqual(
    await reopened.evaluate(() => window.hauntPreview.snapshot().project),
    saved.project,
  );
  await reopened.close();
  assert.equal(saved.buffers, 2);
  assert.equal(saved.activePipelines, 1);
  assert.deepEqual(errors, []);
  await page
    .locator("#wall-selected")
    .selectOption(wing.project.layout.rooms.at(-1).id + ":east");
  await page.locator("#add-room").click();
  await page.waitForFunction(
    () =>
      window.hauntPreview.snapshot().project.layout.rooms.length === 7 &&
      document.querySelector("#scene-status").textContent.includes("Saved"),
  );
  await surface.focus();
  await page.keyboard.down("ArrowRight");
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().pose.yaw > 1.5,
  );
  await page.keyboard.up("ArrowRight");
  await page.locator("#request").fill("add a wing to my right");
  await page.screenshot({ path: `${out}/expanded-house.png`, fullPage: true });
  await page.locator("#floorplan").screenshot({ path: `${out}/floorplan.png` });
  report.final = await snap();
  report.checks.push(
    "layout and new-room assets reopen through Kiln OPFS; failed shaders keep the working view/layout; direct construction controls add another connected room with bounded resources",
  );
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(() => window.hauntPreview.snapshot().disposed);
  assert.equal((await snap()).buffers, 0);
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        decisions: report.decisions.map((r) => ({
          text: r.text,
          action: r.decision.action,
          totalMs: r.totalMs,
        })),
        rooms: report.final.project.layout.rooms.length,
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
  await page.keyboard.up("KeyD").catch(() => {});
  await page
    .screenshot({ path: `${out}/failure.png`, fullPage: true })
    .catch(() => {});
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
