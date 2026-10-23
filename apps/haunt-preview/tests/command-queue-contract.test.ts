import assert from "node:assert/strict";
import { test } from "node:test";
import { CommandQueue } from "../src/command-queue.ts";

test("commands execute in order with one active job and distinct results", async () => {
  const queue = new CommandQueue();
  const starts: number[] = [];
  let active = 0, peak = 0;
  const jobs = Array.from({ length: 8 }, (_, index) => queue.run(async () => {
    starts.push(index);
    active++;
    peak = Math.max(peak, active);
    await Promise.resolve();
    active--;
    return `result-${index}`;
  }));
  assert.deepEqual(await Promise.all(jobs), Array.from({ length: 8 }, (_, i) => `result-${i}`));
  assert.deepEqual(starts, [0, 1, 2, 3, 4, 5, 6, 7]);
  assert.equal(peak, 1);
});

test("rejections and synchronous throws reach their caller without blocking later jobs", async () => {
  const queue = new CommandQueue();
  const rejected = queue.run(async () => { throw new Error("async failure"); });
  const thrown = queue.run(() => { throw new Error("sync failure"); });
  const next = queue.run(async () => 7);
  await assert.rejects(rejected, /async failure/);
  await assert.rejects(thrown, /sync failure/);
  assert.equal(await next, 7);
  assert.equal(await queue.run(async () => 8), 8);
});

test("32 running or waiting commands fill the queue and settled jobs release capacity", { timeout: 2000 }, async () => {
  const queue = new CommandQueue();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  const jobs = [queue.run(async () => { await gate; return 0; })];
  for (let i = 1; i < 32; i++) jobs.push(queue.run(async () => i));
  let extraRan = false;
  const overflow = queue.run(async () => { extraRan = true; return 33; });
  const rejected = overflow.then(() => false, () => true);
  try {
    const outcome = await Promise.race([
      rejected,
      new Promise<false>((resolve) => setTimeout(() => resolve(false), 100)),
    ]);
    assert.equal(outcome, true, "An extra command must be rejected while the queue is full");
  } finally {
    release();
    await Promise.all(jobs);
  }
  assert.equal(extraRan, false);
  assert.equal(await queue.run(async () => 42), 42);
});
