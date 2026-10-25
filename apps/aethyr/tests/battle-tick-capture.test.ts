import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BATTLE_TICK_MAX,
  battleStateDigest,
  battleTickPlan,
  compareBattleTickReports,
  compareRgba,
  pngSize,
} from "../tools/battle-tick-capture.mjs";
import { BATTLE_HOLD_LIMITS } from "../packages/renderer/battle-crowd.ts";

test("fixed-tick capture plans default to the million-fighter spectator views and bound every knob", () => {
  assert.equal(BATTLE_TICK_MAX, BATTLE_HOLD_LIMITS.maxTick);
  assert.deepEqual(battleTickPlan({}), {
    count: 1_000_000,
    seed: 1,
    tick: 180,
    views: ["overview", "orbit", "dolly"],
    cameraSeconds: 3,
    time: 3,
    viewport: { width: 1280, height: 720 },
    label: "",
  });
  const plan = battleTickPlan({
    AF_BATTLE_COUNT: "10000",
    AF_BATTLE_SEED: "7",
    AF_BATTLE_TICK: "600",
    AF_BATTLE_VIEWS: "dolly",
    AF_BATTLE_CAMERA_SECONDS: "0",
    AF_BATTLE_TIME: "12.5",
    AF_BATTLE_LABEL: "af-f42c02c",
  });
  assert.equal(plan.tick, 600);
  assert.equal(plan.cameraSeconds, 0);
  assert.equal(plan.time, 12.5);
  assert.deepEqual(plan.views, ["dolly"]);
  for (const env of [
    { AF_BATTLE_TICK: "-1" },
    { AF_BATTLE_TICK: "1.5" },
    { AF_BATTLE_TICK: String(BATTLE_TICK_MAX + 1) },
    { AF_BATTLE_VIEWS: "first" },
    { AF_BATTLE_VIEWS: "overview,overview" },
    { AF_BATTLE_COUNT: "99" },
    { AF_BATTLE_SEED: String(2 ** 32) },
    { AF_BATTLE_TIME: "-1" },
    { AF_BATTLE_CAMERA_SECONDS: "NaN" },
    { AF_BATTLE_LABEL: "../escape" },
  ])
    assert.throws(() => battleTickPlan(env), Error, JSON.stringify(env));
});

const probe = () => ({
  count: 1000,
  finite: true,
  tick: 180,
  sampleSha256: "a".repeat(64),
  gpu: { alive: 900, dead: 100, ticks: 180, damage: 4000, visible: 600, culled: 400, near: 100, far: 500, triangles: 1e6, visibleClasses: [1, 2, 3, 4], projectilesVisible: 3 },
  samples: [{ id: 0, kind: "paladin", team: 0, position: [1, 0, 2], yaw: 0.5, health: 200, maxHealth: 200, state: "walk", actionTime: 0.1, target: 4 }],
  participant: { position: [0, 0, 0], yaw: 0, health: 200 },
  projectiles: [],
  sieges: [{ id: 0 }],
  draws: [{ kind: "paladin", lod: "near", indexCount: 3, instanceCount: 100 }],
});

test("the battle state digest ignores the camera but not the simulation", () => {
  const base = battleStateDigest(probe());
  assert.equal(base.tick, 180);
  assert.match(base.sha256, /^[a-f0-9]{64}$/);
  const otherCamera = probe();
  Object.assign(otherCamera.gpu, { visible: 10, culled: 990, near: 1, far: 9, triangles: 5, visibleClasses: [0, 0, 0, 10], projectilesVisible: 0 });
  otherCamera.draws[0].instanceCount = 1;
  otherCamera.sampleSha256 = "b".repeat(64);
  assert.equal(battleStateDigest(otherCamera).sha256, base.sha256);
  // Key order does not matter.
  const reordered = probe();
  reordered.samples = [Object.fromEntries(Object.entries(reordered.samples[0]).reverse()) as any];
  assert.equal(battleStateDigest(reordered).sha256, base.sha256);
  for (const change of [
    (p: any) => (p.samples[0].position[0] = 1.0000001),
    (p: any) => (p.gpu.damage += 1),
    (p: any) => (p.tick = 181),
    (p: any) => (p.sieges[0].id = 1),
  ]) {
    const changed = probe();
    change(changed);
    assert.notEqual(battleStateDigest(changed).sha256, base.sha256);
  }
  assert.throws(() => battleStateDigest({}));
  const bad = probe();
  bad.samples[0].yaw = NaN;
  assert.throws(() => battleStateDigest(bad), /non-finite/);
});

test("pixel comparison counts every differing pixel and its largest channel delta", () => {
  const a = new Uint8Array(4 * 6).fill(10),
    b = Uint8Array.from(a);
  assert.deepEqual(compareRgba(a, b, 3, 2), { pixels: 6, differing: 0, maxDelta: 0, identical: true });
  b[1] = 18;
  b[22] = 9;
  assert.deepEqual(compareRgba(a, b, 3, 2), { pixels: 6, differing: 2, maxDelta: 8, identical: false });
  assert.throws(() => compareRgba(a, b.subarray(4), 3, 2));
  const png = new Uint8Array(24);
  png.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 73, 72, 68, 82]);
  new DataView(png.buffer).setUint32(16, 1280);
  new DataView(png.buffer).setUint32(20, 720);
  assert.deepEqual(pngSize(png), { width: 1280, height: 720 });
  assert.throws(() => pngSize(new Uint8Array(24)), /PNG/);
});

test("report comparison requires the same plan, state and bytes for every view", async () => {
  const plan = { count: 1_000_000, seed: 1, tick: 180, cameraSeconds: 3, time: 3 };
  const view = (name: string, rgba: string) => ({ view: name, width: 2, height: 1, stateSha256: "s", rgbaSha256: rgba, rgbaFile: `${name}-${rgba}.rgba` });
  const frames: Record<string, Uint8Array> = {
    "overview-x.rgba": new Uint8Array([1, 2, 3, 255, 4, 5, 6, 255]),
    "overview-y.rgba": new Uint8Array([1, 2, 3, 255, 4, 5, 9, 255]),
  };
  const load = async (_report: any, v: any) => frames[v.rgbaFile];
  const a = { revision: "a", plan, views: [view("overview", "x"), view("dolly", "d")] };
  const same = await compareBattleTickReports(a, { ...a, revision: "b" }, load);
  assert.equal(same.ok, true);
  assert.deepEqual(same.revisions, ["a", "b"]);
  const differ = await compareBattleTickReports(a, { revision: "b", plan, views: [view("overview", "y"), view("dolly", "d")] }, load);
  assert.equal(differ.ok, false);
  assert.deepEqual(differ.views[0].pixels, { pixels: 2, differing: 1, maxDelta: 3, identical: false });
  assert.equal(differ.views[1].ok, true);
  const otherTick = await compareBattleTickReports(a, { ...a, plan: { ...plan, tick: 181 } }, load);
  assert.deepEqual(otherTick.mismatchedPlan, ["tick"]);
  assert.equal(otherTick.ok, false);
  const missing = await compareBattleTickReports(a, { ...a, views: [view("overview", "x")] }, load);
  assert.equal(missing.ok, false);
  const otherState = await compareBattleTickReports(a, { ...a, views: [{ ...view("overview", "x"), stateSha256: "t" }, view("dolly", "d")] }, load);
  assert.equal(otherState.views[0].state, false);
  assert.equal(otherState.ok, false);
});
