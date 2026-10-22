import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const base = process.env.HAUNT_URL;
if (!base) throw new Error("Set HAUNT_URL to the local creator server.");
const out = resolve(process.env.HAUNT_EVIDENCE || "dist/verification", "town");
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0],
  page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 960 });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const ready = (p) =>
  p.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
    null,
    { timeout: 30000 },
  );
const settled = async () => {
  const frame = (await snap()).frame;
  await page.waitForFunction(
    (f) => window.hauntPreview.snapshot().frame > f + 5,
    frame,
  );
};
async function go(axis, target) {
  const before = await snap(),
    value = before.pose[axis];
  if (Math.abs(value - target) < 0.15) return;
  const key =
    axis === "x"
      ? target > value
        ? "KeyD"
        : "KeyA"
      : target > value
        ? "KeyS"
        : "KeyW";
  await page.locator("#house").focus();
  await page.keyboard.down(key);
  try {
    await page.waitForFunction(
      ({ axis, target, positive }) =>
        positive
          ? window.hauntPreview.snapshot().pose[axis] >= target - 0.1
          : window.hauntPreview.snapshot().pose[axis] <= target + 0.1,
      { axis, target, positive: target > value },
      { timeout: 16000 },
    );
  } finally {
    await page.keyboard.up(key);
  }
  await settled();
}
async function ask(text) {
  await page.locator("#request").fill(text);
  await page.locator("#request").press("Enter");
  await page.waitForFunction(
    () =>
      document
        .querySelector("#scene-status")
        .textContent.includes("Changed and saved"),
    null,
    { timeout: 6000 },
  );
  return page.evaluate(() => window.hauntPreview.editor().decisions.at(-1));
}
const report = { checks: [], decisions: [] };
try {
  await page.goto(new URL("/medieval/", base).href);
  await ready(page);
  const initial = await snap();
  assert.equal(initial.project.id, "medieval-town-v1");
  assert.equal(initial.medieval.available, true);
  assert.equal(initial.medieval.meshes, 8);
  assert.ok(
    initial.project.entities.every(
      (e) => !["pumpkin", "ghost", "coffin", "gravestone"].includes(e.kind),
    ),
  );
  const rect = await page.locator("#house").boundingBox();
  assert.equal(rect.width, 1440);
  assert.equal(rect.height, 960);
  assert.equal(await page.locator("#asset-panel").isVisible(), false);
  assert.equal(await page.locator("#build-panel").isVisible(), false);
  await page.getByRole("button", { name: "2 Assets" }).click();
  assert.equal(await page.locator("#asset-panel").isVisible(), true);
  assert.equal(await page.locator("#asset-kind option").count(), 8);
  await page.keyboard.press("Escape");
  await page.screenshot({ path: `${out}/town-outside.png` });
  report.checks.push(
    "Separate Medieval Town Lab fills the viewport, with a floating hotbar, closed tool popovers, eight kit assets, daylight, and no spooky entities.",
  );
  await go("z", -8);
  assert.equal((await snap()).room.room.name, "Front room");
  await page.locator("#house").focus();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(1400);
  await page.keyboard.up("KeyW");
  await settled();
  const blocked = await snap();
  assert.ok(blocked.pose.z > -10);
  const wall = blocked.aimedWall.id;
  const removed = await ask("knock down this wall");
  report.decisions.push(removed);
  const open = await snap();
  assert.deepEqual(open.pose, blocked.pose);
  assert.equal(open.deviceId, initial.deviceId);
  assert.equal(open.hash, initial.hash);
  assert.ok(
    open.project.town.buildings[
      "cottage-starter"
    ].floors[0].removedWalls.includes(wall),
  );
  assert.ok(open.medieval.parts < blocked.medieval.parts);
  await go("z", -11.5);
  assert.equal((await snap()).room.room.name, "Back room");
  await go("z", -8);
  report.checks.push(
    "Player enters the cottage through its actual doorway, stops at an interior wall, removes that wall with a real fast request, and walks through the opening; shader/device/pose remain stable during the edit.",
  );
  const addRoom = await ask("add a room to my left");
  report.decisions.push(addRoom);
  let current = await snap();
  const room =
    current.project.town.buildings["cottage-starter"].floors[0].rooms.at(-1);
  const overlapStart = Math.max(0, room.z - room.depth / 2),
    overlapEnd = Math.min(5, room.z + room.depth / 2),
    doorZ = -10 + (overlapStart + overlapEnd) / 2;
  await go("z", doorZ);
  await go("x", -9 + room.x);
  const entered = await snap();
  assert.ok(entered.pose.x < -13);
  const beforeUndo = entered.pose;
  await command({ kind: "undo" });
  assert.deepEqual((await snap()).pose, beforeUndo);
  await command({ kind: "redo" });
  await page.screenshot({ path: `${out}/cottage-interior.png` });
  const wing = await ask("add a small wing to my left");
  report.decisions.push(wing);
  assert.equal(
    (await snap()).project.town.buildings["cottage-starter"].floors[0].rooms
      .length,
    6,
  );
  await command({ kind: "undo" });
  assert.equal(
    (await snap()).project.town.buildings["cottage-starter"].floors[0].rooms
      .length,
    3,
  );
  await go("x", -11);
  await go("z", 3);
  await go("x", 8);
  await go("z", -6.8);
  await go("x", 13.025);
  await go("z", -14.4);
  current = await snap();
  assert.ok(Math.abs(current.pose.y - 4) < 0.01);
  assert.equal(current.room.level, 1);
  await go("x", 11.5);
  await page.screenshot({ path: `${out}/townhouse-upstairs.png` });
  report.checks.push(
    "A room extension gains kit walls, floor, ceiling, and a real connecting doorway. Undo/redo keeps position. Player then enters the townhouse and walks up its stairs to the second floor.",
  );
  const upstairs = await ask("add a room to my left");
  report.decisions.push(upstairs);
  current = await snap();
  const upper =
    current.project.town.buildings["townhouse-starter"].floors[1].rooms.at(-1);
  const zTop = Math.max(-5, upper.z - upper.depth / 2),
    zBottom = Math.min(0, upper.z + upper.depth / 2),
    upperDoor = -10 + (zTop + zBottom) / 2;
  await go("z", upperDoor);
  await go("x", 10 + upper.x);
  const occupied = await snap();
  const rejection = await page.evaluate(() =>
    window.hauntPreview.command({ kind: "undo" }).then(
      () => "",
      (e) => String(e),
    ),
  );
  assert.match(rejection, /Step back/);
  assert.deepEqual((await snap()).project, occupied.project);
  assert.deepEqual((await snap()).pose, occupied.pose);
  await go("x", 8);
  await command({ kind: "undo" });
  const chair = await ask("Add a chair at (9, -12).");
  report.decisions.push(chair);
  assert.equal((await snap()).project.entities.at(-1).y, 4);
  const old = await snap();
  const bad = await page.evaluate(() =>
    window.hauntPreview.save("this is invalid WGSL"),
  );
  assert.equal(bad.type, "rejected");
  assert.equal((await snap()).hash, old.hash);
  assert.deepEqual((await snap()).pose, old.pose);
  await page.evaluate(() =>
    window.hauntPreview.save(window.hauntPreview.original()),
  );
  const saved = (await snap()).project;
  await page.reload();
  await ready(page);
  assert.deepEqual((await snap()).project, saved);
  report.checks.push(
    "Upstairs rooms and furniture use the correct floor; undo refuses to remove an occupied upper floor. Bad WGSL keeps the last working scene and pose. Town rooms and furniture reopen from real Kiln OPFS.",
  );
  const spawned = await ask("Add a cottage to my left.");
  report.decisions.push(spawned);
  const house = (await snap()).project.entities.at(-1);
  assert.equal(house.kind, "cottage");
  assert.ok(house.x < initial.pose.x);
  await go("z", house.z + 8);
  await go("x", house.x - 2);
  await go("z", house.z + 2.5);
  assert.equal((await snap()).room.entity.id, house.id);
  await page.screenshot({ path: `${out}/new-house-inside.png` });
  await go("z", house.z + 8);
  await command({ kind: "undo" });
  const restored = (await snap()).project;
  assert.deepEqual(restored.town, saved.town);
  report.checks.push(
    "A newly requested cottage gets its own rooms, collision, doorway and interior; a small wing adds three connected rooms and undoes in one step.",
  );
  // Preserve the final revision across the following workspace-isolation checks.
  const finalSaved = restored;
  // A separate Haunted House workspace keeps its rooms and drops old medieval additions.
  const haunt = await context.newPage();
  await haunt.goto(new URL("/", base).href);
  await ready(haunt);
  assert.equal(await haunt.locator("#asset-kind option").count(), 7);
  let hp = await haunt.evaluate(() => window.hauntPreview.snapshot().project);
  assert.equal(hp.id, "haunt-webgpu-v1");
  await haunt.evaluate(() =>
    window.hauntPreview.command({ kind: "set_tint", tint: "blue" }),
  );
  hp = await haunt.evaluate(() => window.hauntPreview.snapshot().project);
  const legacy = {
    ...hp,
    revision: hp.revision + 1,
    entities: [
      ...hp.entities,
      { id: "cottage-old-test", kind: "cottage", x: -12, z: -13, scale: 1 },
    ],
    layout: {
      rooms: [
        ...(hp.layout?.rooms ?? [
          {
            id: "hall",
            name: "Lantern Hall",
            x: 0,
            z: 1.5,
            width: 7,
            depth: 13,
          },
          { id: "passage", name: "Passage", x: 0, z: -7, width: 2.5, depth: 4 },
          { id: "gallery", name: "Gallery", x: 0, z: -13, width: 8, depth: 8 },
        ]),
        {
          id: "old-yard",
          name: "Courtyard",
          x: -12,
          z: -13,
          width: 16,
          depth: 16,
          outdoor: true,
        },
      ],
      removedWalls: hp.layout?.removedWalls ?? [],
    },
  };
  await haunt.evaluate(async (p) => {
    window.kilnSceneChanged = undefined;
    await window.kilnWrite("haunt/scene.json", JSON.stringify(p));
  }, legacy);
  await haunt.reload();
  await ready(haunt);
  const cleaned = await haunt.evaluate(() => window.hauntPreview.snapshot());
  assert.equal(cleaned.project.tint, "blue");
  assert.equal(cleaned.project.entities.length, hp.entities.length);
  assert.equal(
    cleaned.project.layout.rooms.length,
    hp.layout?.rooms.length ?? 3,
  );
  if (hp.layout) assert.deepEqual(cleaned.project.layout, hp.layout);
  assert.equal(cleaned.medieval.available, false);
  assert.deepEqual(
    await haunt.evaluate(() => JSON.parse(window.kilnReadScene())),
    cleaned.project,
  );
  await page.reload();
  await ready(page);
  assert.deepEqual((await snap()).project, finalSaved);
  report.checks.push(
    "The two real Kiln workspaces keep different scene/material paths and OPFS snapshots. Haunted House keeps spooky props and lighting while old medieval prefabs and outdoor additions are removed and the cleaned save is written back.",
  );
  assert.deepEqual(errors, []);
  report.final = await snap();
  report.passed = true;
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(() => window.hauntPreview.snapshot().disposed);
  assert.equal((await snap()).buffers, 0);
  assert.equal((await snap()).medieval.textures, 0);
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        decisions: report.decisions.map((d) => ({
          text: d.text,
          totalMs: d.totalMs,
          action: d.decision.action,
        })),
      },
      null,
      2,
    ),
  );
} catch (error) {
  report.passed = false;
  report.failure = String(error);
  report.state = await snap().catch(() => null);
  process.exitCode = 1;
  console.error(error);
  for (const key of ["KeyW", "KeyS", "KeyA", "KeyD"])
    await page.keyboard.up(key);
  await page.screenshot({ path: `${out}/failure.png` }).catch(() => {});
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
