/** Run through openagents browser run. All captures and draft evidence stay private. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep art and captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["127.0.0.1", "localhost"].includes(url.hostname) &&
    !url.username &&
    !url.password,
  "Verify a local creator server.",
);
const report = {
  schema: "aethyrforge.ground-props-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  ok: false,
  checks: [],
  captures: [],
  receipts: [],
  errors: [],
  console: [],
  failed: [],
  limitations: [
    "These are device-local, noncolliding decorations. This check does not claim shared MMO world editing or building interiors.",
    "The public-build flag is covered by CPU access-policy tests. This run verifies the player-path service guard and worker player authority without opening another GPU world.",
  ],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
const page = await context.newPage();
page.setDefaultTimeout(20_000);
let closing = false,
  reloading = false;
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 4096),
    });
});
page.on("requestfailed", (request) => {
  const error = request.failure()?.errorText;
  if (!closing && !(reloading && error === "net::ERR_ABORTED"))
    report.failed.push({ url: request.url(), error });
});
page.on("response", (response) => {
  if (!closing && response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const inspect = () =>
  page.evaluate(() => window.aethyrforge.agent.inspect({ limit: 16 }));
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
async function until(predicate, message, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const s = await status();
    if (
      s.phase === "error" ||
      ["error", "disconnected"].includes(s.mmorpg?.connection)
    )
      throw Error(`${message}: ${s.message}; ${s.mmorpg?.message || ""}`);
    const result = await predicate(s);
    if (result) return result === true ? s : result;
    await sleep(80);
  }
  throw Error(`${message}: ${JSON.stringify(await status())}`);
}
async function capture(name) {
  const path = resolve(output, `${name}.png`);
  await page.screenshot({ path });
  report.captures.push(path);
}
async function preview() {
  const handle = await page.locator("#preview-frame").elementHandle();
  const frame = await handle.contentFrame();
  assert(frame, "The existing world frame is missing.");
  return frame;
}
async function neutral() {
  for (const code of [
    "KeyW",
    "KeyA",
    "KeyS",
    "KeyD",
    "ShiftLeft",
    "Space",
    "Backspace",
  ])
    await page.keyboard.up(code).catch(() => {});
  await page
    .evaluate(() =>
      window.aethyrforgeEditor?.service.send({
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
          attack: false,
          dance: false,
        },
      }),
    )
    .catch(() => {});
}
async function ready() {
  await until(
    (s) => s.phase === "ready" && !!s.mmorpg,
    "The prepared grass world did not open",
    150_000,
  );
  await until(
    (s) => s.runtimeTables?.liveRecords === 8,
    "The eight real starter props did not activate",
  );
}
async function join(suffix) {
  const name = `Props-${Date.now().toString(36).slice(-6)}-${suffix}`;
  await page.locator("#mmo-name").fill(name);
  assert.equal(await page.locator("#mmo-class").inputValue(), "paladin");
  await page.getByRole("button", { name: "Enter world", exact: true }).click();
  const s = await until(
    (v) => v.mmorpg?.connection === "online" && v,
    "The anonymous Paladin did not join",
  );
  assert.equal(s.mmorpg.player.class, "paladin");
  await page
    .locator(".mmo-login")
    .waitFor({ state: "detached", timeout: 3000 });
  await neutral();
  return until(
    (v) =>
      v.mmorpg.player.alive &&
      v.mmorpg.player.prediction?.grounded &&
      Math.abs(v.mmorpg.player.prediction.velocity) < 0.015 &&
      v,
    "The player did not settle on the real terrain",
  );
}
async function acceptedUI(action, description) {
  const previous = new Set((await snapshot()).receipts.map((r) => r.requestId));
  await action();
  const result = await until(async () => {
    const receipts = (await snapshot()).receipts.filter(
      (r) => !previous.has(r.requestId),
    );
    return receipts[0] || false;
  }, `${description} produced no receipt`);
  report.receipts.push({ description, ...result });
  assert.equal(result.ok, true, `${description}: ${result.message}`);
  return result;
}
function samePlayer(before, after, label) {
  assert.equal(
    after.mmorpg.player.id,
    before.mmorpg.player.id,
    `${label} changed the player ID`,
  );
  const moved = distance(
    before.mmorpg.player.position,
    after.mmorpg.player.position,
  );
  assert.ok(moved <= 0.05, `${label} moved the player ${moved} metres`);
  return moved;
}
async function openEdit() {
  const before = await status();
  await acceptedUI(
    () => page.getByRole("button", { name: "Edit", exact: true }).click(),
    "Enter Edit",
  );
  const after = await until(
    (s) => s.mmorpgEditing === true && s,
    "Edit was not acknowledged by the worker",
  );
  await page
    .getByRole("region", { name: "World editor", exact: true })
    .waitFor();
  samePlayer(before, after, "Entering Edit");
  return after;
}
async function selectByPointer(objects, catalog) {
  await page.getByRole("button", { name: "Close tool", exact: true }).click();
  const candidates = await page.evaluate(
    async ({ objects, catalog }) => {
      const { multiply, perspective, lookAt, modelMatrix, transformPoint } =
        await import("/packages/renderer/math.ts");
      const { rayFromNdc, rayBox } =
        await import("/packages/renderer/picking.ts");
      const s = window.aethyrforge.snapshot().status;
      const iframe = document
        .getElementById("preview-frame")
        .getBoundingClientRect();
      const matrix = multiply(
        perspective(
          Math.PI / 3,
          iframe.width / iframe.height,
          1 / 6,
          10000 / 6,
        ),
        lookAt(s.view.eye, s.view.target),
      );
      const bounds = new Map(catalog.map((a) => [a.id, a.bounds]));
      const points = [];
      for (const object of objects) {
        const box = bounds.get(object.kind);
        for (const x of [0.3, 0.5, 0.7])
          for (const y of [0.3, 0.6, 0.8])
            for (const z of [0.3, 0.5, 0.7]) {
              const local = [x, y, z].map(
                (t, i) => box.min[i] + (box.max[i] - box.min[i]) * t,
              );
              const world = transformPoint(
                modelMatrix(object.position, object.yaw, object.scale),
                local,
              );
              const w =
                matrix[3] * world[0] +
                matrix[7] * world[1] +
                matrix[11] * world[2] +
                matrix[15];
              if (w <= 0) continue;
              const ndc = [0, 1].map(
                (i) =>
                  (matrix[i] * world[0] +
                    matrix[4 + i] * world[1] +
                    matrix[8 + i] * world[2] +
                    matrix[12 + i]) /
                  w,
              );
              if (Math.abs(ndc[0]) > 0.9 || ndc[1] < -0.65 || ndc[1] > 0.7)
                continue;
              const ray = rayFromNdc(matrix, ndc);
              const hits = objects
                .map((o) => ({
                  id: o.id,
                  distance: rayBox(s.view.eye, ray, bounds.get(o.kind), o),
                }))
                .filter((h) => h.distance !== null && h.distance < 80)
                .sort((a, b) => a.distance - b.distance);
              if (hits[0]?.id !== object.id) continue;
              points.push({
                id: object.id,
                kind: object.kind,
                ndc,
                x: iframe.x + (ndc[0] + 1) * 0.5 * iframe.width,
                y: iframe.y + (1 - ndc[1]) * 0.5 * iframe.height,
              });
            }
      }
      return points;
    },
    { objects, catalog },
  );
  assert(
    candidates.length,
    "No starter prop is visible from the unchanged player camera.",
  );
  const pick = candidates[0];
  await page.mouse.click(pick.x, pick.y);
  await until(
    (s) => s.selection?.id === pick.id,
    "Clicking the real projected prop did not select it",
  );
  report.pointerPick = pick;
  await page.getByRole("button", { name: "Select", exact: true }).click();
  return pick;
}
try {
  await page.goto(url.href);
  await ready();
  const scene = await page.evaluate(async () =>
    (await fetch("/private-assets/scene-ground.json")).json(),
  );
  const catalog = await page.evaluate(
    async (path) => (await fetch(path)).json(),
    scene.catalog,
  );
  assert.equal(catalog.assets.length, 8);
  assert.ok(
    catalog.assets.every(
      (a) =>
        a.kind === "prop" && a.id.startsWith("quaternius:fantasy-standard:"),
    ),
  );
  const initial = await inspect();
  assert.equal(initial.totalObjects, 8);
  assert.ok(initial.objects.every((o) => o.collidable === false));
  report.initialObjects = initial.objects;
  record(
    "Eight prepared Quaternius props render over the unchanged MMO grass world",
    {
      catalog: catalog.assets.map((a) => ({ id: a.id, name: a.name })),
      runtimeTables: (await status()).runtimeTables,
    },
  );
  const joined = await join("A");
  const playerId = joined.mmorpg.player.id;
  const entered = await openEdit();
  assert.equal(
    await page
      .getByRole("button", { name: "Statistics", exact: true })
      .isVisible(),
    false,
  );
  assert.equal(
    await page
      .getByRole("list", { name: "Available assets" })
      .getByRole("listitem")
      .count(),
    8,
  );
  const cardLayout = await page
    .locator(".ground-edit .tool-panel--assets")
    .evaluate((panel) => {
      const box = (element) => {
        const { top, bottom, left, right, width, height } =
          element.getBoundingClientRect();
        return { top, bottom, left, right, width, height };
      };
      const textBoxes = (element) => {
        const range = document.createRange();
        range.selectNodeContents(element);
        return Array.from(range.getClientRects(), (rect) => ({
          top: rect.top,
          bottom: rect.bottom,
          left: rect.left,
          right: rect.right,
        }));
      };
      return {
        panel: box(panel),
        grid: box(panel.querySelector(".asset-grid")),
        add: box(panel.querySelector(".primary-button")),
        cards: Array.from(panel.querySelectorAll(".asset-card"), (card) => ({
          bounds: box(card),
          name: card.querySelector(".asset-name").textContent,
          text: [".asset-name", ".asset-category"].flatMap((selector) =>
            textBoxes(card.querySelector(selector)),
          ),
        })),
      };
    });
  assert.equal(cardLayout.cards.length, 8);
  for (const card of cardLayout.cards) {
    assert.ok(
      card.bounds.height >= 77.5,
      `${card.name} has a collapsed asset card`,
    );
    assert.ok(card.text.length > 0, `${card.name} has no visible label text`);
    for (const text of card.text)
      assert.ok(
        text.top >= card.bounds.top - 1 &&
          text.bottom <= card.bounds.bottom + 1 &&
          text.left >= card.bounds.left - 1 &&
          text.right <= card.bounds.right + 1,
        `${card.name} clips its name or category`,
      );
  }
  assert.ok(
    cardLayout.grid.height <= 172,
    "The asset list must scroll within the compact panel",
  );
  assert.ok(
    cardLayout.add.top >= cardLayout.panel.top &&
      cardLayout.add.bottom <= cardLayout.panel.bottom + 1,
    "The Add button must remain inside the visible panel",
  );
  report.assetCardLayout = cardLayout;
  record(
    "Edit opens compact prop tools without replacing the player or camera",
    {
      playerId,
      movement: samePlayer(joined, entered, "Edit"),
    },
  );
  await capture("props-edit-assets");
  await page.getByTitle("Cauldron", { exact: true }).click();
  await page
    .getByRole("spinbutton", { name: "Asset count", exact: true })
    .fill("1");
  await acceptedUI(
    () =>
      page.getByRole("button", { name: "Add Cauldron", exact: true }).click(),
    "Add Cauldron through Assets",
  );
  await until(
    (s) => s.runtimeTables?.liveRecords === 9 && s.saved,
    "The ninth prop did not render and save",
  );
  const added = await inspect();
  assert.equal(added.totalObjects, 9);
  const created = added.objects.find(
    (o) => !initial.objects.some((old) => old.id === o.id),
  );
  assert.equal(created.kind, "quaternius:fantasy-standard:cauldron");
  await page.getByText("Saved on this device", { exact: true }).waitFor();
  record("Assets adds and saves a ninth real noncolliding Cauldron", {
    created,
  });
  const pick = await selectByPointer(added.objects, catalog.assets);
  const selected = added.objects.find((o) => o.id === pick.id);
  const requestedScale = Math.min(3, selected.scale + 0.5);
  const scaleInput = page.getByRole("spinbutton", {
    name: "Object scale",
    exact: true,
  });
  await acceptedUI(async () => {
    await scaleInput.fill(String(requestedScale));
    await scaleInput.press("Tab");
  }, "Change selected prop scale");
  await until(
    (s) => Math.abs(s.selection?.scale - requestedScale) < 1e-5,
    "The selected prop scale did not update",
  );
  await page.getByRole("button", { name: "Command", exact: true }).click();
  await page
    .getByRole("textbox", { name: "Describe a world change", exact: true })
    .fill("Make it bigger");
  await acceptedUI(
    () =>
      page.getByRole("button", { name: "Apply change", exact: true }).click(),
    "Make it bigger through fast command",
  );
  const enlarged = requestedScale * 1.25;
  await until(
    (s) => Math.abs(s.selection?.scale - enlarged) < 1e-5,
    "The bounded fast command did not enlarge the selected prop",
  );
  await acceptedUI(
    () => page.getByRole("button", { name: "Undo edit", exact: true }).click(),
    "Undo scale command",
  );
  await until(
    (s) => Math.abs(s.selection?.scale - requestedScale) < 1e-5,
    "Undo did not restore the prior scale",
  );
  await acceptedUI(
    () => page.getByRole("button", { name: "Redo edit", exact: true }).click(),
    "Redo scale command",
  );
  await until(
    (s) => Math.abs(s.selection?.scale - enlarged) < 1e-5 && s.saved,
    "Redo did not restore and save the enlarged prop",
  );
  await page.getByRole("button", { name: "Select", exact: true }).click();
  await capture("props-selected-transformed");
  record(
    "A real pointer selects a prop; scale, fast command, Undo and Redo use the same edit path",
    { pick, requestedScale, enlarged },
  );
  const saved = await inspect();
  report.savedObjects = saved.objects;
  const canvas = (await preview()).locator("#world");
  await canvas.focus();
  const beforeHeld = await status();
  for (const key of ["KeyW", "KeyA", "ShiftLeft", "Space"])
    await page.keyboard.down(key);
  await sleep(450);
  const duringHeld = await status();
  samePlayer(beforeHeld, duringHeld, "Holding gameplay keys during Edit");
  assert.equal(
    duringHeld.mmorpg.player.attacks,
    beforeHeld.mmorpg.player.attacks,
  );
  const inputState = (await inspect()).input;
  assert.equal(inputState.forward, 0);
  assert.equal(inputState.right, 0);
  assert.equal(inputState.sprint, false);
  await acceptedUI(
    () => page.getByRole("button", { name: "Done", exact: true }).click(),
    "Return to Play",
  );
  await until(
    (s) => s.mmorpgEditing === false,
    "Done did not restore Play mode",
  );
  await sleep(350);
  const played = await status();
  samePlayer(beforeHeld, played, "Returning to Play with previously held keys");
  await neutral();
  assert.equal((await inspect()).totalObjects, 9);
  record(
    "Edit suppresses movement and attacks; Done preserves identity and position with no stuck input",
    { playerId, inputState },
  );
  await openEdit();
  await page.getByRole("button", { name: "Close tool", exact: true }).click();
  await capture("props-cluster-before-reload");
  reloading = true;
  await page.reload({ waitUntil: "domcontentloaded" });
  await readyAfterReload();
  reloading = false;
  const rejoined = await join("B");
  assert.notEqual(
    rejoined.mmorpg.player.id,
    playerId,
    "Reload must join as a new anonymous player, not reuse a fabricated player state",
  );
  await openEdit();
  const restored = await inspect();
  assert.equal(
    (await status()).saved,
    true,
    "Restored edits must report their saved state",
  );
  assert.equal(restored.totalObjects, 9);
  const byId = new Map(restored.objects.map((o) => [o.id, o]));
  for (const before of saved.objects) {
    const after = byId.get(before.id);
    assert(after, `Missing restored prop ${before.id}`);
    assert.equal(after.kind, before.kind);
    assert.equal(after.collidable, false);
    assert.ok(distance(after.position, before.position) < 1e-5);
    assert.ok(Math.abs(after.scale - before.scale) < 1e-5);
    assert.ok(Math.abs(after.yaw - before.yaw) < 1e-5);
  }
  report.restoredObjects = restored.objects;
  await capture("props-restored-after-reload");
  record(
    "Reload restores all nine exact local prop identities, positions and scales for a new anonymous player",
    {
      previousPlayerId: playerId,
      newPlayerId: rejoined.mmorpg.player.id,
    },
  );
  await acceptedUI(
    () => page.getByRole("button", { name: "Done", exact: true }).click(),
    "Finish restored edit",
  );
  const pathGuard = await page.evaluate(() => {
    const previous = location.pathname + location.search + location.hash;
    history.replaceState(null, "", "/player.html");
    try {
      window.aethyrforgeEditor.service.mmorpgEditing(true);
      return { rejected: false };
    } catch (error) {
      return { rejected: true, message: String(error) };
    } finally {
      history.replaceState(null, "", previous);
    }
  });
  assert.equal(pathGuard.rejected, true);
  await page.evaluate(() =>
    window.aethyrforgeEditor.service.send({ type: "initialize", player: true }),
  );
  await sleep(250);
  const requestId = await page.evaluate(() =>
    window.aethyrforgeEditor.service.mmorpgEditing(true),
  );
  const denied = await until(
    async () =>
      (await snapshot()).receipts.find((r) => r.requestId === requestId) ||
      false,
    "The player authority check produced no rejection",
  );
  assert.equal(denied.ok, false);
  assert.equal((await status()).mmorpgEditing, false);
  record(
    "Player paths reject Edit and locked player authority cannot reacquire it",
    { pathGuard, denied },
  );
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(`Ground prop edit checks passed (${report.checks.length}).`);
} catch (error) {
  report.errors.push({ error: String(error), stack: error.stack });
  try {
    report.failureStatus = await status();
    await capture("props-error");
  } catch {}
  console.error(error);
  process.exitCode = 1;
} finally {
  await neutral();
  try {
    report.finalStatus = await status();
  } catch {}
  closing = true;
  await context.close().catch(() => {});
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "ground-props-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await browser.close();
}
async function readyAfterReload() {
  await until(
    (s) =>
      s.phase === "ready" && !!s.mmorpg && s.runtimeTables?.liveRecords === 9,
    "The saved nine-prop draft did not reopen",
    150_000,
  );
}
