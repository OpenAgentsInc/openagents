import test from "node:test";
import assert from "node:assert/strict";
import {
  CpuCullExperiment,
  CpuCullError,
  type CpuCullWorker,
  type CpuCullWorkerReply,
  type CpuCullWorkerRequest,
} from "../packages/runtime/cpu-cull.ts";
import { createCpuCullWorkerHandler } from "../packages/runtime/cpu-cull.worker.ts";
import {
  frustumVisibility,
  type InstanceBounds,
} from "../packages/renderer/culling.ts";

const matrix = () =>
  new Float64Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
function bounds(count = 11): InstanceBounds {
  const spheres = new Float64Array(count * 4),
    aabbs = new Float64Array(count * 6);
  for (let i = 0; i < count; i++) {
    const x = i % 2 ? 3 : 0;
    spheres.set([x, 0, 0.5, 0.1], i * 4);
    aabbs.set([x - 0.1, -0.1, 0.4, x + 0.1, 0.1, 0.6], i * 6);
  }
  return { count, spheres, aabbs };
}
type Fault =
  | "hang-install"
  | "hang-run"
  | "error-run"
  | "bad-mask"
  | "stale"
  | "duplicate";
class TestWorker implements CpuCullWorker {
  readonly messages: CpuCullWorkerRequest[] = [];
  readonly replies: CpuCullWorkerReply[] = [];
  readonly listeners = new Map<string, Set<EventListener>>();
  readonly handler = createCpuCullWorkerHandler();
  terminated = false;
  constructor(public fault?: Fault) {}
  addEventListener(type: string, listener: EventListener) {
    (
      this.listeners.get(type) ?? this.listeners.set(type, new Set()).get(type)!
    ).add(listener);
  }
  removeEventListener(type: string, listener: EventListener) {
    this.listeners.get(type)?.delete(listener);
  }
  terminate() {
    this.terminated = true;
  }
  emit(data: unknown) {
    for (const listener of this.listeners.get("message") ?? [])
      listener(new MessageEvent("message", { data }));
  }
  postMessage(message: unknown, transfer: Transferable[]) {
    // Actual transfer semantics catch accidental detachment of the caller's data.
    const incoming = structuredClone(message, {
      transfer,
    }) as CpuCullWorkerRequest;
    this.messages.push(incoming);
    queueMicrotask(() => {
      if (
        this.terminated ||
        (this.fault === "hang-install" && incoming.type === "install") ||
        (this.fault === "hang-run" && incoming.type === "run")
      )
        return;
      if (this.fault === "error-run" && incoming.type === "run") {
        for (const listener of this.listeners.get("error") ?? [])
          listener(new Event("error"));
        return;
      }
      const processed = this.handler(incoming);
      const reply = structuredClone(processed.reply, {
        transfer: processed.transfer,
      });
      if (reply.type === "result") {
        if (this.fault === "bad-mask" && reply.count)
          new Uint8Array(reply.mask)[0] = 2;
        if (this.fault === "stale")
          this.emit({ ...reply, epoch: reply.epoch - 1 });
      }
      this.replies.push(reply);
      this.emit(reply);
      if (this.fault === "duplicate") this.emit(reply);
    });
  }
}
function setup(workers: 0 | 1 | 2, fault?: Fault) {
  const ports: TestWorker[] = [];
  const probe = new CpuCullExperiment({
    workers,
    workerFactory: () => {
      const port = new TestWorker(fault);
      ports.push(port);
      return port;
    },
  });
  return { probe, ports };
}
const code = (expected: string) => (error: unknown) =>
  error instanceof CpuCullError && error.code === expected;

