/** Verify the complete local MMO through a disposable openagents browser profile. */
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
const output = resolve(process.env.AF_CAPTURE_DIR ?? "");
assert(process.env.AF_CAPTURE_DIR, "Choose a private capture directory.");
await mkdir(output, { recursive: true });
assert(
  !(await realpath(output)).startsWith((await realpath(".")) + sep),
  "Keep captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`;
assert(
  process.env.OPENAGENTS_CHROME_WS || process.env.OPENAGENTS_CHROME_PORT,
  "Use openagents browser run.",
);
const url = process.env.AF_EDITOR_URL ?? "http://127.0.0.1:55316/";
const report = {
  schema: "aethyrforge.mmorpg-world-browser.v1",
  ok: false,
  startedAt: new Date().toISOString(),
  checks: [],
  errors: [],
  warnings: [],
  failures: [],
  samples: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
const page = await context.newPage();
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.warnings.push(message.text().slice(0, 4096));
});
page.on("requestfailed", (request) =>
  report.failures.push({
    url: request.url(),
    error: request.failure()?.errorText,
  }),
);
page.on("response", (response) => {
  if (response.status() >= 400)
    report.failures.push({ url: response.url(), status: response.status() });
});
const status = () => page.evaluate(() => window.aethyrforge?.snapshot().status);
const controls = (
  forward = 0,
  turn = 0,
  sprint = false,
  attack = false,
  dance = false,
) =>
  page.evaluate(
    (input) => window.aethyrforgeEditor.service.send({ type: "input", input }),
    { forward, right: turn, up: 0, sprint, jump: false, attack, dance },
  );
