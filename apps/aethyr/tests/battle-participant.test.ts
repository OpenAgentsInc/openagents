import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BattleParticipant,
  validateBattleParticipant,
  type BattleParticipantSnapshot,
  type BattleParticipantCrowdContext,
} from "../packages/runtime/battle-participant.ts";
import type { BattleCrowdTerrain } from "../packages/renderer/battle-crowd.ts";

const terrain: BattleCrowdTerrain = {
  layout: "mmorpg-patch",
  resolution: 257,
  size: 3328,
  center: [-16, 32],
  heights: new Float32Array(257 ** 2),
};
const pose = (): BattleParticipantSnapshot => ({
  class: "paladin",
  position: [0, 0, 0],
  eye: [0, 1.62, 0],
  yaw: 0,
  pitch: 0,
  height: 1.8,
  grounded: true,
  velocity: [0, 0, 0],
  speed: 0,
  state: "idle",
  actionTime: 0,
  animationTime: 0,
  alive: true,
  attackSequence: 0,
  tick: 0,
  droppedSeconds: 0,
});
const spawn = {
  class: "paladin" as const,
  position: [0, 999, 0] as [number, number, number],
  yaw: 0,
};
const input = {
  forward: 1,
  turn: 0,
  sprint: false,
  attack: false,
  dance: false,
};
const crowd = (): BattleParticipantCrowdContext => ({
  expectedEpoch: 2,
  currentTick: 120,
  ageSeconds: 1 / 60,
  sample: {
    epoch: 2,
    tick: 119,
    position: [0, 0, 0],
    contacts: [{ id: 7, position: [0, 0, -3], radius: 1.33, velocity: [0, 1] }],
    overflow: 0,
  },
});

test("battle controller sends only isolated requests and copies snapshots", () => {
  const messages: Record<string, any>[] = [];
  const original = pose();
  const kernel = {
    request<T>(request: object): T {
      messages.push(request);
      return { ok: true, participant: original } as T;
    },
  };
  const value = new BattleParticipant(kernel);
  assert.equal(value.mode.kind, "inactive");
  assert.equal(value.snapshot, undefined);
  value.enter(terrain, spawn);
  value.advance(1 / 60, input, true, 0.1, -0.2, crowd());
  value.respawn({ position: [120, 0, 400], yaw: 1 });
  const exposed = value.snapshot!;
  exposed.position[0] = 90;
  assert.equal(value.snapshot!.position[0], 0);
  value.clear();
  assert.equal(value.mode.kind, "inactive");
  assert.deepEqual(
    messages.map((m) => m.type),
    [
      "battle_participant_init",
      "battle_participant_step",
      "battle_participant_respawn",
      "battle_participant_clear",
    ],
  );
  assert.equal(messages[0].terrain.heights.length, 257 ** 2);
  assert.equal(messages[1].pitchDelta, -0.2);
  assert.equal(messages[1].yawDelta, 0.1);
  assert.deepEqual(messages[1].crowd, crowd());
  assert(
    !messages.some((m) => "health" in m || "player" in m || "revision" in m),
  );
});

test("bounded crowd samples pass through without TypeScript physics and malformed data stays outside Wasm", () => {
  const messages: Record<string, any>[] = [];
  const kernel = {
    request<T>(message: object): T {
      messages.push(message);
      return { ok: true, participant: pose() } as T;
    },
  };
  const value = new BattleParticipant(kernel);
  value.enter(terrain, spawn);
  for (const mutate of [
    (frame: BattleParticipantCrowdContext) => {
      frame.ageSeconds = NaN;
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.expectedEpoch = -1;
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.currentTick = 0x1_0000_0000;
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.overflow = -1;
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.contacts = [
        { ...frame.sample!.contacts[0], velocity: [NaN, 0] },
      ];
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.contacts = Array.from({ length: 65 }, (_, id) => ({
        ...frame.sample!.contacts[0],
        id,
      }));
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.contacts = [
        ...frame.sample!.contacts,
        ...frame.sample!.contacts,
      ];
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.contacts[0].position[0] = Infinity;
    },
    (frame: BattleParticipantCrowdContext) => {
      frame.sample!.contacts = [{ ...frame.sample!.contacts[0], radius: -1 }];
    },
  ]) {
    const frame = crowd();
    mutate(frame);
    assert.throws(
      () => value.advance(1 / 60, input, true, 0, 0, frame),
      /complete nearby battle contacts/,
    );
  }
  assert.equal(messages.length, 1);
  const oldLife = crowd();
  oldLife.expectedEpoch = 3;
  value.advance(1 / 60, input, true, 0, 0, oldLife);
  assert.deepEqual(
    messages.at(-1)!.crowd,
    oldLife,
    "Rust decides whether an old-life sample can move the player",
  );
  value.advance(1 / 60, input, true);
  assert.equal(
    messages.at(-1)!.crowd,
    null,
    "Missing samples halt translation in Rust while actions still step",
  );
});

test("battle controller rejects unsupported inputs before Wasm and retains last valid pose on failure", () => {
  let calls = 0;
  let reject = false;
  const kernel = {
    request<T>(): T {
      calls++;
      return (
        reject
          ? { ok: false, message: "Choose another battle position." }
          : { ok: true, participant: pose() }
      ) as T;
    },
  };
  const value = new BattleParticipant(kernel);
  assert.throws(() => value.advance(1 / 60, input, true), /Enter the battle/);
  assert.equal(calls, 0);
  assert.throws(
    () => value.enter({ ...terrain, center: [1, 0] }, spawn),
    /displayed terrain/,
  );
  assert.equal(calls, 0);
  value.enter(terrain, spawn);
  assert.throws(
    () => value.advance(1 / 60, { ...input, turn: 2 }, true),
    /supported battle movement/,
  );
  assert.throws(
    () => value.advance(1 / 60, input, true, 0, NaN),
    /supported battle movement/,
  );
  assert.equal(calls, 1);
  reject = true;
  const before = value.snapshot;
  assert.throws(
    () => value.respawn({ position: [0, 0, 0], yaw: 0 }),
    /another battle position/,
  );
  assert.deepEqual(value.snapshot, before);
});

test("battle snapshot validation requires finite physical poses and coherent alive state", () => {
  for (const patch of [
    { position: [NaN, 0, 0] },
    { eye: [0, 9, 0] },
    { state: "death", alive: true },
    { tick: -1 },
    { attackSequence: 0x1_0000_0000 },
    { pitch: Infinity },
    { height: 0 },
  ])
    assert.throws(
      () => validateBattleParticipant({ ...pose(), ...patch }),
      /unsupported player pose/,
    );
  assert.equal(
    validateBattleParticipant({ ...pose(), alive: false, state: "death" })
      .alive,
    false,
  );
});