test("serial, one helper, and two resident helpers execute identical production frustum masks", async () => {
  for (const count of [0, 1, 11, 1000]) {
    const source = bounds(count),
      view = matrix(),
      expected = frustumVisibility(source, view);
    for (const workers of [0, 1, 2] as const) {
      const { probe, ports } = setup(workers);
      const installed = await probe.install({
        datasetId: "owned",
        generation: 4,
        bounds: source,
      });
      assert.equal(installed.workers, workers);
      assert.equal(installed.bytes.resident, count * 80 * (workers ? 2 : 1));
      assert.equal(installed.bytes.copied, count * 80 * (workers ? 2 : 1));
      assert.equal(source.spheres.byteLength, count * 32);
      const result = await probe.run({ matrix: view });
      assert.deepEqual(result.mask, expected);
      assert.equal(result.sequence, 1);
      assert.equal(result.datasetId, "owned");
      assert.equal(result.generation, 4);
      for (const timing of Object.values(result.timings))
        assert.ok(Number.isFinite(timing) && timing >= 0);
      assert.equal(result.bytes.transferredOut, workers ? count : 0);
      assert.equal(ports.length, workers);
      assert.ok(
        ports.every((port) =>
          port.replies.some(
            (reply) => reply.type === "result" && reply.taskRuns === 1,
          ),
        ),
      );
      if (workers) {
        const serial = await probe.run({ matrix: view, mode: "serial" });
        assert.deepEqual(serial.mask, result.mask);
        assert.equal(serial.sequence, 2);
      }
      probe.dispose();
      assert.ok(
        ports.every(
          (port) =>
            port.terminated &&
            [...port.listeners.values()].every((listeners) => !listeners.size),
        ),
      );
      assert.equal(probe.snapshot().residentBytes, 0);
    }
  }
});

test("installed copies survive source mutation and camera changes never reuse an old mask", async () => {
  const { probe } = setup(2),
    source = bounds();
  const expected = frustumVisibility(source, matrix());
  await probe.install({ datasetId: "owned", generation: 1, bounds: source });
  source.spheres.fill(NaN);
  source.aabbs.fill(NaN);
  assert.deepEqual((await probe.run({ matrix: matrix() })).mask, expected);
  const moved = matrix();
  moved[12] = -3;
  const shifted = await probe.run({ matrix: moved });
  assert.notDeepEqual(shifted.mask, expected);
  probe.dispose();
});

test("invalid input, shared memory, stale sequences, and unsupported helper counts fail before work", async () => {
  assert.throws(() => new CpuCullExperiment({ workers: 3 as never }));
  const { probe, ports } = setup(1);
  await assert.rejects(probe.run({ matrix: matrix() }), code("not-installed"));
  const invalid = bounds();
  invalid.aabbs[3] = -1;
  await assert.rejects(
    probe.install({ datasetId: "bad", generation: 0, bounds: invalid }),
    code("invalid-input"),
  );
  const shared = bounds();
  shared.spheres = new Float64Array(new SharedArrayBuffer(shared.count * 32));
  await assert.rejects(
    probe.install({ datasetId: "bad", generation: 0, bounds: shared }),
    code("invalid-input"),
  );
  assert.equal(ports.length, 0);
  await probe.install({ datasetId: "valid", generation: 0, bounds: bounds() });
  await assert.rejects(
    probe.run({ matrix: new Float64Array(16) }),
    code("invalid-input"),
  );
  await probe.run({ matrix: matrix(), sequence: 10 });
  await assert.rejects(
    probe.run({ matrix: matrix(), sequence: 10 }),
    code("invalid-input"),
  );
  await assert.rejects(
    probe.run({ matrix: matrix(), timeoutMs: 0 }),
    code("invalid-input"),
  );
  probe.dispose();
});

test("one active job rejects queues and abort terminates every helper without publishing a mask", async () => {
  const { probe, ports } = setup(2);
  await probe.install({ datasetId: "owned", generation: 0, bounds: bounds() });
  ports.forEach((port) => {
    port.fault = "hang-run";
  });
  const abort = new AbortController();
  const pending = probe.run({ matrix: matrix(), signal: abort.signal });
  const rejected = assert.rejects(pending, code("aborted"));
  await assert.rejects(probe.run({ matrix: matrix() }), code("busy"));
  abort.abort();
  await rejected;
  assert.ok(ports.every((port) => port.terminated));
  assert.equal(probe.snapshot().active, false);
  assert.equal(probe.snapshot().workers, 0);
  assert.equal(probe.snapshot().residentBytes, bounds().count * 80);
  assert.equal(
    (await probe.run({ matrix: matrix(), mode: "serial" })).mask.length,
    bounds().count,
  );
  probe.dispose();
});