const shortestAngle = (value) => Math.atan2(Math.sin(value), Math.cos(value));
async function fight(kind, range, capture) {
  let state = await status();
  const candidates = state.mmorpg.actors.filter(
    (a) => a.kind === "npc" && a.class === kind && a.alive,
  );
  candidates.sort(
    (a, b) =>
      Math.hypot(
        a.position[0] - state.mmorpg.player.position[0],
        a.position[2] - state.mmorpg.player.position[2],
      ) -
      Math.hypot(
        b.position[0] - state.mmorpg.player.position[0],
        b.position[2] - state.mmorpg.player.position[2],
      ),
  );
  const target = candidates[0];
  assert(target, `No nearby ${kind}`);
  const record = { target, frames: [], damageParticles: 0, castParticles: 0 };
  const deadline = Date.now() + 45000;
  let captured = false;
  while (Date.now() < deadline) {
    state = await status();
    if (state.phase !== "ready") throw Error(state.message);
    assert(state.mmorpg.player.alive, "Player died during the fight");
    const actor = state.mmorpg.actors.find((a) => a.id === target.id);
    assert(actor, "The target left the nearby view");
    const local = state.mmorpg.player,
      dx = actor.position[0] - local.position[0],
      dz = actor.position[2] - local.position[2];
    const distance = Math.hypot(dx, dz),
      delta = shortestAngle(Math.atan2(dx, -dz) - local.yaw);
    const turn = Math.max(-1, Math.min(1, delta * 2));
    const attacking = distance <= range && Math.abs(delta) < 0.2;
    await controls(
      !attacking && Math.abs(delta) < 0.35 ? 1 : 0,
      Math.abs(delta) > 0.04 ? turn : 0,
      distance > 6,
      attacking,
    );
    record.damageParticles = Math.max(
      record.damageParticles,
      state.runtimeMetrics.mmorpgParticles ?? 0,
    );
    if (local.state === "attack")
      record.castParticles = Math.max(
        record.castParticles,
        state.runtimeMetrics.mmorpgParticles ?? 0,
      );
    if (
      !captured &&
      local.state === "attack" &&
      (state.runtimeMetrics.mmorpgParticles ?? 0) > 0
    ) {
      await page.screenshot({
        path: resolve(output, capture + "-casting.png"),
      });
      captured = true;
    }
    if (!actor.alive) {
      await controls();
      const rendered = await wait(
        (next) =>
          next.frame > state.frame &&
          (next.runtimeMetrics.mmorpgParticles ?? 0) > 0,
        "The accepted hit did not reach the particle draw",
        2,
      );
      record.damageParticles = Math.max(
        record.damageParticles,
        rendered.runtimeMetrics.mmorpgParticles,
      );
      record.final = {
        player: local,
        target: actor,
        metrics: rendered.runtimeMetrics,
      };
      await page.screenshot({ path: resolve(output, capture + "-hit.png") });
      report.checks.push(
        `${local.class} defeated ${kind} through authoritative combat`,
      );
      console.log(
        "Combat passed",
        JSON.stringify({
          class: local.class,
          target: kind,
          health: local.health,
          particles: record.damageParticles,
        }),
      );
      return record;
    }
    if (record.frames.length < 500)
      record.frames.push({
        at: Date.now(),
        position: local.position,
        state: local.state,
        targetHealth: actor.health,
        distance,
        delta,
      });
    await page.waitForTimeout(100);
  }
  await controls();
  throw Error(`The ${kind} fight did not finish`);
}
async function fullStory() {
  report.paladinZombie = await fight("zombie", 1.2, "paladin-blood");
  assert(report.paladinZombie.final.target.state === "death");
  assert(
    report.paladinZombie.damageParticles > 0,
    "No actual blood particles were submitted",
  );
  report.paladinWarrok = await fight("warrok", 1.2, "paladin-warrok");
  assert.equal(report.paladinWarrok.final.target.health, 0);
  assert(
    report.paladinWarrok.final.player.health < 200,
    "No enemy strike damaged the player",
  );
  await page.reload();
  await wait(
    (s) => s?.phase === "ready" && s.mmorpg?.connection === "login",
    "Fresh class login did not open",
    60,
  );
  await page.locator("#mmo-name").fill("Port Sorceror");
  await page.locator("#mmo-class").selectOption("sorceror");
  await page.getByRole("button", { name: "Enter world", exact: true }).click();
  await wait(
    (s) =>
      s?.mmorpg?.connection === "online" &&
      s.mmorpg.player.class === "sorceror",
    "Sorceror did not join",
  );
  report.sorcerorZombie = await fight("zombie", 5.5, "sorceror-fire");
  assert(
    report.sorcerorZombie.castParticles > 0,
    "No actual hand fire was submitted",
  );
  await controls();
  await page.waitForTimeout(500);
  const before = await status(),
    started = performance.now(),
    samples = [];
  for (let i = 0; i < 60; i++) {
    await controls(1, i < 20 ? 0 : i < 40 ? 0.3 : -0.3, true);
    await page.waitForTimeout(250);
    const current = await status();
    if (current.phase !== "ready") throw Error(current.message);
    assert(current.mmorpg.player.alive, "Player died while running away");
    samples.push({
      frame: current.frame,
      fps: current.fps,
      position: current.mmorpg.player.position,
      actors: current.runtimeMetrics.mmorpgActors,
      classes: current.runtimeMetrics.mmorpgClasses,
      terrainUpdates: current.runtimeMetrics.mmorpgTerrainUpdates,
    });
  }
  await controls();
  const after = await status(),
    seconds = (performance.now() - started) / 1000;
  report.movingPerformance = {
    seconds,
    submittedFrames:
      after.runtimeMetrics.submittedFrames -
      before.runtimeMetrics.submittedFrames,
    submissionFps:
      (after.runtimeMetrics.submittedFrames -
        before.runtimeMetrics.submittedFrames) /
      seconds,
    samples,
  };
  assert(
    report.movingPerformance.submissionFps >=
      Number(process.env.AF_MIN_FPS ?? 54),
    `Moving cadence ${report.movingPerformance.submissionFps.toFixed(2)} FPS`,
  );
  assert(
    after.runtimeMetrics.mmorpgTerrainUpdates >
      before.runtimeMetrics.mmorpgTerrainUpdates,
    "Terrain did not follow the moving player",
  );
  assert(
    Math.hypot(
      ...after.mmorpg.player.position.map(
        (v, i) => v - before.mmorpg.player.position[i],
      ),
    ) > 20,
    "Source controls did not move the player",
  );
  report.checks.push(
    "Sorceror fire, moving terrain/scenery, and sustained moving render cadence",
  );
  await page.screenshot({ path: resolve(output, "mmorpg-running-field.png") });
  if (process.env.AF_MMO_LIFECYCLE === "1") await deathAndRespawn();
}

