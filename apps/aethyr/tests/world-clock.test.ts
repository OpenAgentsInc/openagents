import assert from "node:assert/strict";
import test from "node:test";
import {
  MAX_WORLD_ANIMATION_TIME,
  validWorldAnimationTime,
  WorldClock,
} from "../packages/runtime/world-clock.ts";

test("loading time does not age a new world", () => {
  const clock = new WorldClock();
  assert.equal(clock.sample(600000), 0);
  assert.equal(clock.start(600100), 0);
  assert.equal(clock.sample(600116), 0.016);
  assert.equal(clock.sample(601100), 1);
});

test("repeated starts, duplicate samples and reversed timestamps never reset or rewind", () => {
  const clock = new WorldClock();
  clock.start(1000);
  assert.equal(clock.sample(2000), 1);
  assert.equal(clock.sample(2000), 1);
  assert.equal(clock.sample(1500), 1);
  assert.equal(clock.start(2500), 1.5);
  assert.equal(clock.sample(3000), 2);
  for (const invalid of [NaN, Infinity, -1])
    assert.throws(() => clock.sample(invalid), /timestamps/);
  assert.equal(clock.time, 2);
});

test("checkpoint restoration anchors after a fresh load without counting its duration", () => {
  const clock = new WorldClock(17.25);
  assert.equal(clock.sample(1000000), 17.25);
  assert.equal(clock.start(2000000), 17.25);
  assert.equal(clock.sample(2001000), 18.25);
  const restored = new WorldClock(clock.time);
  assert.equal(restored.start(9000000), 18.25);
  assert.equal(restored.sample(9000250), 18.5);
});

test("the session clock has an explicit bound separate from the 24-hour capture limit", () => {
  const clock = new WorldClock(90000);
  clock.start(1000);
  assert.equal(clock.time, 90000);
  assert.equal(clock.sample(2500), 90001.5);
  assert.ok(validWorldAnimationTime(86401));
  assert.ok(validWorldAnimationTime(MAX_WORLD_ANIMATION_TIME));
  for (const invalid of [NaN, Infinity, -1, MAX_WORLD_ANIMATION_TIME + 1])
    assert.throws(() => new WorldClock(invalid), /limit/);
  const bounded = new WorldClock(MAX_WORLD_ANIMATION_TIME);
  bounded.start(0);
  assert.throws(() => bounded.sample(1000), /exceeded/);
  assert.equal(bounded.time, MAX_WORLD_ANIMATION_TIME);
});
