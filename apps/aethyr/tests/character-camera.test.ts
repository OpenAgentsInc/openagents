import assert from "node:assert/strict";
import test from "node:test";
import { CharacterCamera } from "../packages/renderer/character-camera.ts";
import type { V3 } from "../packages/renderer/math.ts";

const feet: V3 = [0, 0, 0];
const open = () => 1;

test("the orbit stays behind the -Z facing character and uses a downward initial pitch", () => {
  const camera = new CharacterCamera();
  const frame = camera.frame(feet, 0, open, 0, 1 / 60);
  assert.ok(frame.eye[2] > 5);
  assert.ok(frame.eye[1] > 3);
  assert.ok(frame.direction[2] < 0);
  assert.ok(frame.direction[1] < 0);
  assert.equal(frame.hideAvatar, false);
  assert.equal(frame.orbitDistance, 6);
});

test("free orbit and mouselook keep character facing and camera offset separate", () => {
  const camera = new CharacterCamera();
  camera.orbit(100, 20);
  assert.ok(Math.abs(camera.yawOffset - 0.4) < 1e-12);
  assert.ok(Math.abs(camera.pitch + 0.36) < 1e-12);
  assert.equal(camera.mouselook(25, -10), 0.1);
  assert.ok(Math.abs(camera.takeOffset() - 0.4) < 1e-12);
  assert.equal(camera.yawOffset, 0);
  camera.orbit(100, 0);
  camera.settle(1);
  assert.ok(Math.abs(camera.yawOffset - 0.008) < 1e-12);
});

test("first person glides into the head, hides the druid, and returns without moving the player", () => {
  const camera = new CharacterCamera();
  camera.setPerspective("first");
  let frame = camera.frame(feet, 0, open, 0, 0.125);
  assert.equal(frame.blend, 0.5);
  assert.equal(frame.hideAvatar, false);
  frame = camera.frame(feet, 0, open, 0, 0.125);
  assert.equal(frame.blend, 1);
  assert.deepEqual(frame.eye, [0, 1.7, 0]);
  assert.equal(frame.hideAvatar, true);
  assert.equal(frame.near, 0.05);
  camera.setPerspective("third");
  frame = camera.frame(feet, 0, open, 0, 0.25);
  assert.equal(frame.blend, 0);
  assert.ok(frame.eye[2] > 5);
  assert.deepEqual(feet, [0, 0, 0]);
});

test("zoom requires deliberate overshoot and reversal before changing perspective", () => {
  const camera = new CharacterCamera();
  camera.setOrbitDistance(2.5);
  camera.zoomBy(1.05);
  assert.equal(camera.perspective, "third");
  camera.zoomBy(1 / 1.01);
  camera.zoomBy(1.05);
  assert.equal(camera.perspective, "third");
  camera.zoomBy(1.2);
  assert.equal(camera.perspective, "first");
  camera.zoomBy(1 / 1.05);
  assert.equal(camera.perspective, "first");
  camera.zoomBy(1 / 1.2);
  assert.equal(camera.perspective, "third");
  assert.equal(camera.orbitDistance, 2.5);
  camera.setOrbitDistance(200);
  assert.equal(camera.orbitDistance, 40);
});

test("the first collision frame pulls in immediately, then eases out and resets after teleport", () => {
  const camera = new CharacterCamera();
  let frame = camera.frame(
    feet,
    0,
    (a, b) => (b[2] === a[2] ? 1 : 0.1),
    0,
    1 / 60,
  );
  assert.equal(frame.limited, true);
  assert.equal(frame.hideAvatar, true);
  assert.equal(frame.near, 0.05);
  const blockedZ = frame.eye[2];
  frame = camera.frame(feet, 0, open, 0, 1 / 60);
  assert.equal(frame.limited, true);
  assert.ok(frame.eye[2] > blockedZ && frame.eye[2] < 1.2);
  frame = camera.frame([10, 0, 0], 0, open, 0, 1 / 60);
  assert.equal(frame.limited, false);
  assert.ok(frame.eye[2] > 5);
});

test("ground clearance keeps the eye above the world without changing its look angle", () => {
  const camera = new CharacterCamera();
  camera.pitch = 1.4;
  const frame = camera.frame(feet, 0, open, 0, 1 / 60);
  assert.equal(frame.eye[1], 0.4);
  assert.ok(frame.direction[1] > 0.9);
  camera.pitch = -0.28;
  const limited = camera.frame(feet, 0, () => NaN, 0, 1 / 60);
  assert.equal(limited.limited, true);
  assert.equal(limited.hideAvatar, true);
});
