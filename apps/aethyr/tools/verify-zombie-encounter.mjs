/** Run through openagents browser run; keep combat captures outside Git. */
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
  "Keep combat captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["localhost", "127.0.0.1"].includes(url.hostname) &&
    !url.username &&
    !url.password,
  "Verify a local Open Ground server.",
);
const minimumFps = Number(process.env.AF_MIN_FPS || 54);
assert(Number.isFinite(minimumFps) && minimumFps > 0 && minimumFps <= 240);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const sourcePaladinSha =
  "8d28a9ef055ac10ed8afbe832888cb042195ed2aaa02a20458297cfbb5476d40";
const sourceZombieSha =
  "9f68487474a736153042c89786120bd4255556e64681ee740a98dbe0c7e1dba9";
const sourceZombieMaps = {
  texture: "d21a3f055e72b0def0057ae11adb0bba180c4b819de008c12da25ee0ea827625",
  normalTexture:
    "3eec4982e46b7c33074619f7a74ebafe7bb840c8d5d649513cb5d387ea4b1a80",
  emissiveTexture:
    "0a1ecb77ed0a1a754363f108ccba477ea48a54b576ece2de58d79676ccb553eb",
};
const report = {
  schema: "aethyrforge.zombie-encounter-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  minimumSubmissionFps: minimumFps,
  ok: false,
  checks: [],
  captures: [],
  receipts: [],
  scenes: [],
  characters: [],
  zombies: [],
  grassSettings: [],
  assets: [],
  errors: [],
  console: [],
  failed: [],
  responseFailures: [],
  limitations: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
await context.addInitScript(() => {
  window.__zombieReady = [];
  window.__zombieGraphics = [];
  window.__zombieInput = [];
  for (const type of ["keydown", "keyup", "pointerlockchange"])
    (type === "pointerlockchange" ? document : window).addEventListener(
      type,
      (event) => {
        if (window.__zombieInput.length < 256)
          window.__zombieInput.push({
            type,
            code: event.code,
            at: performance.now(),
            locked: !!document.pointerLockElement,
          });
      },
    );
  window.addEventListener("message", (event) => {
    const frame = document.getElementById("preview-frame");
    if (
      !frame?.contentWindow ||
      event.source !== frame.contentWindow ||
      event.origin !== new URL(frame.src).origin ||
      event.data?.protocol !== "aethyrforge.preview.v1"
    )
      return;
    if (event.data.type === "ready")
      window.__zombieReady.push({ hasSourceSky: event.data.hasSourceSky });
    const message = event.data?.status?.message || event.data?.message;
    if (
      typeof message === "string" &&
      /^Graphics(?::| device stopped:)/.test(message)
    )
      window.__zombieGraphics.push(message.slice(0, 4096));
  });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
const collect = (promise) =>
  bodies.push(
    promise.catch((error) => {
      if (!closing) report.responseFailures.push(String(error));
    }),
  );
let closing = false;
const bodies = [];
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 2048),
    });
});
page.on("requestfailed", (request) => {
  if (!closing)
    report.failed.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
});
page.on("response", (response) => {
  const path = new URL(response.url()).pathname;
  if (response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
  if (/\/private-assets\/scene-ground(?:-night)?\.json$/.test(path))
    collect(
      response
        .json()
        .then((scene) => report.scenes.push({ url: response.url(), scene })),
    );
  if (/\/character-quick-mmorpg-paladin-[a-f0-9]{64}\.json$/.test(path))
    collect(
      response.body().then((bytes) =>
        report.characters.push({
          url: response.url(),
          sha256: sha256(bytes),
          byteLength: bytes.length,
          manifest: JSON.parse(bytes),
        }),
      ),
    );
  if (/\/character-quick-mmorpg-zombie-[a-f0-9]{64}\.json$/.test(path))
    collect(
      response.body().then((bytes) =>
        report.zombies.push({
          url: response.url(),
          sha256: sha256(bytes),
          byteLength: bytes.length,
          manifest: JSON.parse(bytes),
        }),
      ),
    );
  if (/\/grass-[a-f0-9]{64}\.json$/.test(path))
    collect(
      response
        .json()
        .then((manifest) =>
          report.grassSettings.push({ url: response.url(), manifest }),
        ),
    );
  if (
    /\/character-[a-f0-9]{64}\.(?:png|vertices\.bin|indices\.bin)$/.test(path)
  )
    collect(
      response.body().then((bytes) =>
        report.assets.push({
          url: response.url(),
          sha256: sha256(bytes),
          byteLength: bytes.length,
          pngExtent: path.endsWith(".png")
            ? [bytes.readUInt32BE(16), bytes.readUInt32BE(20)]
            : undefined,
        }),
      ),
    );
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
let readyEstablished = false;
const status = async () => {
  const value = (await snapshot()).status;
  if (readyEstablished && (value.phase !== "ready" || !value.character)) {
    report.statusDuringFailure = value;
    throw Error(
      `The ready world restarted or failed during input: ${JSON.stringify(value)}`,
    );
  }
  return value;
};
const feet = (s) => s.character.position;
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
const speed = (s) =>
  Math.hypot(s.character.velocity[0], s.character.velocity[2]);
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
const percentile = (values, f) =>
  [...values].sort((a, b) => a - b)[
    Math.min(values.length - 1, Math.floor(values.length * f))
  ] ?? null;
const submittedFrames = (s) => s.runtimeMetrics?.submittedFrames ?? s.frame;
const heading = (s) =>
  Math.atan2(
    s.view.target[0] - s.view.eye[0],
    s.view.target[2] - s.view.eye[2],
  );
const angularDelta = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
function healthy(s) {
  assert.equal(s.phase, "ready", s.message);
  assert.equal(s.character?.controller, "quick-mmorpg-v1");
  assert.equal(s.character.modelLoaded, true);
  assert.equal(s.character.perspective, "third");
  assert(s.character.grounded, "The Paladin left the continuous ground");
  assert(
    Math.abs(feet(s)[1]) < 0.02,
    "The Paladin's feet left the zero-height plane",
  );
  assert.notEqual(
    s.gameplay?.enabled,
    true,
    "Open Ground enabled spell gameplay",
  );
  assert.equal(s.game, undefined, "Open Ground installed a quest");
  assert.equal(s.instances, 1);
  assert.equal(s.triangles, 2);
  assert(
    s.combat && s.combat.total === 8 && s.combat.actors.length === 8,
    "Eight combat actors are required",
  );
  assert.equal(s.combat.player.maxHealth, 200);
  assert.equal(s.combat.alive + s.combat.killed, 8);
}
function grass(s) {
  const g = s.groundGrass;
  assert(
    g && g.nearBlades > 0 && g.farBlades > 0,
    "Both submitted grass detail levels are required",
  );
  for (const key of [
    "nearPatches",
    "farPatches",
    "nearBlades",
    "farBlades",
    "blades",
    "triangles",
    "drawCalls",
    "accountedBytes",
  ])
    assert(Number.isSafeInteger(g[key]) && g[key] >= 0, `Invalid grass ${key}`);
  assert.equal(g.drawCalls, 2);
  assert.equal(g.blades, g.nearBlades + g.farBlades);
  assert.equal(g.triangles, g.nearBlades * 12 + g.farBlades * 6);
  assert(g.accountedBytes > 0 && Number.isFinite(g.windTime));
  return g;
}
async function until(test, message, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(40);
  } while (Date.now() < deadline);
  throw Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function waitFrames(count = 3) {
  const before = (await status()).frame;
  return until(async () => {
    const s = await status();
    assert.notEqual(s.phase, "error", s.message);
    return s.frame >= before + count && s;
  }, "Rendering stopped");
}
async function capture(name) {
  const path = resolve(output, `${name}.png`),
    pixels = await page.screenshot({ path });
  report.captures.push(path);
  return pixels;
}
async function focusWorld() {
  const frame = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  await frame.locator("#world").click({ position: { x: 640, y: 300 } });
  await frame.waitForFunction(
    () =>
      document.hasFocus() &&
      document.pointerLockElement === document.getElementById("world"),
  );
  return frame;
}
async function releaseInput(unlock = false) {
  for (const key of [
    "KeyW",
    "KeyA",
    "KeyS",
    "KeyD",
    "ShiftLeft",
    "Space",
    "Backspace",
  ])
    await page.keyboard.up(key);
  if (unlock) {
    await page.keyboard.press("Escape");
    const frame = await (
      await page.locator("#preview-frame").elementHandle()
    )?.contentFrame();
    await frame?.evaluate(() => document.exitPointerLock());
  }
}
async function settled() {
  await releaseInput();
  return until(async () => {
    const s = await status();
    healthy(s);
    return s.character.animation === "idle" && speed(s) < 0.005 && s;
  }, "The source deceleration did not settle");
}
async function receiptAfter(action, expected = true) {
  const prior = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const requestId = await action();
  const result = await until(
    async () =>
      (await snapshot()).receipts.find((r) =>
        typeof requestId === "string"
          ? r.requestId === requestId
          : !prior.has(r.requestId),
      ),
    "No command receipt arrived",
  );
  report.receipts.push(result);
  assert.equal(result.ok, expected, result.message);
  return result;
}

function rendering(s) {
  const r = s.combatRendering;
  assert(r?.modelLoaded, "The zombie renderer did not load its actual model");
  for (const field of [
    "submittedActors",
    "submittedTriangles",
    "submittedHealthBars",
    "submittedHealthBarTriangles",
    "geometryBytes",
  ])
    assert(
      Number.isSafeInteger(r[field]) && r[field] >= 0,
      `Invalid combat rendering ${field}`,
    );
  assert.equal(r.submittedActors, 8);
  assert.equal(r.submittedTriangles, 8 * 12486);
  assert(
    r.submittedHealthBars > 0 && r.submittedHealthBars <= s.combat.alive,
    "Only living, visible zombie anchors should submit health bars",
  );
  assert.equal(r.submittedHealthBarTriangles, r.submittedHealthBars * 4);
  assert(
    r.geometryBytes > 64 * 1024 * 1024,
    "The shared source model/maps are missing from accounting",
  );
  return r;
}

function checkSourceZombie() {
  const entry = report.zombies[0];
  assert(entry, "The original ZombieGuy manifest was not loaded");
  assert(
    entry.url.includes(entry.sha256),
    "The zombie manifest filename does not match its hash",
  );
  const m = entry.manifest;
  assert.equal(m.name, "npc/quick-mmorpg-zombie");
  assert.equal(m.animationProfile, "quick-mmorpg-zombie-v1");
  assert.equal(m.metadata.sourceSha256, sourceZombieSha);
  assert.equal(
    m.metadata.sourceCommit,
    "547884332ca650abe96264f7230702d36481b9bc",
  );
  assert.equal(m.metadata.originalAppScale, 4);
  assert.equal(m.metadata.worldScale, 1 / 6);
  assert.equal(m.metadata.effectiveScale, 4 / 6);
  assert.deepEqual(m.metadata.includedMeshes, ["skeletonZombie"]);
  assert.deepEqual(m.metadata.sourceStats, {
    health: 20,
    maxHealth: 50,
    strength: 25,
  });
  assert.equal(m.metadata.totalAnimationKeys, 80925);
  assert.equal(m.joints.length, 66);
  assert.equal(m.geometry.vertexCount, 7417);
  assert.equal(m.geometry.indexCount, 12486 * 3);
  assert.deepEqual(
    m.clips.map((c) => c.name),
    ["idle", "walk", "attack", "death"],
  );
  for (const clip of m.clips)
    assert.equal(clip.loop, ["idle", "walk"].includes(clip.name));
  assert.equal(m.materials.length, 1);
  const material = m.materials[0];
  assert.equal(material.metallic, 0.5);
  assert.equal(material.roughness, 0.5527864098548889);
  assert.deepEqual(material.emissiveColor, [1, 1, 1]);
  assert.equal(material.doubleSided, true);
  for (const [field, hash] of Object.entries(sourceZombieMaps)) {
    const descriptor = material[field];
    assert(descriptor, `Missing original zombie ${field}`);
    assert.equal(descriptor.sha256, hash);
    assert.deepEqual([descriptor.width, descriptor.height], [2048, 2048]);
    const actual = report.assets.find(
      (a) => a.url === new URL(descriptor.url, entry.url).href,
    );
    assert(actual, `The rendered path did not request ${field}`);
    assert.equal(actual.sha256, hash);
    assert.equal(actual.byteLength, descriptor.byteLength);
    assert.deepEqual(actual.pngExtent, [2048, 2048]);
  }
  for (const descriptor of [m.geometry.vertices, m.geometry.indices]) {
    const actual = report.assets.find(
      (a) => a.url === new URL(descriptor.url, entry.url).href,
    );
    assert(actual, "The rendered zombie geometry was not requested");
    assert.equal(actual.sha256, descriptor.sha256);
    assert.equal(actual.byteLength, descriptor.byteLength);
  }
  const paladin = report.characters[0];
  assert(
    paladin && paladin.manifest.metadata.sourceSha256 === sourcePaladinSha,
  );
  return {
    url: entry.url,
    sha256: entry.sha256,
    geometry: m.geometry,
    clips: m.clips.map(({ name, duration, loop }) => ({
      name,
      duration,
      loop,
    })),
    material,
    healthBarHeight: m.metadata.healthBarHeight,
  };
}

let spawn, worldRevision;
async function restart(name, { releasePointer = true } = {}) {
  await releaseInput(releasePointer);
  if (!releasePointer) {
    const preview = await (
      await page.locator("#preview-frame").elementHandle()
    ).contentFrame();
    assert(
      await preview.evaluate(() => document.pointerLockElement === null),
      "Restart still requires a manual mouse unlock after death",
    );
  }
  const receipt = await receiptAfter(() =>
    page.getByRole("button", { name: "Restart", exact: true }).click(),
  );
  assert.match(receipt.message, /restarted/i);
  const s = await until(async () => {
    const value = await status();
    return (
      value.character?.grounded &&
      Math.abs(feet(value)[1]) < 0.02 &&
      value.combat.player.health === 200 &&
      value.combat.alive === 8 &&
      value.combat.killed === 0 &&
      value.character.animation === "idle" &&
      value
    );
  }, "Restart did not restore the encounter");
  healthy(s);
  assert.equal(s.combat.player.alive, true);
  assert.equal(s.combat.player.attacks, 0);
  assert.equal(s.combat.player.hits, 0);
  assert(
    s.combat.actors.every(
      (a) => a.alive && a.health === 20 && a.maxHealth === 50,
    ),
  );
  if (!spawn) spawn = [...feet(s)];
  assert(
    distance(feet(s), spawn) < 0.003,
    "Restart changed the original spawn",
  );
  if (worldRevision === undefined) worldRevision = s.revision;
  assert.equal(s.revision, worldRevision, "Restart edited the world document");
  await waitFrames(3);
  const rendered = await status();
  rendering(rendered);
  if (name) await capture(name);
  return {
    receipt,
    character: s.character,
    combat: s.combat,
    rendering: rendered.combatRendering,
    revision: s.revision,
  };
}

async function pursuitAndDamage() {
  await restart();
  await focusWorld();
  const before = await status(),
    start = performance.now();
  await page.waitForTimeout(1000);
  const after = await status(),
    seconds = (performance.now() - start) / 1000;
  healthy(after);
  grass(after);
  rendering(after);
  const motions = before.combat.actors.map((actor) => {
    const next = after.combat.actors.find((a) => a.id === actor.id);
    assert(next);
    const moved = horizontal(actor.position, next.position);
    const oldDistance = horizontal(actor.position, feet(before));
    const newDistance = horizontal(next.position, feet(after));
    assert(
      moved > 0.9 && moved / seconds < 2.15,
      `Zombie ${actor.id} did not pursue at the source walk speed: ${moved / seconds}`,
    );
    assert(
      newDistance < oldDistance - 0.8,
      `Zombie ${actor.id} moved away from the player`,
    );
    assert.equal(next.state, "walk");
    return {
      id: actor.id,
      moved,
      metresPerSecond: moved / seconds,
      oldDistance,
      newDistance,
    };
  });
  assert.equal(
    after.combat.player.health,
    200,
    "Zombies damaged the player before reaching attack range",
  );
  await capture("zombies-pursuing");
  const windup = await until(async () => {
    const s = await status();
    return (
      s.combat.actors.some((a) => a.state === "attack" && a.actionTime < 0.4) &&
      s
    );
  }, "Zombies never began their attack windup");
  const initialHealth = windup.combat.player.health,
    samples = [];
  assert.equal(
    initialHealth,
    200,
    "Damage arrived before the first visible windup",
  );
  const damaged = await until(
    async () => {
      const s = await status();
      healthy(s);
      samples.push({
        frame: s.frame,
        health: s.combat.player.health,
        actors: s.combat.actors.map((a) => ({
          id: a.id,
          state: a.state,
          actionTime: a.actionTime,
        })),
      });
      return s.combat.player.health < initialHealth && s;
    },
    "Attack windups never damaged the player",
    6000,
  );
  assert((200 - damaged.combat.player.health) % 5 === 0);
  assert(
    samples.some((s) =>
      s.actors.some((a) => a.state === "attack" && a.actionTime >= 0.75),
    ),
    "Damage did not follow the visible one-second windup",
  );
  await capture("zombies-damage-player");
  const hud = await page
    .getByRole("progressbar", { name: "Health", exact: true })
    .getAttribute("aria-valuenow");
  assert(Number(hud) < 200, "The health HUD did not show damage");
  return {
    seconds,
    motions,
    windup: windup.combat,
    damaged: damaged.combat,
    hud,
    samples,
  };
}

async function swordKill() {
  await releaseInput();
  await focusWorld();
  const target = await until(
    async () => {
      const s = await status();
      const direction = [
        s.view.target[0] - s.view.eye[0],
        s.view.target[2] - s.view.eye[2],
      ];
      const length = Math.hypot(...direction);
      const front = s.combat.actors
        .filter((a) => a.alive)
        .map((a) => {
          const dx = a.position[0] - feet(s)[0],
            dz = a.position[2] - feet(s)[2],
            range = Math.hypot(dx, dz);
          return {
            actor: a,
            range,
            facing: (dx * direction[0] + dz * direction[1]) / (range * length),
          };
        })
        .filter((a) => a.range < 1.667 && a.facing > 0.97)
        .sort((a, b) => a.range - b.range)[0];
      return (
        s.character.animation === "idle" && front && { status: s, ...front }
      );
    },
    "No nearby living zombie entered the sword's forward range",
    6000,
  );
  await page.keyboard.down("Space");
  let attack;
  try {
    attack = await until(async () => {
      const s = await status();
      return (
        s.character.animation === "attack" &&
        s.combat.player.attacks > target.status.combat.player.attacks &&
        s
      );
    }, "Real Space input did not start the sword attack");
  } finally {
    await page.keyboard.up("Space");
  }
  assert.equal(attack.combat.player.state, "attack");
  assert.equal(attack.character.grounded, true);
  const killed = await until(
    async () => {
      const s = await status(),
        actor = s.combat.actors.find((a) => a.id === target.actor.id);
      return (
        actor &&
        !actor.alive &&
        s.combat.killed > target.status.combat.killed &&
        s
      );
    },
    "The sword did not kill its forward zombie",
    2500,
  );
  assert(killed.combat.player.hits > target.status.combat.player.hits);
  const dead = killed.combat.actors.find((a) => a.id === target.actor.id);
  assert.equal(dead.health, 0);
  assert.equal(dead.state, "death");
  await waitFrames(3);
  const rendered = await status();
  rendering(rendered);
  assert(
    rendered.combatRendering.submittedHealthBars <
      target.status.combatRendering.submittedHealthBars,
    "The forward zombie's health bar remained after its death",
  );
  await capture("zombie-sword-death");
  const held = await until(
    async () => {
      const s = await status(),
        actor = s.combat.actors.find((a) => a.id === dead.id);
      return actor.actionTime > 3.05 && s;
    },
    "The zombie death clip never reached its held pose",
    5000,
  );
  assert.equal(held.combat.actors.find((a) => a.id === dead.id).state, "death");
  await capture("zombie-death-held");
  return {
    target: { id: dead.id, range: target.range, facing: target.facing },
    attack: attack.combat,
    killed: killed.combat,
    rendering: rendered.combatRendering,
    held: held.combat,
  };
}

async function playerDeath() {
  await restart();
  await focusWorld();
  const before = await status(),
    start = performance.now(),
    samples = [];
  const dead = await until(
    async () => {
      const s = await status();
      healthy(s);
      samples.push({
        frame: s.frame,
        health: s.combat.player.health,
        state: s.combat.player.state,
        character: s.character.animation,
      });
      assert(
        horizontal(feet(s), feet(before)) < 0.003,
        "An idle player was displaced",
      );
      return !s.combat.player.alive && s;
    },
    "Eight zombie attackers did not defeat the idle player within 30 seconds",
    30_000,
  );
  assert.equal(dead.combat.player.health, 0);
  assert.equal(dead.combat.player.state, "death");
  await until(
    async () => (await status()).character.animation === "death",
    "The Paladin did not play Death",
  );
  const preview = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  await until(
    async () => preview.evaluate(() => document.pointerLockElement === null),
    "Death did not release pointer lock for the Restart button",
  );
  await page.waitForTimeout(600);
  const locked = await status();
  await page.keyboard.down("KeyW");
  await page.keyboard.down("KeyD");
  let attempted;
  try {
    await page.waitForTimeout(700);
    attempted = await status();
  } finally {
    await page.keyboard.up("KeyW");
    await page.keyboard.up("KeyD");
  }
  assert(
    distance(feet(attempted), feet(locked)) < 0.003,
    "The dead player could move",
  );
  assert(
    distance(attempted.camera, locked.camera) < 0.03,
    "The dead player could turn the follow camera",
  );
  assert.equal(attempted.character.animation, "death");
  await page.getByText("You fell. Try again.", { exact: true }).waitFor();
  await capture("paladin-defeated");
  const seconds = (performance.now() - start) / 1000;
  return {
    seconds,
    samples,
    dead: dead.combat,
    locked: locked.character,
    attempted: attempted.character,
    pointerLockReleased: await preview.evaluate(
      () => document.pointerLockElement === null,
    ),
    restart: await restart("combat-restarted-after-death", {
      releasePointer: false,
    }),
  };
}

async function frameSample() {
  await restart();
  await focusWorld();
  const before = await status(),
    start = performance.now(),
    samples = [];
  let after;
  for (let i = 0; i < 60; i++) {
    await page.waitForTimeout(250);
    const s = await status();
    healthy(s);
    rendering(s);
    assert.equal(s.combat.alive, 8, "The performance fixture lost a zombie");
    assert(
      s.combat.player.alive,
      "The 15-second performance fixture ran beyond player death",
    );
    samples.push({
      atMs: performance.now() - start,
      frame: s.frame,
      submittedFrames: submittedFrames(s),
      fps: s.fps,
      health: s.combat.player.health,
      activeZombies: s.combat.actors.filter(
        (a) => a.state === "walk" || a.state === "attack",
      ).length,
      rendering: s.combatRendering,
      grass: grass(s),
    });
    after = s;
  }
  const seconds = (performance.now() - start) / 1000,
    frames = submittedFrames(after) - submittedFrames(before);
  const result = {
    seconds,
    submittedFrames: frames,
    submissionFps: frames / seconds,
    hudFpsMedian: percentile(
      samples.map((s) => s.fps),
      0.5,
    ),
    hudFpsMinimum: Math.min(...samples.map((s) => s.fps)),
    minimumActiveZombies: Math.min(...samples.map((s) => s.activeZombies)),
    samples,
  };
  report.frameSample = result;
  assert(
    result.submissionFps >= minimumFps,
    `Eight-zombie grass cadence ${result.submissionFps.toFixed(2)} fell below ${minimumFps}`,
  );
  assert(
    result.minimumActiveZombies === 8,
    "The performance fixture did not keep all eight zombies active",
  );
  await capture("zombies-grass-performance");
  return result;
}

try {
  report.browser = await browser.version();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge);
  const initial = await until(
    async () => {
      const s = await status();
      assert.notEqual(s.phase, "error", s.message);
      return (
        s.phase === "ready" &&
        s.frame > 3 &&
        s.character?.modelLoaded &&
        s.character.grounded &&
        s.combat?.total === 8 &&
        s
      );
    },
    "The source zombie encounter did not become ready",
    180_000,
  );
  healthy(initial);
  readyEstablished = true;
  grass(initial);
  await Promise.all(bodies);
  const day = report.scenes.find((s) =>
    new URL(s.url).pathname.endsWith("/scene-ground.json"),
  );
  const night = report.scenes.find((s) =>
    new URL(s.url).pathname.endsWith("/scene-ground-night.json"),
  );
  assert(day && night);
  assert.equal(day.scene.enemyCharacterURL, night.scene.enemyCharacterURL);
  assert.equal(day.scene.characterURL, night.scene.characterURL);
  for (const { scene } of [day, night]) {
    assert.equal(scene.instances.length, 1);
    assert.equal(scene.meshes[0].name, "OpenGroundPlane");
    assert.equal(scene.source.groundZone.profile, "ground-sky-player-v1");
    assert(
      scene.environmentURL &&
        scene.collisionURL &&
        scene.grassURL &&
        scene.enemyCharacterURL,
    );
    for (const field of [
      "gameplayURL",
      "enemyPatrolsURL",
      "creaturesURL",
      "particlesURL",
      "waterURL",
    ])
      assert(!scene[field], `The standalone encounter contains ${field}`);
  }
  record(
    "Original ZombieGuy body, four authored clips and three exact source PBR/glow maps load",
    checkSourceZombie(),
  );
  record(
    "Restart restores 200 health, eight living zombies and the same unedited spawn",
    await restart("zombie-encounter-initial"),
  );
  const ui = await page.evaluate(() => {
    const r = document.getElementById("preview-frame").getBoundingClientRect();
    return {
      text: document.body.innerText,
      frame: { x: r.x, y: r.y, width: r.width, height: r.height },
      editors: [
        ...document.querySelectorAll(
          "textarea,input[type=text],[contenteditable=true]",
        ),
      ].filter((e) => e.getClientRects().length).length,
    };
  });
  assert.equal(ui.editors, 0);
  assert(
    Math.abs(ui.frame.x) < 1 &&
      Math.abs(ui.frame.y) < 1 &&
      Math.abs(ui.frame.width - 1280) < 1 &&
      Math.abs(ui.frame.height - 720) < 1,
  );
  assert.match(ui.text, /W\/S move.*A\/D turn/s);
  assert.match(ui.text, /Space attack.*Backspace dance/s);
  assert.match(ui.text, /8 alive/);
  assert.match(ui.text, /0 defeated/);
  record(
    "Fullscreen combat HUD keeps health, counts, Restart and source movement controls",
    { ui, rendering: rendering(await status()) },
  );
  record(
    "All eight zombies pursue at the source speed and damage follows visible attack windup",
    await pursuitAndDamage(),
  );
  record(
    "Real Space sword input kills a forward zombie, holds Death and removes its health bar",
    await swordKill(),
  );
  record(
    "An idle player can die, cannot move or turn afterward, and can restart",
    await playerDeath(),
  );
  record(
    "Fifteen seconds of eight active zombies and two grass LODs meet the submitted-frame target",
    await frameSample(),
  );
  await restart();
  await page.waitForTimeout(1200);
  const settings = report.grassSettings[0]?.manifest?.reference?.groundShading;
  assert(settings, "The ground shading reference was not prepared");
  assert.deepEqual(settings.nearColorLinear, [0.05, 0.2, 0.01]);
  assert.deepEqual(settings.farColorLinear, settings.nearColorLinear);
  assert.deepEqual(settings.occlusionRange, [0.25, 0.45]);
  assert.equal(settings.colorDistance, undefined);
  await capture("zombie-grass-horizon-daylight");
  record(
    "Daylight ground retains green horizon metadata and supplies a visual review capture",
    { settings, capture: report.captures.at(-1) },
  );
  report.limitations.push(
    "Horizon naturalness, character poses, glow and health-bar placement require visual review of the saved screenshots; submission counters alone do not prove image quality.",
  );
  const final = await status();
  healthy(final);
  rendering(final);
  assert.equal(final.revision, worldRevision);
  report.finalStatus = final;
  report.graphicsMessages = await page.evaluate(() => window.__zombieGraphics);
  await Promise.all(bodies);
  assert.deepEqual(report.graphicsMessages, []);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.responseFailures, []);
  assert.deepEqual(
    report.failed.filter((f) => new URL(f.url).pathname !== "/favicon.ico"),
    [],
  );
  assert(
    !report.console.some(
      (m) =>
        m.type === "error" &&
        /GPUValidation|device lost|WGSL.*error/i.test(m.text),
    ),
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
  try {
    report.finalStatus = (await snapshot()).status;
  } catch {}
  try {
    report.graphicsMessages = await page.evaluate(
      () => window.__zombieGraphics,
    );
  } catch {}
  try {
    await capture("zombie-encounter-failure");
  } catch {}
} finally {
  closing = true;
  try {
    await releaseInput(true);
  } catch {}
  try {
    const frame = await (
      await page.locator("#preview-frame").elementHandle()
    )?.contentFrame();
    report.inputEvents = await frame?.evaluate(() => window.__zombieInput);
  } catch {}
  await Promise.allSettled(bodies);
  report.finishedAt = new Date().toISOString();
  try {
    await writeFile(
      resolve(output, "zombie-encounter-browser.json"),
      JSON.stringify(report, null, 2) + "\n",
    );
  } finally {
    await page.close().catch(() => {});
    await context.close().catch(() => {});
    await browser.close();
  }
}
