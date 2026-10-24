import assert from "node:assert/strict";
import test from "node:test";
import { FrameGpuTimer } from "../packages/renderer/frame-gpu-timer.ts";

Object.assign(globalThis, {
  GPUBufferUsage: { QUERY_RESOLVE: 1, COPY_SRC: 2, COPY_DST: 4, MAP_READ: 8 },
  GPUMapMode: { READ: 1 },
});
function deferred() {
  let resolve!: () => void, reject!: (reason?: unknown) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function fixture() {
  const buffers: any[] = [];
  const queries: any[] = [];
  const device = {
    createQuerySet(descriptor: any) {
      const query = {
        descriptor,
        destroyed: false,
        destroy() {
          this.destroyed = true;
        },
      };
      queries.push(query);
      return query;
    },
    createBuffer(descriptor: any) {
      const gate = deferred();
      const buffer = {
        descriptor,
        gate,
        unmapped: 0,
        destroyed: false,
        data: new ArrayBuffer(descriptor.size),
        mapAsync() {
          assert.equal(this, buffer);
          return gate.promise;
        },
        getMappedRange() {
          assert.equal(this, buffer);
          return this.data;
        },
        unmap() {
          assert.equal(this, buffer);
          this.unmapped++;
        },
        destroy() {
          this.destroyed = true;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
  };
  const timer = new FrameGpuTimer(device as unknown as GPUDevice);
  const passes: any[] = [];
  const calls: string[] = [];
  const encoder: any = {
    label: "native-encoder",
    beginRenderPass(descriptor: any) {
      assert.equal(this, encoder);
      return begin(descriptor, "render");
    },
    beginComputePass(descriptor: any) {
      assert.equal(this, encoder);
      return begin(descriptor, "compute");
    },
    copyBufferToBuffer() {
      assert.equal(this, encoder);
      calls.push("copy");
    },
    resolveQuerySet(
      _query: any,
      _start: number,
      count: number,
      _target: any,
      offset: number,
    ) {
      assert.equal(this, encoder);
      assert.equal(count, 512);
      assert.equal(offset, 0);
      calls.push("resolve");
    },
    finish() {
      assert.equal(this, encoder);
      calls.push("finish");
      return "native-command-buffer";
    },
  };
  function begin(descriptor: any, kind: string) {
    const record = {
      kind,
      label: descriptor.label,
      timestamps: descriptor.timestampWrites,
      native: null as any,
    };
    const pass: any = {
      label: descriptor.label,
      setPipeline() {
        assert.equal(this, pass);
      },
      draw() {
        assert.equal(this, pass);
      },
      drawIndexed() {
        assert.equal(this, pass);
      },
      dispatchWorkgroups() {
        assert.equal(this, pass);
      },
      executeBundles() {
        assert.equal(this, pass);
      },
      end() {
        assert.equal(this, pass);
      },
    };
    record.native = pass;
    passes.push(record);
    return pass;
  }
  async function complete(
    slot: any,
    frame: number,
    fill: (write: (index: number, value: bigint) => void) => void,
  ) {
    const read = slot.read;
    const data = new BigUint64Array(read.data);
    fill((index, value) => {
      data[index] = value;
    });
    timer.submitted(frame, slot);
    read.gate.resolve();
    await new Promise((resolve) => setImmediate(resolve));
  }
  return { timer, buffers, queries, encoder, passes, calls, complete };
}
function render(
  encoder: GPUCommandEncoder,
  descriptor: GPURenderPassDescriptor,
) {
  const pass = encoder.beginRenderPass(descriptor);
  pass.draw(3);
  pass.end();
}
const color: any = [{ view: {}, loadOp: "clear", storeOp: "store" }];

test("actual-pass proxy preserves native method binding, frozen descriptors and reserved HDR queries", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  assert.equal(f.queries.length, 8);
  assert.equal(f.queries[0].descriptor.count, 512);
  assert.equal(encoder.finish, encoder.finish);
  encoder.label = "renamed";
  assert.equal(f.encoder.label, "renamed");
  const first = Object.freeze({ label: "shadow", colorAttachments: color });
  render(encoder, first);
  const hdr = Object.freeze({
    label: "HDR",
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot),
  });
  render(encoder, hdr);
  const compute = encoder.beginComputePass({ label: "final-compute" });
  compute.dispatchWorkgroups(1);
  compute.end();
  encoder.copyBufferToBuffer({} as GPUBuffer, 0, {} as GPUBuffer, 0, 4);
  f.timer.resolve(encoder, slot);
  assert.equal(encoder.finish(), "native-command-buffer");
  assert.equal((first as any).timestampWrites, undefined);
  assert.equal(hdr.timestampWrites?.beginningOfPassWriteIndex, 2);
  assert.deepEqual(
    f.passes.map((value) => [
      value.timestamps.beginningOfPassWriteIndex,
      value.timestamps.endOfPassWriteIndex,
    ]),
    [
      [0, 1],
      [2, 3],
      [4, 5],
    ],
  );
  await f.complete(slot, 1, (write) => {
    [1000000n, 2000000n, 3000000n, 23000000n, 24000000n, 25000000n].forEach(
      (value, index) => write(index, value),
    );
  });
  assert.equal(f.timer.sample(1), 20);
  assert.equal(f.timer.sampleFrame(1), 24);
  assert.equal(f.timer.status().availability, "available");
  assert.equal(f.timer.status().lastCoverage?.measuredPasses, 3);
  assert.deepEqual(f.timer.status().lastCoverage?.durations, [
    {
      label: "shadow",
      kind: "render",
      nonempty: true,
      ms: 1,
      startMs: 0,
      endMs: 1,
      reason: null,
    },
    {
      label: "HDR",
      kind: "render",
      nonempty: true,
      ms: 20,
      startMs: 2,
      endMs: 22,
      reason: null,
    },
    {
      label: "final-compute",
      kind: "compute",
      nonempty: true,
      ms: 1,
      startMs: 23,
      endMs: 24,
      reason: null,
    },
  ]);
  const status = f.timer.status();
  status.lastCoverage!.durations[0].ms = 999;
  assert.equal(f.timer.status().lastCoverage?.durations[0].ms, 1);
  assert.deepEqual(f.calls, ["copy", "resolve", "copy", "finish"]);
});

test("split HDR boundaries retain two samples on every actual pass", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  render(encoder, {
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot, "begin"),
  });
  render(encoder, {
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot, "end"),
  });
  assert.deepEqual(
    f.passes.map((value) => [
      value.timestamps.beginningOfPassWriteIndex,
      value.timestamps.endOfPassWriteIndex,
    ]),
    [
      [2, 0],
      [1, 3],
    ],
  );
  await f.complete(slot, 2, (write) => {
    write(2, 1000000n);
    write(0, 5000000n);
    write(1, 6000000n);
    write(3, 11000000n);
  });
  assert.equal(f.timer.sample(2), 10);
  assert.equal(f.timer.sampleFrame(2), 10);
});

