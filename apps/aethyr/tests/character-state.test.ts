import test from "node:test";
import assert from "node:assert/strict";
import {
  CharacterLocomotion,
  characterAnimation,
  type CharacterPlayer,
} from "../packages/runtime/character-state.ts";
import { CHARACTER_MOTIONS } from "../packages/renderer/character.ts";
import { direction, modelMatrix } from "../packages/renderer/math.ts";

const player = (patch: Partial<CharacterPlayer> = {}): CharacterPlayer => ({
  position: [0, 0, 0],
  eye: [0, 1.62, 0],
  yaw: 0,
  pitch: 0,
  grounded: true,
  velocity: [0, 0, 0],
  speed: 0,
  ahead: 0,
  side: 0,
  ...patch,
});
const clips = CHARACTER_MOTIONS.map((name) => ({
  name,
  duration: 1,
  distance: name === "idle" || name === "jump" ? 0 : 2,
  tracks: [],
}));
test("admitted motion chooses grounded and airborne animation", () => {
  assert.equal(characterAnimation(player({ ahead: 1 })), "idle");
  assert.equal(characterAnimation(player({ speed: 6.4 })), "run");
  assert.equal(
    characterAnimation(player({ grounded: false, velocity: [0, 2, 0] })),
    "jump",
  );
  assert.equal(
    characterAnimation(player({ grounded: false, velocity: [0, -2, 0] })),
    "fall",
  );
});
test("gait follows actual distance and ignores a teleport", () => {
  const motion = new CharacterLocomotion(clips);
  motion.update(player({ speed: 6.4, ahead: 1 }), 0.1, true);
  const moving = motion.update(
    player({ position: [0, 0, -1], speed: 6.4, ahead: 1 }),
    0.1,
    true,
  );
  assert.equal(moving.time, 0.5);
  assert.equal(
    motion.update(
      player({ position: [100, 0, 100], speed: 6.4, ahead: 1 }),
      0.1,
      true,
    ).time,
    0.5,
  );
  assert.equal(
    motion.update(player({ position: [100, 0, 100], ahead: 1 }), 0.1, true)
      .animation,
    "idle",
  );
});
test("source +Z rig faces the Aethyrforge movement direction", () => {
  const motion = new CharacterLocomotion(clips);
  for (const yaw of [0, 0.5, -2, Math.PI]) {
    const pose = motion.update(player({ yaw }), 0.1, true),
      matrix = modelMatrix(pose.position, pose.yaw);
    const forward = direction(yaw, 0);
    assert.ok(
      Math.hypot(matrix[8] - forward[0], matrix[10] - forward[2]) < 1e-6,
    );
  }
});
test("directional clips and airborne last pose do not loop in midair", () => {
  const motion = new CharacterLocomotion(clips);
  assert.equal(
    motion.update(player({ speed: 4, ahead: -1 }), 0.1, true).animation,
    "backpedal",
  );
  assert.equal(
    motion.update(player({ speed: 4, side: -1 }), 0.1, true).animation,
    "strafe_left",
  );
  for (let i = 0; i < 20; i++)
    motion.update(player({ grounded: false }), 0.1, false);
  assert.ok(motion.update(player({ grounded: false }), 0.1, false).time < 1);
});
