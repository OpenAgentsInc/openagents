import assert from "node:assert/strict";
import test from "node:test";
import { FrameCompletions } from "../packages/renderer/frame-completions.ts";

test("completion windows accept late fences after stop and isolate later recordings", () => {
  const frames = new FrameCompletions();
  assert.equal(frames.submitted(1, 0), undefined);
  frames.start(2);
  const a = frames.submitted(2, 5)!;
  const b = frames.submitted(3, 8)!;
  frames.stop();
  assert.equal(frames.submitted(4, 10), undefined);
  b(15);
  a(17);
  a(18);
  assert.deepEqual(frames.snapshot(), {
    completionRange: { firstFrame: 2, lastFrame: 3 },
    completedFrames: [
      { frame: 3, submittedAt: 8, completedAt: 15, latencyMs: 7, ok: true },
      { frame: 2, submittedAt: 5, completedAt: 17, latencyMs: 12, ok: true },
    ],
    completionTruncated: false,
  });
  frames.start();
  b(19);
  frames.submitted(5, 20)!(23, false);
  assert.deepEqual(
    frames.snapshot().completedFrames.map((e) => [e.frame, e.ok]),
    [[5, false]],
  );
});

test("completion storage is bounded and returned observations cannot mutate it", () => {
  const frames = new FrameCompletions();
  frames.start(1);
  frames.submitted(1, 0)!(1);
  assert.equal(frames.submitted(2, 1), undefined);
  const snapshot = frames.snapshot();
  assert.equal(snapshot.completionTruncated, true);
  snapshot.completedFrames[0].frame = 99;
  assert.equal(frames.snapshot().completedFrames[0].frame, 1);
  for (const invalid of [0, 20_001, Infinity, 1.5])
    assert.throws(() => frames.start(invalid));
});

test("post-submit observations include synchronous submission cost without changing the historical origin", () => {
  const frames = new FrameCompletions();
  frames.start();
  const complete = frames.submitted(7, 10, 14)!;
  frames.stop();
  complete(20);
  const observation = frames.snapshot().completedFrames[0];
  assert.equal(observation.submittedAt, 10);
  assert.equal(observation.submitReturnedAt, 14);
  assert.equal(observation.latencyMs, 10);
  observation.submitReturnedAt = 999;
  assert.equal(frames.snapshot().completedFrames[0].submitReturnedAt, 14);
});