test("overlapping pass intervals retain their relative timeline without summing GPU costs", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  render(encoder, { label: "shadow", colorAttachments: color });
  render(encoder, {
    label: "HDR",
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot),
  });
  const compute = encoder.beginComputePass({ label: "overlapped-compute" });
  compute.dispatchWorkgroups(1);
  compute.end();
  await f.complete(slot, 1, (write) => {
    [1000000n, 22000000n, 3000000n, 23000000n, 2000000n, 2500000n].forEach(
      (value, index) => write(index, value),
    );
  });
  assert.equal(f.timer.sampleFrame(1), 22);
  const durations = f.timer.status().lastCoverage!.durations;
  assert.deepEqual(
    durations.map(({ startMs, endMs }) => [startMs, endMs]),
    [
      [0, 21],
      [2, 22],
      [1, 1.5],
    ],
  );
  assert(
    durations.reduce((total, pass) => total + pass.ms!, 0) >
      f.timer.sampleFrame(1)!,
  );
});

test("unavailable or reset nonempty-pass timestamps cannot become a false zero-cost frame", async () => {
  for (const invalid of [0n, 100n]) {
    const f = fixture(),
      slot = f.timer.acquire()!,
      encoder = f.timer.instrumentEncoder(f.encoder, slot);
    render(encoder, { colorAttachments: color, label: "missing-shadow" });
    render(encoder, {
      colorAttachments: color,
      timestampWrites: f.timer.writes(slot),
    });
    await f.complete(slot, 3, (write) => {
      write(0, invalid);
      write(1, 0n);
      write(2, 1000000n);
      write(3, 21000000n);
    });
    assert.equal(f.timer.sample(3), 20);
    assert.equal(f.timer.sampleFrame(3), null);
    assert.equal(f.timer.status().availability, "unavailable");
    assert(
      f.timer
        .status()
        .lastCoverage?.reasons.includes("invalid-nonempty-pass-timestamps"),
    );
  }
});

test("empty zero-valued passes are visible but do not invalidate positive real work", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  const empty = encoder.beginComputePass({ label: "empty" });
  empty.end();
  render(encoder, {
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot),
  });
  await f.complete(slot, 4, (write) => {
    write(2, 1000000n);
    write(3, 21000000n);
  });
  assert.equal(f.timer.sampleFrame(4), 20);
  assert.equal(f.timer.status().lastCoverage?.passes, 2);
  assert.equal(f.timer.status().lastCoverage?.nonemptyPasses, 1);
});