async function deathAndRespawn() {
  let state = await status();
  const origin = [...state.mmorpg.player.position];
  const target = state.mmorpg.actors
    .filter((actor) => actor.kind === "npc" && actor.alive)
    .sort(
      (a, b) =>
        Number(b.class === "warrok") - Number(a.class === "warrok") ||
        Math.hypot(a.position[0] - origin[0], a.position[2] - origin[2]) -
          Math.hypot(b.position[0] - origin[0], b.position[2] - origin[2]),
    )[0];
  assert(target, "No nearby enemy for the death/respawn check");
  const deadline = Date.now() + 180000;
  const health = [];
  while (state.mmorpg.player.alive && Date.now() < deadline) {
    const local = state.mmorpg.player;
    const actor = state.mmorpg.actors.find((value) => value.id === target.id);
    assert(actor?.alive, "The enemy left before the lifecycle check");
    const dx = actor.position[0] - local.position[0],
      dz = actor.position[2] - local.position[2];
    const delta = shortestAngle(Math.atan2(dx, -dz) - local.yaw);
    const distance = Math.hypot(dx, dz);
    await controls(
      distance > 0.8 && Math.abs(delta) < 0.35 ? 1 : 0,
      Math.abs(delta) > 0.04 ? Math.max(-1, Math.min(1, delta * 2)) : 0,
      distance > 6,
    );
    if (health.at(-1) !== local.health) health.push(local.health);
    await page.waitForTimeout(200);
    state = await status();
    assert.equal(state.phase, "ready", state.message);
  }
  await controls();
  assert.equal(
    state.mmorpg.player.alive,
    false,
    "Enemy attacks did not kill the Sorceror",
  );
  assert.equal(state.mmorpg.player.health, 0);
  const fallen = state.mmorpg.player;
  await page.getByRole("heading", { name: "You have fallen" }).waitFor();
  await page.screenshot({ path: resolve(output, "mmorpg-player-death.png") });
  await page
    .getByRole("button", { name: "Return to the world", exact: true })
    .click();
  const respawned = await wait(
    (next) => next.mmorpg.player.alive && next.mmorpg.player.health === 100,
    "The accepted respawn did not restore the player",
  );
  assert.equal(respawned.mmorpg.player.id, fallen.id);
  assert.deepEqual(respawned.mmorpg.player.inventory, fallen.inventory);
  assert.equal(respawned.mmorpg.player.class, "sorceror");
  report.lifecycle = { health, fallen, respawned: respawned.mmorpg.player };
  report.checks.push(
    "Enemy death, source death animation, and personal respawn preserve identity and inventory",
  );
  await page.screenshot({ path: resolve(output, "mmorpg-respawn.png") });
}

async function wait(predicate, message, seconds = 30) {
  const until = Date.now() + seconds * 1000;
  while (Date.now() < until) {
    const value = await status();
    if (value?.phase === "error") throw Error(value.message);
    if (predicate(value)) return value;
    await page.waitForTimeout(100);
  }
  throw Error(`${message}: ${JSON.stringify(await status())}`);
}
try {
  await page.goto(url);
  const ready = await wait(
    (s) => s?.phase === "ready" && !!s.mmorpg,
    "Game did not open",
    120,
  );
  report.checks.push("Four-class game and source UI opened");
  await page.locator("#mmo-name").fill("Port Paladin");
  await page.getByRole("button", { name: "Enter world", exact: true }).click();
  const online = await wait(
    (s) => s?.mmorpg?.connection === "online",
    "Could not join",
  );
  assert.equal(online.mmorpg.player.class, "paladin");
  assert.equal(online.mmorpg.player.health, 200);
  report.checks.push("Guest Paladin joined the authoritative world");
  console.log(
    "MMO joined",
    JSON.stringify({
      fps: online.fps,
      population: online.mmorpg.population,
      actors: online.mmorpg.actors.length,
    }),
  );
  if (process.env.AF_MMO_FULL === "1") await fullStory();
  await page.waitForTimeout(3000);
  await page.screenshot({ path: resolve(output, "mmorpg-initial.png") });
  const before = await status();
  for (let i = 0; i < 20; i++) {
    await page.waitForTimeout(250);
    const s = await status();
    assert.equal(s.phase, "ready", s.message);
    report.samples.push({
      frame: s.frame,
      fps: s.fps,
      player: s.mmorpg.player,
      actors: s.mmorpg.actors.length,
    });
  }
  const after = await status();
  report.initial = ready;
  report.online = online;
  report.final = after;
  report.submittedFrames = after.frame - before.frame;
  assert(report.submittedFrames > 0);
  assert.equal(report.errors.length, 0);
  assert.equal(report.failures.length, 0);
  report.ok = true;
  console.log(
    "MMO startup probe passed",
    JSON.stringify({
      frames: report.submittedFrames,
      fps: after.fps,
      checks: report.checks,
    }),
  );
} catch (error) {
  report.errors.push(String(error));
  try {
    report.final = await status();
    await page.screenshot({ path: resolve(output, "mmorpg-error.png") });
  } catch {}
  console.error(String(error));
  process.exitCode = 1;
} finally {
  await writeFile(
    resolve(output, "mmorpg-world-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await context.close();
  await browser.close();
}