test("installation and running timeouts terminate stalled workers and clear their listeners", async () => {
  const stalled = setup(2, "hang-install");
  await assert.rejects(
    stalled.probe.install(
      { datasetId: "owned", generation: 0, bounds: bounds() },
      { timeoutMs: 15 },
    ),
    code("timeout"),
  );
  assert.ok(stalled.ports.every((port) => port.terminated));
  stalled.probe.dispose();
  const running = setup(2);
  await running.probe.install({
    datasetId: "owned",
    generation: 0,
    bounds: bounds(),
  });
  running.ports[1].fault = "hang-run";
  await assert.rejects(
    running.probe.run({ matrix: matrix(), timeoutMs: 15 }),
    code("timeout"),
  );
  assert.ok(running.ports.every((port) => port.terminated));
  assert.equal(running.probe.snapshot().workers, 0);
  running.probe.dispose();
});

test("new installation supersedes active work and old generations cannot deliver a result", async () => {
  const { probe, ports } = setup(1);
  await probe.install({ datasetId: "old", generation: 1, bounds: bounds(4) });
  ports[0].fault = "hang-run";
  const pending = probe.run({ matrix: matrix() });
  const rejected = assert.rejects(pending, code("superseded"));
  await probe.install({ datasetId: "new", generation: 2, bounds: bounds(7) });
  await rejected;
  assert.equal(ports[0].terminated, true);
  const result = await probe.run({ matrix: matrix() });
  assert.equal(result.datasetId, "new");
  assert.equal(result.generation, 2);
  assert.equal(result.mask.length, 7);
  probe.dispose();
});

test("malformed results and helper failures terminate the pool; stale and duplicate replies are ignored", async () => {
  for (const fault of ["bad-mask", "error-run"] as const) {
    const { probe, ports } = setup(2);
    await probe.install({
      datasetId: "owned",
      generation: 1,
      bounds: bounds(),
    });
    ports[0].fault = fault;
    await assert.rejects(
      probe.run({ matrix: matrix() }),
      code(fault === "bad-mask" ? "invalid-result" : "worker-failed"),
    );
    assert.ok(ports.every((port) => port.terminated));
    probe.dispose();
  }
  for (const fault of ["stale", "duplicate"] as const) {
    const { probe } = setup(2, fault);
    await probe.install({
      datasetId: "owned",
      generation: 1,
      bounds: bounds(),
    });
    assert.deepEqual(
      (await probe.run({ matrix: matrix() })).mask,
      frustumVisibility(bounds(), matrix()),
    );
    assert.ok(probe.snapshot().ignoredReplies > 0);
    probe.dispose();
  }
});

test("shutdown rejects pending operations, closes listeners, and makes later use explicit", async () => {
  const { probe, ports } = setup(2);
  await probe.install({ datasetId: "owned", generation: 1, bounds: bounds() });
  ports.forEach((port) => {
    port.fault = "hang-run";
  });
  const pending = probe.run({ matrix: matrix() });
  const rejected = assert.rejects(pending, code("disposed"));
  probe.dispose();
  await rejected;
  probe.dispose();
  const snapshot = probe.snapshot();
  assert.equal(snapshot.disposed, true);
  assert.equal(snapshot.active, false);
  assert.equal(snapshot.residentBytes, 0);
  assert.ok(ports.every((port) => port.terminated));
  await assert.rejects(probe.run({ matrix: matrix() }), code("disposed"));
  await assert.rejects(
    probe.install({ datasetId: "new", generation: 2, bounds: bounds() }),
    code("disposed"),
  );
});