test("capacity overflow and foreign timestamp owners preserve native calls but refuse complete coverage", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  render(encoder, {
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot),
  });
  for (let i = 0; i < 256; i++)
    render(encoder, { colorAttachments: color, label: `pass-${i}` });
  assert.equal(f.passes.length, 257);
  for (const pass of f.passes) {
    const values = pass.timestamps;
    if (values)
      assert(
        values.beginningOfPassWriteIndex < 512 &&
          values.endOfPassWriteIndex < 512,
      );
  }
  await f.complete(slot, 5, (write) => {
    for (const pass of f.passes) {
      const values = pass.timestamps;
      if (values) {
        write(values.beginningOfPassWriteIndex, 1000000n);
        write(values.endOfPassWriteIndex, 2000000n);
      }
    }
  });
  assert.equal(f.timer.sampleFrame(5), null);
  assert.equal(f.timer.status().lastCoverage?.overflow, 1);
  const second = fixture(),
    next = second.timer.acquire()!,
    wrapped = second.timer.instrumentEncoder(second.encoder, next);
  const foreign = {} as GPUQuerySet,
    original = {
      querySet: foreign,
      beginningOfPassWriteIndex: 0,
      endOfPassWriteIndex: 1,
    };
  render(wrapped, { colorAttachments: color, timestampWrites: original });
  assert.equal(second.passes[0].timestamps, original);
  render(wrapped, {
    colorAttachments: color,
    timestampWrites: second.timer.writes(next),
  });
  await second.complete(next, 6, (write) => {
    write(2, 1000000n);
    write(3, 2000000n);
  });
  assert.equal(second.timer.sampleFrame(6), null);
  assert.equal(second.timer.status().lastCoverage?.foreignTimestampPasses, 1);
});

test("clear generations reject late map results and disposal is bounded", async () => {
  const f = fixture(),
    slot = f.timer.acquire()!,
    encoder = f.timer.instrumentEncoder(f.encoder, slot);
  render(encoder, {
    colorAttachments: color,
    timestampWrites: f.timer.writes(slot),
  });
  const read = slot.read as any;
  new BigUint64Array(read.data).set([0n, 0n, 1000000n, 21000000n]);
  f.timer.submitted(7, slot);
  assert.equal(f.timer.status().pendingFrames, 1);
  f.timer.clear();
  read.gate.resolve();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(f.timer.sample(7), null);
  assert.equal(f.timer.sampleFrame(7), null);
  assert.equal(f.timer.status().observedFrames, 0);
  assert.equal(slot.busy, false);
  assert.equal(read.unmapped, 1);
  const copy = f.timer.status();
  copy.excludes.push("changed");
  assert.equal(f.timer.status().excludes.length, 3);
  f.timer.dispose();
  assert(f.buffers.every((buffer) => buffer.destroyed));
  assert(f.queries.every((query) => query.destroyed));
  assert.equal(f.timer.acquire(), undefined);
});

test("bundle-only render passes require timestamps while empty bundle lists remain empty", async () => {
  for (const scenario of ["empty", "missing", "measured"] as const) {
    const f = fixture(),
      slot = f.timer.acquire()!,
      encoder = f.timer.instrumentEncoder(f.encoder, slot);
    const bundle = encoder.beginRenderPass({
      label: "bundled-meshes",
      colorAttachments: [
        { view: {} as GPUTextureView, loadOp: "load", storeOp: "store" },
      ],
    });
    bundle.executeBundles(scenario === "empty" ? [] : [{} as GPURenderBundle]);
    bundle.end();
    render(encoder, {
      label: "HDR",
      colorAttachments: color,
      timestampWrites: f.timer.writes(slot),
    });
    await f.complete(slot, 1, (write) => {
      if (scenario === "measured") {
        write(0, 1000000n);
        write(1, 3000000n);
      }
      write(2, 5000000n);
      write(3, 20000000n);
    });
    assert.equal(f.timer.sample(1), 15);
    assert.equal(
      f.timer.status().lastCoverage?.nonemptyPasses,
      scenario === "empty" ? 1 : 2,
    );
    if (scenario === "missing") {
      assert.equal(f.timer.sampleFrame(1), null);
      assert(
        f.timer
          .status()
          .lastCoverage!.reasons.includes("invalid-nonempty-pass-timestamps"),
      );
    } else assert.equal(f.timer.sampleFrame(1), scenario === "empty" ? 15 : 19);
  }
});
