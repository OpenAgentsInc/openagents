import assert from "node:assert/strict";
import test from "node:test";
import {
  QUICK_MMORPG_MOTIONS,
  QuickMmorpgCamera,
  QuickMmorpgLocomotion,
  quickMmorpgFrameDelta,
  type QuickMmorpgInput,
} from "../packages/runtime/quick-mmorpg-controller.ts";
import type { CharacterClip } from "../packages/renderer/character.ts";

const durations = { idle: 2, walk: 1, run: 0.5, attack: 1, dance: 2, death: 1 };
const clips = () =>
  QUICK_MMORPG_MOTIONS.map((name) => ({
    name,
    duration: durations[name],
    distance: 0,
    tracks: [],
    loop: !["attack", "dance", "death"].includes(name),
  })) as CharacterClip[];
const released: QuickMmorpgInput = {
  forward: 0,
  sprint: false,
  attack: false,
  dance: false,
};
const player = { position: [6, 0, 20] as [number, number, number], yaw: 0 };
const near = (actual: number, expected: number, tolerance = 1e-9) =>
  assert.ok(
    Math.abs(actual - expected) < tolerance,
    `${actual} != ${expected}`,
  );

test("source input chooses forward/backward gaits and enters sprint through walk", () => {
  const locomotion = new QuickMmorpgLocomotion(clips());
  assert.equal(
    locomotion.advance({ ...released, forward: 1, sprint: true }, 1 / 60)
      .animation,
    "walk",
  );
  assert.equal(
    locomotion.advance({ ...released, forward: 1, sprint: true }, 1 / 60)
      .animation,
    "run",
  );
  assert.equal(
    locomotion.advance({ ...released, forward: -1, sprint: true }, 1 / 60)
      .animation,
    "run",
  );
  assert.equal(
    locomotion.advance({ ...released, forward: -1 }, 1 / 60).animation,
    "walk",
  );
  assert.equal(
    locomotion.advance({ ...released, turn: 1 }, 1 / 60).animation,
    "idle",
  );
  assert.equal(locomotion.pose(player).yaw, Math.PI);
});

test("source walk/run transitions preserve phase and advance both cross-fading actions", () => {
  const locomotion = new QuickMmorpgLocomotion(clips());
  for (let i = 0; i < 24; i++)
    locomotion.advance({ ...released, forward: 1 }, 1 / 60);
  const walkTime = locomotion.time;
  locomotion.advance({ ...released, forward: 1, sprint: true }, 1 / 60);
  const pose = locomotion.pose(player),
    weight = 1 / 60 / 0.1;
  assert.equal(pose.blend?.from, "walk");
  near(pose.blend!.weight, weight);
  near(pose.time, walkTime * 0.5 + (1 / 60) * (0.5 * (1 - weight) + weight));
  near(pose.blend!.fromTime, walkTime + (1 / 60) * (1 + weight));
  locomotion.advance({ ...released, forward: 1, sprint: true }, 1 / 60);
  assert.ok(locomotion.pose(player).blend!.fromTime > pose.blend!.fromTime);
  locomotion.advance(released, 1 / 60);
  near(locomotion.pose(player).blend!.weight, 1 / 60 / 0.25);
  assert.equal(locomotion.pose(player).blend?.from, "run");
});

test("attack and dance lock movement until their one-shot ends; death holds its last frame", () => {
  const locomotion = new QuickMmorpgLocomotion(clips());
  assert.equal(
    locomotion.advance({ ...released, attack: true }, 1 / 60).movementLocked,
    true,
  );
  assert.equal(locomotion.animation, "attack");
  near(locomotion.pose(player).blend!.weight, 1 / 60 / 0.4);
  let locked = 0;
  while (locomotion.animation === "attack" && locked++ < 180)
    locomotion.advance({ ...released, forward: 1 }, 1 / 60);
  assert.ok(locked > 30 && locked < 180);
  assert.equal(locomotion.animation, "idle");
  assert.equal(locomotion.movementLocked, false);
  assert.equal(
    locomotion.advance({ ...released, dance: true }, 1 / 60).animation,
    "dance",
  );
  near(locomotion.pose(player).blend!.weight, 1 / 60 / 0.2);
  locomotion.die();
  for (let i = 0; i < 180; i++)
    locomotion.advance({ ...released, forward: 1 }, 1 / 60);
  assert.equal(locomotion.animation, "death");
  near(locomotion.time, 1);
  assert.equal(locomotion.movementLocked, true);
  locomotion.reset();
  assert.equal(locomotion.animation, "idle");
  assert.equal(locomotion.pose(player).blend, undefined);
});

test("source camera scales offsets, follows exponentially, and keeps its terrain floor", () => {
  const camera = new QuickMmorpgCamera(1 / 6);
  let pose;
  for (let i = 0; i < 60; i++)
    pose = camera.frame(player.position, 0, 0, 1 / 60);
  for (const [actual, expected] of pose!.eye.map((value, i) => [
    value,
    [6, 10 / 6, 22.5][i] * 0.99,
  ]))
    near(actual, expected);
  for (const [actual, expected] of pose!.target.map((value, i) => [
    value,
    [6, 5 / 6, 20 - 20 / 6][i] * 0.99,
  ]))
    near(actual, expected);
  near(pose!.near, 1 / 6);
  near(pose!.far, 10000 / 6);
  near(pose!.fov, Math.PI / 3);
  near(Math.hypot(...pose!.direction), 1);
  const split = new QuickMmorpgCamera(1 / 6);
  for (let i = 0; i < 120; i++)
    pose = split.frame(player.position, 0, 0, 1 / 120);
  near(pose!.eye[2], 22.5 * 0.99);
  const raised = new QuickMmorpgCamera(1 / 6);
  for (let i = 0; i < 300; i++)
    pose = raised.frame([0, 0, 0], Math.PI / 2, 3, 1 / 60);
  assert.ok(pose!.eye[1] > 3.8 && pose!.eye[0] < -2.49);
});

test("source frame cap and input validation do not silently accept invalid values", () => {
  near(quickMmorpgFrameDelta(0.1), 1 / 30);
  assert.throws(() => quickMmorpgFrameDelta(NaN));
  assert.throws(() => quickMmorpgFrameDelta(-1));
  assert.throws(() => new QuickMmorpgCamera(0));
  assert.throws(
    () =>
      new QuickMmorpgLocomotion(
        clips().filter((clip) => clip.name !== "dance"),
      ),
  );
  assert.throws(() =>
    new QuickMmorpgLocomotion(clips()).advance(
      { ...released, forward: NaN },
      1 / 60,
    ),
  );
  const locomotion = new QuickMmorpgLocomotion(clips());
  locomotion.advance(released, 0);
  assert.equal(locomotion.time, 0);
});
