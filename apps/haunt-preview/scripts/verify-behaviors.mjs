import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
import { PNG } from "pngjs";

const url = process.env.HAUNT_URL;
if (!url) throw new Error("Set HAUNT_URL to the local editor.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "behaviors",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0],
  page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 1060 });
const errors = [],
  posts = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("request", (r) => {
  if (r.method() === "POST") posts.push(new URL(r.url()).pathname);
});
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
const capture = async (name) => {
  const bytes = Buffer.from(
    await page.evaluate(async () => [
      ...new Uint8Array(
        await (await window.hauntPreview.capture()).arrayBuffer(),
      ),
    ]),
  );
  await writeFile(`${out}/${name}.png`, bytes);
  return PNG.sync.read(bytes);
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
      !document.querySelector("#add-pumpkin").disabled,
  );
  // Fix the lantern flicker so changed capture pixels establish real mesh movement.
  const source = await page.evaluate(() => window.hauntPreview.original());
  const fixed = source.replace(/let flicker = .*;/, "let flicker = 1.0;");
  assert.equal(
    (await page.evaluate((s) => window.hauntPreview.save(s), fixed)).type,
    "accepted",
  );
  for (const [id, x] of [
    ["ghost-behavior-one", -1.4],
    ["ghost-behavior-two", 1.4],
  ])
    await command({
      kind: "add_asset",
      entity: { id, kind: "ghost", x, z: 3, scale: 1 },
    });
  const initial = await snap();
  report.initial = {
    project: initial.project,
    pose: initial.pose,
    compiles: initial.compiles,
    geometryBuilds: initial.geometryBuilds,
  };
  report.decisions.push(await ask("make the ghosts float around"));
  assert.equal(report.decisions[0].decision.target_scope, "all_ghosts");
  const floating = await snap();
  assert.ok(
    floating.project.entities
      .filter((e) => e.kind === "ghost")
      .every((e) => e.behavior === "float"),
  );
  assert.equal(floating.project.revision, initial.project.revision + 1);
  const imageA = await capture("float-a");
  await page.waitForFunction(
    (before) =>
      window.hauntPreview
        .snapshot()
        .actors.some(
          (a) =>
            Math.hypot(
              a.x - before.find((b) => b.id === a.id).x,
              a.z - before.find((b) => b.id === a.id).z,
            ) > 0.15,
        ),
    floating.actors,
  );
  const imageB = await capture("float-b"),
    floated = await snap();
  let changed = 0;
  for (let i = 0; i < imageA.data.length; i += 4)
    if (
      imageA.data[i] !== imageB.data[i] ||
      imageA.data[i + 1] !== imageB.data[i + 1] ||
      imageA.data[i + 2] !== imageB.data[i + 2]
    )
      changed++;
  assert.ok(changed > 100, "no rendered ghost movement");
  report.changedPixels = changed;
  assert.deepEqual(floated.project, floating.project);
  assert.deepEqual(floated.pose, initial.pose);
  assert.equal(floated.compiles, initial.compiles);
  assert.equal(floated.geometryBuilds, initial.geometryBuilds);
  report.checks.push(
    "exact plural request floats all ghosts; captures with fixed lighting show moving geometry; simulation does not change authored positions or add history",
  );
  await page.locator("#selected").selectOption("ghost-behavior-two");
  report.decisions.push(await ask("make that ghost chase me"));
  const chasing = await snap(),
    actor = chasing.actors.find((a) => a.id === "ghost-behavior-two");
  const distance = Math.hypot(
    actor.x - chasing.pose.x,
    actor.z - chasing.pose.z,
  );
  await page.waitForFunction(
    ({ distance, id }) => {
      const s = window.hauntPreview.snapshot(),
        a = s.actors.find((a) => a.id === id);
      return Math.hypot(a.x - s.pose.x, a.z - s.pose.z) < distance - 0.3;
    },
    { distance, id: actor.id },
  );
  const caught = await snap();
  assert.equal(
    caught.project.entities.find((e) => e.id === actor.id).behavior,
    "chase",
  );
  assert.equal(
    caught.project.entities.find((e) => e.id !== actor.id && e.kind === "ghost")
      .behavior,
    "float",
  );
  assert.deepEqual(caught.pose, initial.pose);
  const surface = page.locator("#house");
  await surface.focus();
  await page.keyboard.down("KeyD");
  await page.waitForTimeout(350);
  await page.keyboard.up("KeyD");
  await page.waitForFunction(
    (x) => window.hauntPreview.snapshot().pose.x > x + 0.3,
    initial.pose.x,
  );
  report.checks.push(
    "exact singular request changes only the selected ghost; it approaches the player while walking stays responsive",
  );
  report.decisions.push(await ask("make that ghost stop moving"));
  const stopped = await snap(),
    frozen = stopped.actors.find((a) => a.id === actor.id);
  await page.waitForFunction(
    (frame) => window.hauntPreview.snapshot().frame > frame + 15,
    stopped.frame,
  );
  assert.deepEqual(
    (await snap()).actors.find((a) => a.id === actor.id),
    frozen,
  );
  assert.equal(
    stopped.project.entities.find((e) => e.id === actor.id).x,
    frozen.x,
  );
  assert.equal(
    stopped.project.entities.find((e) => e.id === actor.id).z,
    frozen.z,
  );
  await command({ kind: "undo" });
  assert.equal(
    (await snap()).project.entities.find((e) => e.id === actor.id).behavior,
    "chase",
  );
  await command({ kind: "redo" });
  assert.equal(
    (await snap()).project.entities.find((e) => e.id === actor.id).behavior,
    "still",
  );
  report.checks.push(
    "stop freezes the current live position and saves it; behavior and group changes support undo/redo",
  );
  for (const [button, mode] of [
    ["float-ghost", "float"],
    ["chase-ghost", "chase"],
    ["stop-ghost", "still"],
  ]) {
    await page.locator(`#${button}`).click();
    await page.waitForFunction(
      ({ id, mode }) =>
        window.hauntPreview.snapshot().project.entities.find((e) => e.id === id)
          .behavior === mode && !document.querySelector("#ask").disabled,
      { id: actor.id, mode },
    );
  }
  const builds = (await snap()).geometryBuilds;
  const samples = [];
  for (let i = 0; i < 30; i++)
    samples.push(
      await command({
        kind: "set_behavior",
        ids: [actor.id],
        behavior: i % 2 ? "chase" : "float",
      }),
    );
  assert.equal((await snap()).geometryBuilds, builds);
  report.samples = samples;
  report.checks.push(
    "direct behavior controls and 30 repeated rule changes keep the existing geometry buffer and shader",
  );
  await command({
    kind: "batch",
    operations: [
      { kind: "set_behavior", ids: [actor.id], behavior: "still" },
      { kind: "set_tint", tint: "warm" },
    ],
  });
  const batchStopped = await snap(),
    batchActor = batchStopped.actors.find((a) => a.id === actor.id);
  assert.equal(
    batchStopped.project.entities.find((e) => e.id === actor.id).x,
    batchActor.x,
  );
  assert.equal(
    batchStopped.project.entities.find((e) => e.id === actor.id).z,
    batchActor.z,
  );
  await command({ kind: "undo" });
  report.checks.push(
    "batched stop captures the live position in the same atomic transaction",
  );
  const beforeBad = await snap();
  const bad = await page.evaluate(() =>
    window.hauntPreview.save("invalid WGSL"),
  );
  assert.equal(bad.type, "rejected");
  await page.waitForFunction(
    (ticks) => window.hauntPreview.snapshot().behaviorTicks > ticks + 5,
    beforeBad.behaviorTicks,
  );
  const afterBad = await snap();
  assert.equal(afterBad.hash, beforeBad.hash);
  assert.deepEqual(afterBad.project, beforeBad.project);
  assert.ok(
    afterBad.actors.some(
      (a) =>
        Math.hypot(
          a.x - beforeBad.actors.find((b) => b.id === a.id).x,
          a.z - beforeBad.actors.find((b) => b.id === a.id).z,
        ) > 0.01,
    ),
  );
  await page.evaluate((s) => window.hauntPreview.save(s), fixed);
  const beforeUnsupported = await snap();
  await page.evaluate(() =>
    window.hauntPreview
      .request("make the ghosts shoot fireballs")
      .catch(() => {}),
  );
  assert.deepEqual((await snap()).project, beforeUnsupported.project);
  assert.ok(
    await page.evaluate(async () => {
      try {
        await window.hauntPreview.command({
          kind: "set_behavior",
          ids: ["pumpkin-1"],
          behavior: "chase",
        });
        return false;
      } catch {
        return true;
      }
    }),
  );
  report.checks.push(
    "invalid shaders retain the working shader and running behaviors; unsupported behaviors and non-ghost targets make no scene change",
  );
  await command({
    kind: "set_behavior",
    ids: initial.actors.map((a) => a.id),
    behavior: "float",
  });
  const saved = await snap();
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => window.kilnReadScene())),
    saved.project,
  );
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
  await reopened.waitForFunction(() =>
    window.hauntPreview.snapshot().actors.some((a) => a.y > 0.02),
  );
  await reopened.close();
  assert.equal(saved.buffers, 2);
  assert.equal(saved.activePipelines, 1);
  assert.equal(saved.depthTextures, 1);
  assert.deepEqual(errors, []);
  assert.ok(posts.every((p) => p === "/api/haunt/decision"));
  report.final = {
    project: saved.project,
    actors: saved.actors,
    pose: saved.pose,
    buffers: saved.buffers,
    compiles: saved.compiles,
    geometryBuilds: saved.geometryBuilds,
  };
  await page.screenshot({ path: `${out}/floating-ghosts.png`, fullPage: true });
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(() => window.hauntPreview.snapshot().disposed);
  assert.equal((await snap()).buffers, 0);
  report.checks.push(
    "rules persist through real Kiln OPFS/reopen; animation resumes from authored positions; teardown releases resources",
  );
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        changedPixels: changed,
        decisions: report.decisions.map((r) => ({
          text: r.text,
          totalMs: r.totalMs,
          behavior: r.decision.behavior,
          target: r.decision.target_scope,
        })),
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
