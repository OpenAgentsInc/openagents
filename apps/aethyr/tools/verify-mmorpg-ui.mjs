/** Real source-style UI checks. Run through openagents browser run; captures stay private. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
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
  "Keep source artwork and captures outside Git.",
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
  "Verify the local game server.",
);
const suffix = Date.now().toString(36).slice(-6);
const report = {
  schema: "aethyrforge.mmorpg-ui-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  ok: false,
  checks: [],
  captures: [],
  clients: [],
  actions: [],
  receipts: [],
  images: [],
  errors: [],
  console: [],
  failed: [],
  limitations: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const clients = [],
  bodies = [];
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const angular = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
const speed = (s) => Math.abs(s.mmorpg.player.prediction?.velocity ?? 0);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const snapshot = (c) => c.page.evaluate(() => window.aethyrforge?.snapshot());
const status = async (c) => (await snapshot(c))?.status;
async function until(c, predicate, message, timeout = 15_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const s = await status(c);
    if (
      s?.phase === "error" ||
      ["error", "disconnected"].includes(s?.mmorpg?.connection)
    )
      throw Error(`${message}: ${s.message}; ${s.mmorpg?.message ?? ""}`);
    const result = await predicate(s);
    if (result) return result === true ? s : result;
    await sleep(80);
  }
  throw Error(`${message}: ${JSON.stringify(await status(c))}`);
}
async function capture(c, name) {
  const path = resolve(output, `${name}.png`);
  await c.page.screenshot({ path });
  report.captures.push({ client: c.label, path });
}
async function frame(c) {
  const handle = await c.page.locator("#preview-frame").elementHandle();
  const f = await handle.contentFrame();
  assert(f, "Preview frame is missing");
  return f;
}
async function focusWorld(c) {
  await c.page.bringToFront();
  const f = await frame(c);
  await f.locator("#world").focus();
  await f.waitForFunction(
    () =>
      document.hasFocus() &&
      document.activeElement === document.getElementById("world"),
  );
  assert.equal(
    await f.evaluate(() => !!document.pointerLockElement),
    false,
    "The following camera must not lock the mouse",
  );
}
async function neutral(c) {
  if (c.closed) return;
  for (const key of [
    "KeyW",
    "KeyA",
    "KeyS",
    "KeyD",
    "ShiftLeft",
    "ShiftRight",
    "Space",
    "Backspace",
  ])
    await c.page.keyboard.up(key).catch(() => {});
  await c.page
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
async function live(c) {
  let s = await status(c);
  if (!s.mmorpg.player.alive) {
    await c.page.bringToFront();
    await c.page
      .getByRole("button", { name: "Return to the world", exact: true })
      .click();
    s = await until(
      c,
      (v) => v?.mmorpg?.player?.alive && v,
      "The real Respawn button did not restore the player",
    );
    report.actions.push({
      client: c.label,
      action: "respawn through the visible UI",
      health: s.mmorpg.player.health,
      id: s.mmorpg.player.id,
    });
  }
  return s;
}
async function settled(c) {
  for (let attempt = 0; attempt < 3; attempt++) {
    await neutral(c);
    await live(c);
    const state = await until(
      c,
      (s) =>
        !s.mmorpg.player.alive ||
        (s.mmorpg.player.state === "idle" && speed(s) < 0.08)
          ? s
          : false,
      "The source controller did not settle",
      8500,
    );
    if (state.mmorpg.player.alive) return state;
  }
  throw Error(
    "Nearby combat prevented a stable idle control fixture after three real respawns.",
  );
}

async function makeClient(label, reducedMotion = "no-preference") {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    serviceWorkers: "block",
    reducedMotion,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(20_000);
  const c = {
    label,
    context,
    page,
    closed: false,
    images: new Map(),
    sockets: [],
  };
  clients.push(c);
  page.on("pageerror", (error) =>
    report.errors.push({ client: label, error: String(error) }),
  );
  page.on("console", (message) => {
    if (["error", "warning"].includes(message.type()))
      report.console.push({
        client: label,
        type: message.type(),
        text: message.text().slice(0, 4096),
      });
  });
  page.on("requestfailed", (request) => {
    if (!c.closed)
      report.failed.push({
        client: label,
        url: request.url(),
        error: request.failure()?.errorText,
      });
  });
  page.on("response", (response) => {
    if (response.status() >= 400 && !c.closed)
      report.failed.push({
        client: label,
        url: response.url(),
        status: response.status(),
      });
    if (
      /\/private-assets\/mmorpg-icon-[A-Za-z]+-[a-f0-9]{64}\.(?:png|jpg|jpeg)$/.test(
        new URL(response.url()).pathname,
      )
    )
      bodies.push(
        response
          .body()
          .then((bytes) => {
            c.images.set(new URL(response.url()).pathname, {
              status: response.status(),
              origin: new URL(response.url()).origin,
              sha256: createHash("sha256").update(bytes).digest("hex"),
              byteLength: bytes.length,
            });
          })
          .catch((error) => {
            if (!c.closed)
              report.failed.push({
                client: label,
                url: response.url(),
                error: String(error),
              });
          }),
      );
  });
  page.on("websocket", (socket) => {
    if (!new URL(socket.url()).pathname.endsWith("/ws")) return;
    const row = {
      url: socket.url(),
      sent: 0,
      received: 0,
      sentTypes: {},
      receivedTypes: {},
      closed: false,
    };
    c.sockets.push(row);
    const count = (event, direction) => {
      row[direction]++;
      try {
        const value = JSON.parse(String(event.payload));
        const key = String(value.type);
        const types = row[`${direction}Types`];
        types[key] = (types[key] ?? 0) + 1;
      } catch {}
    };
    socket.on("framesent", (event) => count(event, "sent"));
    socket.on("framereceived", (event) => count(event, "received"));
    socket.on("close", () => {
      row.closed = true;
    });
  });
  await page.goto(url.href);
  await until(
    c,
    (s) => s?.phase === "ready" && !!s.mmorpg,
    "The four-class world did not open",
    150_000,
  );
  return c;
}
async function join(c, name, selectedClass) {
  assert.equal(
    await c.page.locator("#mmo-class").inputValue(),
    "paladin",
    "Paladin must be the default guest class",
  );
  assert.equal(await c.page.locator("#mmo-name").inputValue(), "");
  await c.page.locator("#mmo-name").fill(name);
  if (selectedClass !== "paladin")
    await c.page.locator("#mmo-class").selectOption(selectedClass);
  await c.page
    .getByRole("button", { name: "Enter world", exact: true })
    .click();
  const s = await until(
    c,
    (v) => v?.mmorpg?.connection === "online" && v,
    "Guest login did not reach the authoritative world",
  );
  assert.equal(s.mmorpg.player.name, name);
  assert.equal(s.mmorpg.player.class, selectedClass);
  assert.equal(
    s.mmorpg.player.maxHealth,
    selectedClass === "paladin" ? 200 : 100,
  );
  await c.page
    .locator(".mmo-login")
    .waitFor({ state: "detached", timeout: 3000 });
  report.clients.push({
    client: c.label,
    playerId: s.mmorpg.player.id,
    name,
    class: selectedClass,
    initialHealth: s.mmorpg.player.health,
  });
  return s;
}
async function openPanel(c, name) {
  await live(c);
  await c.page.getByRole("button", { name, exact: true }).click();
  await c.page
    .getByRole("region", { name, exact: true })
    .waitFor({ state: "visible" });
}
async function changedInventory(c, from, expectedWeapon, mode) {
  await live(c);
  const before = await snapshot(c),
    version = before.status.mmorpg.player.inventoryVersion;
  const oldReceipts = new Set(before.receipts.map((r) => r.requestId));
  const sourceSlot = c.page.locator(`button.mmo-slot[data-slot="${from}"]`),
    weaponSlot = c.page.locator('button.mmo-slot[data-slot="24"]');
  if (mode === "drag") await sourceSlot.dragTo(weaponSlot);
  else {
    await sourceSlot.click();
    await weaponSlot.click();
  }
  const accepted = await until(
    c,
    (s) =>
      s.mmorpg.player.inventoryVersion > version &&
      s.mmorpg.player.weapon === expectedWeapon &&
      s,
    "The real equipment swap did not reach the server",
  );
  assert.equal(accepted.mmorpg.player.inventoryVersion, version + 1);
  await sourceSlot.waitFor({ state: "visible" });
  await until(
    c,
    async () => {
      const latest = await snapshot(c);
      return (
        latest.receipts.some((r) => !oldReceipts.has(r.requestId) && r.ok) &&
        !(await sourceSlot.isDisabled())
      );
    },
    "The equipment command did not receive a successful receipt",
  );
  const receipts = (await snapshot(c)).receipts.filter(
    (r) => !oldReceipts.has(r.requestId),
  );
  report.receipts.push(
    ...receipts.map((r) => ({
      client: c.label,
      operation: `equip ${expectedWeapon}`,
      ...r,
    })),
  );
  assert.ok(receipts.some((r) => r.ok));
  report.actions.push({
    client: c.label,
    action: `${mode} inventory slot ${from} to24`,
    versionBefore: version,
    versionAfter: accepted.mmorpg.player.inventoryVersion,
    weapon: accepted.mmorpg.player.weapon,
  });
}
async function sendChat(c, text) {
  await live(c);
  await focusWorld(c);
  await c.page.keyboard.press("Enter");
  await c.page.waitForFunction(() =>
    document.activeElement?.matches(".mmo-chat-input"),
  );
  await c.page.getByRole("textbox", { name: "Chat message" }).fill(text);
  await c.page.keyboard.press("Enter");
  await until(
    c,
    (s) =>
      s.mmorpg.events.some(
        (e) =>
          e.kind === "chat" &&
          e.actor === s.mmorpg.player.id &&
          e.text === text,
      ),
    "The typed chat did not return from the server",
  );
  await c.page.waitForFunction(
    () => document.querySelector(".mmo-chat-input")?.value === "",
  );
  await c.page.keyboard.press("Escape");
  await (await frame(c)).waitForFunction(() => document.hasFocus());
  report.actions.push({
    client: c.label,
    action: "typed chat through Enter bridge",
    text,
  });
}
async function keyboardControls(c) {
  await c.page.keyboard.press("Escape");
  await settled(c);
  await focusWorld(c);
  const before = await status(c);
  let walk, run;
  await c.page.keyboard.down("KeyW");
  try {
    await sleep(900);
    walk = await until(
      c,
      (s) => s.mmorpg.player.state === "walk" && s,
      "W did not select the source Walk state",
    );
    assert.ok(walk.mmorpg.player.alive);
    assert.ok(
      horizontal(walk.mmorpg.player.position, before.mmorpg.player.position) >
        0.5,
      "W did not move the actual player",
    );
    await c.page.keyboard.down("ShiftLeft");
    await sleep(800);
    run = await until(
      c,
      (s) => s.mmorpg.player.state === "run" && s,
      "Shift+W did not select Run",
    );
    assert.ok(
      speed(run) > speed(walk) * 1.55,
      `Shift did not accelerate: ${speed(walk)} to${speed(run)}`,
    );
  } finally {
    await neutral(c);
  }
  record("W and Shift use real source walking and running", {
    walk: { position: walk.mmorpg.player.position, speed: speed(walk) },
    run: { position: run.mmorpg.player.position, speed: speed(run) },
  });
  await settled(c);
  await focusWorld(c);
  const turns = [];
  for (const key of ["KeyD", "KeyA"]) {
    const start = await status(c);
    await c.page.keyboard.down(key);
    let end;
    try {
      await sleep(700);
      end = await status(c);
    } finally {
      await c.page.keyboard.up(key);
    }
    assert.ok(end.mmorpg.player.alive, "The player died during the turn check");
    const delta = angular(end.mmorpg.player.yaw, start.mmorpg.player.yaw);
    assert.ok(
      Math.abs(delta) > 0.5 && Math.abs(delta) < 1.6,
      `${key} did not turn about63degrees: ${delta}`,
    );
    assert.ok(
      horizontal(end.mmorpg.player.position, start.mmorpg.player.position) <
        0.3,
      "A/D strafed instead of turning",
    );
    turns.push({ key, delta });
  }
  assert.ok(turns[0].delta * turns[1].delta < 0);
  const start = await status(c);
  await c.page.keyboard.down("KeyS");
  let back;
  try {
    await sleep(800);
    back = await status(c);
  } finally {
    await c.page.keyboard.up("KeyS");
  }
  assert.equal(back.mmorpg.player.state, "walk");
  const displacement = [
    back.mmorpg.player.position[0] - start.mmorpg.player.position[0],
    back.mmorpg.player.position[2] - start.mmorpg.player.position[2],
  ];
  assert.ok(
    displacement[0] * Math.sin(start.mmorpg.player.yaw) -
      displacement[1] * Math.cos(start.mmorpg.player.yaw) <
      -0.3,
    "S did not move backward along the source heading",
  );
  record("A/D turn and S reverses without strafing", {
    turns,
    backwardPosition: back.mmorpg.player.position,
  });
  for (const [key, expected, maxMs] of [
    ["Space", "attack", 3500],
    ["Backspace", "dance", 7000],
  ]) {
    await settled(c);
    await focusWorld(c);
    const before = await status(c);
    await c.page.keyboard.down(key);
    let active;
    try {
      active = await until(
        c,
        (s) => s.mmorpg.player.alive && s.mmorpg.player.state === expected && s,
        `${key} did not enter ${expected}`,
        3500,
      );
    } finally {
      await c.page.keyboard.up(key);
    }
    if (expected === "attack")
      await until(
        c,
        (s) => s.mmorpg.player.attacks > before.mmorpg.player.attacks,
        "Space did not reach the authoritative attack counter",
      );
    const finished = await until(
      c,
      (s) =>
        !s.mmorpg.player.alive || s.mmorpg.player.state === "idle" ? s : false,
      `${expected} did not finish`,
      maxMs,
    );
    record(`${key} enters the authored ${expected} action`, {
      state: active.mmorpg.player.state,
      animationTime: active.mmorpg.player.actionTime,
      completed: finished.mmorpg.player.alive,
      interruptedByDeath: !finished.mmorpg.player.alive,
    });
    if (!finished.mmorpg.player.alive) await live(c);
  }
}
try {
  const a = await makeClient("Paladin");
  await capture(a, "mmorpg-ui-login");
  const joined = await join(a, `UI Paladin ${suffix}`, "paladin");
  assert.equal(joined.mmorpg.player.health, 200);
  await a.page.getByRole("progressbar", { name: "Health" }).waitFor();
  assert.equal(
    await a.page
      .getByRole("progressbar", { name: "Health" })
      .getAttribute("aria-valuemax"),
    "200",
  );
  assert.equal(await a.page.locator(".ground-zone-title").count(), 0);
  assert.equal(
    await a.page.getByRole("button", { name: "Effects", exact: true }).count(),
    0,
  );
  record("Guest login defaults to Paladin and opens the source health frame", {
    id: joined.mmorpg.player.id,
    health: joined.mmorpg.player.health,
  });
  await openPanel(a, "Statistics");
  const stats = await a.page
    .locator(".mmo-stats")
    .evaluate((element) =>
      Object.fromEntries(
        [...element.children].map((row) => [
          row.querySelector(".mmo-stat-label").textContent,
          row.querySelector("dd").textContent,
        ]),
      ),
    );
  assert.deepEqual(stats, {
    Strength: "50",
    Wisdomness: "5",
    Benchpress: "20",
    Curl: "100",
    XP: "0",
  });
  const sourceTooltips = {
    strength:
      "How strong you are, affects how much damage you do. So blah blah if you're doing stuff then its stronger or whatever, the damage is up. This is text to show the tooltip.",
    wisdomness:
      "Wisdom is the guage of something to do with wisdom in the game because wisdom is important and wisdom is wise to wisdoming.",
    benchpress: "Gotta have a good bench to be a warrior or something.",
    curl: "The ultimate expression of strength, this affects literally everything in your life.",
    experience:
      "How much xp you've accumulated by killing things for xp. Get enough and you'll gain a level or something.",
  };
  for (const [key, text] of Object.entries(sourceTooltips)) {
    const label = a.page.locator(`.mmo-stats-tooltip[data-stat="${key}"]`),
      tooltip = label.getByRole("tooltip");
    await label.hover();
    await tooltip.waitFor({ state: "visible" });
    assert.equal(await tooltip.textContent(), text);
    assert.ok(
      (await tooltip.boundingBox()).x < (await label.boundingBox()).x,
      "Source statistics tooltips should open to the left",
    );
    if (key === "strength") await capture(a, "mmorpg-ui-statistics-tooltip");
    await a.page
      .getByRole("heading", { name: "Statistics", exact: true })
      .hover();
    await tooltip.waitFor({ state: "hidden" });
  }
  await capture(a, "mmorpg-ui-statistics");
  record("Statistics preserve the source character values", {
    stats,
    sourceHoverTooltips: 5,
  });
  await openPanel(a, "Inventory");
  assert.equal(await a.page.locator("button.mmo-slot").count(), 32);
  await a.page
    .locator(".mmo-slot img")
    .evaluateAll((images) =>
      Promise.all(images.map((image) => image.decode())),
    );
  await Promise.all(bodies);
  const s = await status(a),
    keys = Object.keys(s.mmorpg.icons.icons);
  assert.equal(keys.length, 9);
  for (const [key, descriptor] of Object.entries(s.mmorpg.icons.icons)) {
    const observed = a.images.get(descriptor.url);
    assert.ok(observed, `The real UI did not load ${key}`);
    assert.equal(observed.status, 200);
    assert.equal(observed.origin, url.origin);
    assert.equal(observed.sha256, descriptor.sha256);
    assert.equal(observed.byteLength, descriptor.byteLength);
    report.images.push({ key, ...observed });
  }
  record(
    "All nine source UI images load on the editor origin with exact hashes",
  );
  await changedInventory(a, 0, "weapon.axe1", "click");
  await changedInventory(a, 1, "weapon.hammer1", "click");
  await changedInventory(a, 0, "weapon.sword1", "drag");
  await capture(a, "mmorpg-ui-inventory");
  record(
    "24 backpack slots and eight equipment slots perform real click and drag swaps",
  );
  await openPanel(a, "Quest journal");
  assert.equal(
    await a.page.locator(".mmo-empty-journal").textContent(),
    "No quests.",
  );
  await a.page
    .getByRole("button", { name: "Quest journal", exact: true })
    .click();
  assert.equal(await a.page.locator(".mmo-panel").count(), 0);
  record(
    "The source's empty quest journal opens and closes without invented quests",
  );
  await live(a);
  await settled(a);
  await focusWorld(a);
  await a.page.keyboard.down("KeyW");
  await sleep(250);
  await a.page.keyboard.press("Enter");
  await a.page.waitForFunction(() =>
    document.activeElement?.matches(".mmo-chat-input"),
  );
  await sleep(900);
  const chatStopped = await status(a);
  assert.ok(
    speed(chatStopped) < 0.12,
    `Movement kept running while chat had focus: ${speed(chatStopped)}`,
  );
  await a.page.keyboard.up("KeyW");
  await a.page.keyboard.press("Escape");
  await (await frame(a)).waitForFunction(() => document.hasFocus());
  record(
    "Enter from the iframe opens chat, releases held movement, and Escape returns to the world",
  );

  const b = await makeClient("Sorceror", "reduce");
  const second = await join(b, `UI Sorceror ${suffix}`, "sorceror");
  await live(a);
  const aId = (await status(a)).mmorpg.player.id,
    bId = second.mmorpg.player.id;
  assert.notEqual(aId, bId);
  await until(
    a,
    (v) =>
      v.mmorpg.actors.some(
        (actor) => actor.id === bId && actor.class === "sorceror",
      ),
    "The first independent context did not see the second player",
  );
  await until(
    b,
    (v) =>
      v.mmorpg.actors.some(
        (actor) => actor.id === aId && actor.class === "paladin",
      ),
    "The second independent context did not see the first player",
  );
  const text = `Two guests ${suffix}`;
  await sendChat(b, text);
  await until(
    a,
    (v) =>
      v.mmorpg.events.some(
        (e) => e.kind === "chat" && e.actor === bId && e.text === text,
      ),
    "Nearby chat did not reach the other independent context",
  );
  await a.page
    .locator(".mmo-chat-line")
    .filter({ hasText: text })
    .waitFor({ state: "visible" });
  await a.page.bringToFront();
  await capture(a, "mmorpg-ui-two-players");
  const named = await until(
    a,
    (v) => v.runtimeMetrics?.mmorpgNameLabels > 0 && v,
    "The current-frame GPU did not submit source name labels",
  );
  const metrics = named.runtimeMetrics;
  const names = {
    labels: metrics.mmorpgNameLabels,
    triangles: metrics.mmorpgNameTriangles,
    drawCalls: metrics.mmorpgNameDrawCalls,
    textureBytes: metrics.mmorpgNameTextureBytes,
    textureLayers: metrics.mmorpgNameTextureLayers,
    uploads: metrics.mmorpgNameUploads,
  };
  for (const [key, value] of Object.entries(names))
    assert.ok(
      Number.isSafeInteger(value) && value >= 0,
      `Invalid GPU name metric ${key}: ${value}`,
    );
  assert.ok(names.labels > 0 && names.labels <= 64);
  assert.equal(names.triangles, names.labels * 2);
  assert.equal(names.drawCalls, 1);
  assert.ok(names.textureBytes > 0 && names.textureBytes <= 12 * 1024 * 1024);
  assert.ok(names.textureLayers > 0 && names.textureLayers <= 64);
  assert.ok(names.uploads > 0);
  assert.equal(
    await a.page.locator("canvas.mmo-world-name").count(),
    0,
    "Legacy DOM labels duplicate GPU name sprites",
  );

  record(
    "Independent Paladin and Sorceror contexts see each other and exchange nearby chat",
    { aId, bId, names },
  );
  b.closed = true;
  await b.context.close();
  await until(
    a,
    (v) => !v.mmorpg.actors.some((actor) => actor.id === bId),
    "Closing the second context did not remove its player",
    15_000,
  );
  record("Closing the second context despawns its player on the server");
  await keyboardControls(a);
  await capture(a, "mmorpg-ui-final");
  await Promise.all(bodies);
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  for (const c of clients)
    if (!c.sockets.length)
      report.limitations.push(
        `${c.label}: this CDP build did not expose worker WebSocket frames; real mutual presence/chat were checked through server snapshots.`,
      );
  report.ok = true;
  console.log(
    "MMO real UI checks passed",
    JSON.stringify({
      checks: report.checks.length,
      images: report.images.length,
      clients: report.clients.length,
    }),
  );
} catch (error) {
  report.errors.push({ error: String(error), stack: error.stack });
  for (const c of clients)
    if (!c.closed) {
      try {
        report[`${c.label}FailureStatus`] = await status(c);
        await capture(c, `mmorpg-ui-error-${c.label.toLowerCase()}`);
      } catch {}
    }
  console.error(error);
  process.exitCode = 1;
} finally {
  for (const c of clients)
    if (!c.closed) {
      await neutral(c);
      try {
        report[`${c.label}Final`] = await status(c);
      } catch {}
      c.closed = true;
      await c.context.close().catch(() => {});
    }
  report.webSockets = clients.map((c) => ({
    client: c.label,
    sockets: c.sockets,
  }));
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "mmorpg-ui-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await browser.close();
}
