import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  BattleFollowCamera,
  battleCamera,
  BATTLE_ORBIT_RANGE,
} from "../packages/runtime/battle-view.ts";
import { BattleParticipant } from "../packages/runtime/battle-participant.ts";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";
import { mmorpgHeight } from "../packages/runtime/mmorpg-terrain.ts";

const actor = {
  position: [0, 100, 0] as [number, number, number],
  yaw: 0,
  pitch: 0,
  height: 1.8,
};

test("third-person orbit and zoom are independent of the unchanged body pose", () => {
  const camera = new BattleFollowCamera();
  camera.reset(actor);
  const before = structuredClone(actor);
  const start = battleCamera([0, 0, 0], 100, "third", 0, actor, camera.orbit);
  camera.look(Math.PI / 2, 0.4);
  camera.zoom(12);
  const orbited = battleCamera([0, 0, 0], 100, "third", 0, actor, camera.orbit);
  assert.deepEqual(actor, before);
  assert(orbited.eye[0] < -11);
  assert(Math.abs(orbited.eye[2]) < 1);
  assert(orbited.pitch > start.pitch + 0.35);
  const turnedBody = battleCamera(
    [0, 0, 0],
    100,
    "third",
    0,
    { ...actor, yaw: 2 },
    camera.orbit,
  );
  assert.deepEqual(
    orbited,
    turnedBody,
    "A/D body heading does not drag the orbit around",
  );
  const copy = camera.orbit;
  copy.distance = 999;
  assert.equal(camera.orbit.distance, 12);
});

test("zoom and vertical orbit stay bounded and clear terrain at every distance", () => {
  const camera = new BattleFollowCamera();
  camera.reset(actor);
  camera.look(1000, 1000);
  assert.equal(camera.orbit.pitch, 1.25);
  for (const value of [-100, ...BATTLE_ORBIT_RANGE, 100]) {
    camera.zoom(value);
    assert(camera.orbit.distance >= 2.5 && camera.orbit.distance <= 18);
    const feet = [0, mmorpgHeight(0, 0), 0] as [number, number, number];
    const view = battleCamera(
      [0, 0, 0],
      100,
      "third",
      0,
      { ...actor, position: feet },
      camera.orbit,
    );
    assert(view.eye[1] >= mmorpgHeight(view.eye[0], view.eye[2]) + 0.4);
  }
  const finite = camera.orbit;
  camera.look(NaN, Infinity);
  assert.deepEqual(camera.orbit, finite);
  assert.throws(() => camera.zoom(NaN));
});

test("third-person clicks turn toward aim at four radians/second before striking and cancel safely", () => {
  const camera = new BattleFollowCamera();
  camera.reset(actor);
  camera.look(Math.PI, 0);
  let yaw = 0,
    releasedClickFired = false;
  for (let frame = 0; frame < 60; frame++) {
    const result = camera.aimAttack(
      yaw,
      1 / 60,
      frame === 0,
      (frame * 1000) / 60,
    );
    assert(Math.abs(result.yawDelta) <= 4 / 60 + 1e-9);
    yaw += result.yawDelta;
    if (result.attack) {
      assert(
        Math.abs(
          Math.atan2(
            Math.sin(camera.orbit.yaw - yaw),
            Math.cos(camera.orbit.yaw - yaw),
          ),
        ) <= 0.075,
      );
      releasedClickFired = true;
      break;
    }
  }
  assert(
    releasedClickFired,
    "one click survives the bounded turn, without requiring a held button",
  );
  camera.cancelAttack();
  assert.deepEqual(camera.aimAttack(yaw, 1 / 60, false, 1001), {
    yawDelta: 0,
    attack: false,
  });
  camera.aimAttack(yaw, 1 / 60, true, 1100);
  assert.deepEqual(camera.aimAttack(yaw, 1 / 60, false, 2400), {
    yawDelta: 0,
    attack: false,
  });
  camera.reset({ yaw: 0.4, pitch: 0.1 });
  assert.equal(camera.orbit.distance, 6);
  assert.deepEqual(camera.aimAttack(0.4, 1 / 60, false, 2500), {
    yawDelta: 0,
    attack: false,
  });
});

test("first person keeps the controller's exact eye, mouse look and heading", () => {
  const orbit = new BattleFollowCamera();
  orbit.reset(actor);
  orbit.look(2, 0.5);
  orbit.zoom(18);
  const body = {
    ...actor,
    eye: [0, 101.62, 0] as [number, number, number],
    yaw: -0.7,
    pitch: 0.3,
  };
  orbit.followFirst(body);
  const first = battleCamera([0, 0, 0], 100, "first", 0, body, orbit.orbit);
  assert.deepEqual(first.eye, body.eye);
  assert.equal(first.yaw, body.yaw);
  assert.equal(first.pitch, body.pitch);
  const third = battleCamera([0, 0, 0], 100, "third", 0, body, orbit.orbit);
  assert(Math.abs(third.pitch - first.pitch) < 0.01);
});

test("actual Wasm body stays still during orbit, preserves W/A controls and turns before one attack", async () => {
  const kernel = await RuntimeKernel.fromBytes(
    await readFile(new URL("../public/kernel.wasm", import.meta.url)),
  );
  assert.equal(
    kernel.request<any>({
      type: "init",
      entities: [],
      spawn: { position: [0, 0, 0], yaw: 0, pitch: 0, mode: "walk" },
    }).ok,
    true,
  );
  const participant = new BattleParticipant(kernel);
  participant.enter(
    {
      layout: "mmorpg-patch",
      center: [0, 0],
      size: 3328,
      resolution: 257,
      heights: new Float32Array(257 ** 2),
    },
    { class: "paladin", position: [0, 0, 0], yaw: 0 },
  );
  const camera = new BattleFollowCamera();
  camera.reset(participant.snapshot!);
  const original = participant.snapshot!;
  camera.look(2, 0.3);
  camera.zoom(10);
  assert.deepEqual(participant.snapshot, original);
  const neutral = {
    forward: 0,
    turn: 0,
    sprint: false,
    attack: false,
    dance: false,
  };
  const step = (input = neutral, yawDelta = 0) => {
    const current = participant.snapshot!;
    return participant.advance(1 / 60, input, true, yawDelta, 0, {
      expectedEpoch: 1,
      currentTick: current.tick,
      ageSeconds: 0,
      sample: {
        epoch: 1,
        tick: current.tick,
        position: current.position,
        contacts: [],
        overflow: 0,
      },
    });
  };
  for (let i = 0; i < 60; i++) step({ ...neutral, forward: 1 });
  assert(
    participant.snapshot!.position[2] < -1,
    "W still follows the body's heading, not orbit yaw",
  );
  const beforeTurn = participant.snapshot!.yaw;
  for (let i = 0; i < 30; i++) step({ ...neutral, turn: 1 });
  assert(
    participant.snapshot!.yaw > beforeTurn + 0.6,
    "A/D still turns the source body controller",
  );
  assert.equal(camera.orbit.yaw, 2);
  let entered = false;
  for (let i = 0; i < 90; i++) {
    const pose = participant.snapshot!;
    const intent = camera.aimAttack(pose.yaw, 1 / 60, i === 0, (i * 1000) / 60);
    const next = step({ ...neutral, attack: intent.attack }, intent.yawDelta);
    if (next.state === "attack") {
      entered = true;
      break;
    }
    assert.equal(
      next.attackSequence,
      0,
      "no strike is accepted before the body faces the camera",
    );
  }
  assert(entered);
  assert.equal(participant.snapshot!.attackSequence, 1);
});
