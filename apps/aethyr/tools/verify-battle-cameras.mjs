/** Real application camera transitions through typed editor actions. No FPS claim. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { battleSourcePin } from "./battle-evidence.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR);
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(output !== root && !output.startsWith(root + sep));
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`;
assert(
  process.env.OPENAGENTS_CHROME_WS || process.env.OPENAGENTS_CHROME_PORT,
  "Run through openagents browser run.",
);
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55326/");
assert(["127.0.0.1", "localhost"].includes(url.hostname));
url.searchParams.set("battle", "10000");
const report = {
  schema: "aethyrforge.battle-cameras.v1",
  ok: false,
  startedAt: new Date().toISOString(),
  sourcePin: await battleSourcePin(root),
  checks: [],
  captures: [],
  errors: [],
  failed: [],
  limitations: [
    "Uses public typed battle inputs and the editor's bounded camera-distance request. Native pointer/right-drag/wheel gestures need a separate visible UI check.",
    "Camera geometry and captures are application correctness evidence, not a sustained FPS measurement.",
  ],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
const page = await context.newPage();
let closing = false;
page.on("pageerror", (e) => report.errors.push(String(e)));
page.on("requestfailed", (r) => {
  if (!closing)
    report.failed.push({ url: r.url(), error: r.failure()?.errorText });
});
page.on("response", (r) => {
  if (!closing && r.status() >= 400)
    report.failed.push({ url: r.url(), status: r.status() });
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, label, timeout = 30000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const s = await snapshot();
    if (s?.status?.phase === "error") throw Error(s.status.message);
    if (predicate(s)) return s;
    await wait(80);
  }
  throw Error(label);
}
async function action(a) {
  const id = await page.evaluate((a) => window.aethyrforge.battle(a), a);
  const s = await until(
    (s) => s.receipts.some((r) => r.requestId === id),
    `Action ${a.op} stalled`,
  );
  const r = s.receipts.find((r) => r.requestId === id);
  assert(r.ok, r.message);
  return r;
}
const read = () =>
  page.evaluate(() => window.aethyrforge.cTownBenchmark({ op: "read" }));
const neutral = {
  op: "input",
  forward: 0,
  turn: 0,
  sprint: false,
  attack: false,
};
const angle = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
async function capture(name) {
  const path = resolve(output, name + ".png");
  await page.screenshot({ path });
  report.captures.push(path);
}
try {
  await page.goto(url.href);
  await until(
    (s) => s?.status?.phase === "ready" && s.status.battle?.enabled,
    "Battle did not open",
    150000,
  );
  for (const count of [300000, 1000000]) {
    await action({ op: "start", count, seed: 1 });
    for (const view of [
      "overview",
      "orbit",
      "dolly",
      "first",
      "third",
      "overview",
    ]) {
      const beforeStatus = (await snapshot()).status;
      const before = beforeStatus.frame;
      const beforeSample = beforeStatus.battle.stats.sampleSerial;
      await action({ op: "camera", view });
      const s = await until(
        (s) =>
          s.status.battle?.count === count &&
          s.status.battle.camera === view &&
          s.status.frame > before + 3 &&
          s.status.battle.stats.sampleSerial > beforeSample &&
          s.status.battle.stats.visible > 0,
        `${count}/${view} did not render`,
      );
      const metrics = await read(),
        camera = metrics.resources.battleCamera;
      assert(camera && camera.eye.every(Number.isFinite));
      assert(Number.isFinite(camera.yaw) && Number.isFinite(camera.pitch));
      if (view === "overview" || view === "orbit")
        assert(camera.sourceFogScale > 40);
      assert(s.status.battle.stats.visible > 0);
      report.checks.push({
        count,
        view,
        ok: true,
        camera,
        visible: s.status.battle.stats.visible,
        alive: s.status.battle.stats.alive,
      });
      await capture(`${count}-${view}-${report.checks.length}`);
    }
  }
  await action({ op: "start", count: 10000, seed: 1 });
  const field = (await snapshot()).status.battle;
  await action({
    op: "join",
    team: 0,
    class: "paladin",
    view: "third",
    at: [field.center[0] - field.radius, field.center[2] + field.radius],
  });
  await until(
    (s) =>
      s.status.battle.participant?.ready && s.status.battle.participant.alive,
    "Participant not ready",
  );
  await action(neutral);
  await wait(400);
  const before = (await snapshot()).status.battle.participant,
    cameraBefore = (await read()).resources.battleCamera;
  await action({ ...neutral, lookX: 0.45, lookY: 0.2 });
  await until(
    (s) => s.status.frame > 0 && s.status.battle.participant.alive,
    "Participant died during camera check",
  );
  await wait(250);
  const after = (await snapshot()).status.battle.participant,
    cameraAfter = (await read()).resources.battleCamera;
  report.freeLook = {
    before,
    after,
    cameraBefore,
    cameraAfter,
    positionMovement: distance(after.position, before.position),
  };
  assert(
    Math.abs(angle(after.yaw, before.yaw)) < 0.002,
    "Free look rotated the fighter",
  );
  assert(
    distance(after.position, before.position) < 0.08,
    "Free look moved the fighter",
  );
  assert(
    Math.abs(angle(cameraAfter.yaw, cameraBefore.yaw)) > 0.3,
    "Look input did not orbit the real camera",
  );
  assert(
    cameraAfter.pitch > cameraBefore.pitch + 0.1,
    "Look input did not change vertical aim",
  );
  report.checks.push({
    name: "independent-free-look",
    ok: true,
    before,
    after,
    cameraBefore,
    cameraAfter,
  });
  for (const requested of [2.5, 18, 40, 6]) {
    await page.evaluate(
      (d) => window.aethyrforgeEditor.service.orbitDistance(d),
      requested,
    );
    const expected = Math.min(requested, 18);
    const s = await until(
      (s) => s.status.character?.orbitDistance === expected,
      "Wheel distance did not reach the battle camera",
    );
    const camera = (await read()).resources.battleCamera;
    report.checks.push({
      name: "bounded-zoom",
      requested,
      expected,
      camera,
      bodyYaw: s.status.battle.participant.yaw,
      ok: true,
    });
  }
  await capture("free-look-third-person");
  const third = (await read()).resources.battleCamera;
  await action({ op: "camera", view: "first" });
  await wait(200);
  const first = (await read()).resources.battleCamera;
  assert(
    Math.abs(angle(first.yaw, before.yaw + 0.45)) < 0.002,
    "First-person switch lost the independent orbit heading",
  );
  await action({ ...neutral, lookX: -0.3, lookY: -0.1 });
  await wait(200);
  const looked = (await read()).resources.battleCamera,
    body = (await snapshot()).status.battle.participant;
  assert(Math.abs(angle(looked.yaw, first.yaw)) > 0.2);
  assert(Math.abs(angle(body.yaw, looked.yaw)) < 0.02);
  await action({ op: "camera", view: "third" });
  await wait(200);
  const returned = (await read()).resources.battleCamera;
  const expectedShoulderYaw =
    looked.yaw - Math.atan2(0.48, Math.cos(looked.pitch) * (6 + 2.6));
  assert(
    Math.abs(angle(returned.yaw, expectedShoulderYaw)) < 0.002,
    "Third-person switch lost the orbit heading after shoulder parallax",
  );
  report.checks.push({
    name: "first-third-aim-continuity",
    ok: true,
    third,
    first,
    looked,
    returned,
  });
  await capture("returned-third-person");
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(`PASS ${report.checks.length} actual application camera checks`);
} catch (e) {
  report.errors.push({ error: String(e), stack: e.stack });
  process.exitCode = 1;
  console.error(e);
} finally {
  closing = true;
  await context.close();
  await browser.close();
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-cameras-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
