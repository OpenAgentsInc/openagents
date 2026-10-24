import assert from "node:assert/strict";
import test from "node:test";
import { FramePacer } from "../packages/runtime/frame-pacer.ts";

test("display timing is unchanged and unsupported clocks or profiles do not mutate a deadline", () => {
  const p = new FramePacer();
  p.configure("display");
  for (const now of [0, 0, 1, 8, 16, 10000]) assert(p.due(now));
  assert.throws(() => p.configure(120 as never));
  for (const now of [NaN, Infinity, -1, 9999]) assert.throws(() => p.due(now));
  assert(p.due(10001));
});

test("the production default uses each display opportunity", () => {
  const p = new FramePacer();
  let due = 0;
  for (let i = 0; i < 120; i++) due += Number(p.due(i * (1000 / 120)));
  assert.equal(due, 120);
});

test("60 Hz callbacks with early timestamp jitter never fall to half rate", () => {
  for (const offset of [0, 1000]) {
    const p = new FramePacer();
    p.configure(60);
    for (let i = 0; i < 1800; i++) {
      const now = offset + i * (1000 / 60) + (i % 2 ? -0.4 : 0.4);
      assert(p.due(now), `Callback ${i} at ${now} must be due.`);
    }
  }
  const p = new FramePacer();
  p.configure(60);
  let now = 0,
    due = 0;
  for (let i = 0; i < 600; i++) {
    due += Number(p.due(now));
    now += i % 2 ? 16.7 : 16.6;
  }
  assert(due >= 598 && due <= 600, `${due} render attempts at rounded 60 Hz.`);
});

test("120/144/240 Hz displays retain phase while requesting about 60 frames per second", () => {
  for (const displayHz of [120, 144, 240]) {
    const p = new FramePacer();
    p.configure(60);
    const due: number[] = [];
    for (let i = 0; i < displayHz * 10; i++) {
      const now = i * (1000 / displayHz);
      if (p.due(now)) due.push(now);
    }
    assert(
      Math.abs(due.length - 600) <= 1,
      `${displayHz} Hz selected ${due.length}.`,
    );
    for (let i = 1; i < due.length; i++) {
      assert(due[i] - due[i - 1] > 10);
      assert(due[i] - due[i - 1] <= 1000 / 60 + 1000 / displayHz + 0.001);
    }
  }
});

test("variable cadence and hidden gaps skip old slots without catch-up bursts", () => {
  const p = new FramePacer();
  p.configure(60);
  assert(p.due(0));
  assert(!p.due(8));
  assert(p.due(17));
  assert(!p.due(25));
  assert(p.due(34));
  assert(p.due(10000));
  assert(!p.due(10000.1));
  assert(!p.due(10008));
  assert(p.due(10017));
  assert(!p.due(10017));
  p.reset();
  assert(p.due(1));
  assert(!p.due(9));
  p.configure("display");
  assert(p.due(10));
  assert(p.due(11));
});

test("slow callbacks all remain due without concealing their actual timing", () => {
  const p = new FramePacer();
  p.configure(60);
  for (const now of [0, 33.3, 66.7, 99.9, 150, 184, 218]) assert(p.due(now));
});
