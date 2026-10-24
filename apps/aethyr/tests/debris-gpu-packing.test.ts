import assert from "node:assert/strict";
import test from "node:test";
import {
  DebrisGpuPacking,
  transformPackingRecord,
  DEBRIS_PACKING_WGSL,
  type PackingTarget,
} from "../packages/renderer/debris-gpu-packing.ts";
import type { FractureChunk } from "../packages/renderer/source-fracture.ts";
Object.assign(globalThis, {
  GPUBufferUsage: { STORAGE: 1, COPY_DST: 2, COPY_SRC: 4 },
});
function chunk(count = 6): FractureChunk {
  const vertices = new Float32Array(count * 12),
    colors = new Uint8Array(count * 4),
    uv1 = new Float32Array(count * 2);
  for (let i = 0; i < count; i++) {
    vertices.set(
      [i * 0.3, -i * 0.7, i * 0.11, 0, 1, 0, 0.2, 0.3, 1, 0, 0, -1],
      i * 12,
    );
    colors.set([i, 23, 127, 255], i * 4);
    uv1.set([i * 0.1, i * 0.2], i * 2);
  }
  return {
    center: [0.1234567890123, -14.234567890123, 23.98765432109],
    vertices,
    colors,
    uv1,
    draws: [
      { firstIndex: 0, indexCount: 3, material: 1 },
      { firstIndex: 3, indexCount: count - 3, material: 2 },
    ],
  };
}
async function fixture() {
  const buffers: any[] = [],
    dispatches: any[] = [],
    writes: any[] = [];
  let accounted = 0;
  const device: any = {
    limits: {
      maxStorageBufferBindingSize: 1024 * 1024,
      maxBufferSize: 64 * 1024 * 1024,
      minStorageBufferOffsetAlignment: 256,
    },
    queue: {
      writeBuffer(
        buffer: any,
        offset: number,
        data: ArrayBuffer | ArrayBufferView,
        dataOffset = 0,
        size?: number,
      ) {
        const input =
          data instanceof ArrayBuffer
            ? new Uint8Array(data, dataOffset, size)
            : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        buffer.data.set(input, offset);
        writes.push({ buffer, offset, size: input.byteLength });
      },
    },
    createBuffer(descriptor: any) {
      let bytes: Uint8Array | undefined;
      const buffer = {
        ...descriptor,
        destroyed: false,
        get data() {
          return (bytes ??= new Uint8Array(descriptor.size));
        },
        destroy() {
          this.destroyed = true;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createShaderModule({ code }: any) {
      assert.equal(code, DEBRIS_PACKING_WGSL);
      return {};
    },
    async createComputePipelineAsync() {
      return { getBindGroupLayout: () => ({}) };
    },
    createBindGroup({ entries }: any) {
      return entries;
    },
  };
  const packer = await DebrisGpuPacking.create(device, (delta) => {
    accounted += delta;
  });
  function target(count = 64): PackingTarget {
    return {
      vertices: device.createBuffer({ size: count * 48 }),
      colors: device.createBuffer({ size: count * 4 }),
      uv1: device.createBuffer({ size: count * 8 }),
    };
  }
  const encoder: any = {
    beginComputePass() {
      let groups: any;
      return {
        setPipeline() {},
        setBindGroup(_slot: number, value: any) {
          groups = value;
        },
        dispatchWorkgroups(count: number) {
          dispatches.push({ count, groups });
          const read = (binding: number) =>
            groups.find((item: any) => item.binding === binding).resource;
          const source = read(0).buffer.data,
            descriptor = read(1),
            jobs = new Uint32Array(
              descriptor.buffer.data.buffer,
              descriptor.offset,
              descriptor.size / 4,
            ),
            floats = new Float32Array(
              jobs.buffer,
              jobs.byteOffset,
              jobs.length,
            );
          const outV = new Uint32Array(read(2).buffer.data.buffer),
            outC = new Uint32Array(read(3).buffer.data.buffer),
            outUV = new Uint32Array(read(4).buffer.data.buffer),
            input = new Uint32Array(source.buffer),
            inputF = new Float32Array(source.buffer);
          for (let j = 0; j < count; j++) {
            const job = j * 16,
              src = jobs[job],
              dst = jobs[job + 1],
              n = jobs[job + 2],
              transform = jobs[job + 3];
            for (let i = 0; i < n; i++) {
              outV.set(
                input.subarray((src + i) * 16, (src + i) * 16 + 12),
                (dst + i) * 12,
              );
              if (transform) {
                const m = new Float32Array(16);
                for (let axis = 0; axis < 3; axis++)
                  for (let col = 0; col < 4; col++)
                    m[axis + col * 4] = floats[job + 4 + axis * 4 + col];
                outV.set(
                  new Uint32Array(
                    transformPackingRecord(
                      inputF.slice((src + i) * 16, (src + i) * 16 + 12),
                      m,
                    ).buffer,
                  ),
                  (dst + i) * 12,
                );
              }
              outC[dst + i] = input[(src + i) * 16 + 12];
              outUV.set(
                input.subarray((src + i) * 16 + 13, (src + i) * 16 + 15),
                (dst + i) * 2,
              );
            }
          }
        },
        end() {},
      };
    },
  };
  return {
    packer,
    target,
    encoder,
    buffers,
    writes,
    dispatches,
    account: () => accounted,
  };
}
function finish<T>(steps: Generator<void, T>) {
  for (;;) {
    const result = steps.next();
    if (result.done) return result.value;
  }
}
function expected(chunk: FractureChunk, world: boolean) {
  const v = chunk.vertices.slice();
  if (world)
    for (let i = 0; i < v.length; i += 12)
      for (let axis = 0; axis < 3; axis++) v[i + axis] += chunk.center[axis];
  return v;
}

test("world preparation plus GPU copy preserves exact CPU f64-to-f32 translated records and all attributes", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  assert.equal(
    finish(f.packer.prepare([{ chunk: c, world: true }], () => false)),
    true,
  );
  const result = f.packer.stage(
    [{ chunk: c, world: true, instance: 0 }],
    t,
    64,
  )!;
  assert.deepEqual(result.draws, [
    { material: 1, firstIndex: 0, indexCount: 3, instance: 0 },
    { material: 2, firstIndex: 3, indexCount: 3, instance: 0 },
  ]);
  f.packer.encode(f.encoder);
  assert.deepEqual(
    new Float32Array((t.vertices as any).data.buffer, 0, c.vertices.length),
    expected(c, true),
  );
  assert.deepEqual((t.colors as any).data.slice(0, c.colors.length), c.colors);
  assert.deepEqual(
    new Float32Array((t.uv1 as any).data.buffer, 0, c.uv1.length),
    c.uv1,
  );
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.submitted();
  assert.equal(f.packer.snapshot().pendingTargets, 0);
  f.packer.dispose();
  assert.equal(f.account(), 0);
});

test("material/source order and merged instance draws match the established packing order", async () => {
  const f = await fixture(),
    a = chunk(),
    b = chunk(),
    t = f.target();
  b.draws.reverse();
  assert(
    finish(
      f.packer.prepare(
        [
          { chunk: a, world: false },
          { chunk: b, world: false },
        ],
        () => false,
      ),
    ),
  );
  const result = f.packer.stage(
    [
      { chunk: a, world: false, instance: 2 },
      { chunk: b, world: false, instance: 2 },
    ],
    t,
    64,
  )!;
  assert.deepEqual(result.draws, [
    { material: 1, firstIndex: 0, indexCount: 6, instance: 2 },
    { material: 2, firstIndex: 6, indexCount: 6, instance: 2 },
  ]);
  f.packer.encode(f.encoder);
  const expectedV = new Float32Array([
    ...a.vertices.slice(0, 36),
    ...b.vertices.slice(0, 36),
    ...a.vertices.slice(36),
    ...b.vertices.slice(36),
  ]);
  assert.deepEqual(
    new Float32Array((t.vertices as any).data.buffer, 0, expectedV.length),
    expectedV,
  );
  f.packer.dispose();
});

test("GPU rubble job matrices keep position, normals, tangents and untouched UV/handedness fields", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  const m = new Float32Array([
    0.7, 0.2, -0.5, 0, -0.3, 0.9, 0.1, 0, 0.6, -0.2, 0.8, 0, 123, -45, 67, 1,
  ]);
  assert(finish(f.packer.prepare([{ chunk: c, world: false }], () => false)));
  const result = f.packer.stage(
    [{ chunk: c, world: false, instance: 0, transform: m }],
    t,
    64,
  )!;
  f.packer.encode(f.encoder);
  const out = new Float32Array(
    (t.vertices as any).data.buffer,
    0,
    c.vertices.length,
  );
  let maxError = 0;
  for (let i = 0; i < c.vertices.length; i += 12) {
    const input = c.vertices.slice(i, i + 12),
      predicted = transformPackingRecord(input, m);
    assert.deepEqual(out.slice(i, i + 12), predicted);
    for (const attribute of [0, 3, 8])
      for (let axis = 0; axis < 3; axis++) {
        const ref = Math.fround(
          m[axis] * input[attribute] +
            m[axis + 4] * input[attribute + 1] +
            m[axis + 8] * input[attribute + 2] +
            (attribute === 0 ? m[axis + 12] : 0),
        );
        maxError = Math.max(
          maxError,
          Math.abs(ref - predicted[attribute + axis]),
        );
      }
    for (let axis = 0; axis < 3; axis++)
      assert(
        out[i + axis] >= result.bounds!.min[axis] &&
          out[i + axis] <= result.bounds!.max[axis],
      );
    assert.equal(out[i + 11], -1);
  }
  assert(maxError < 0.00002);
  f.packer.dispose();
});

test("cancelled cooperative preparation frees candidate slots and preserves previous resident chunks", async () => {
  const f = await fixture(),
    a = chunk(),
    b = chunk(4096);
  assert(finish(f.packer.prepare([{ chunk: a, world: false }], () => false)));
  let cancelled = false;
  const steps = f.packer.prepare(
    [
      { chunk: a, world: false },
      { chunk: b, world: true },
    ],
    () => cancelled,
  );
  assert.equal(steps.next().done, false);
  cancelled = true;
  assert.equal(steps.next().value, false);
  assert.equal(f.packer.snapshot().residentChunks, 1);
  assert(
    f.packer.stage([{ chunk: a, world: false, instance: 0 }], f.target(), 64),
  );
  assert.throws(
    () =>
      f.packer.stage(
        [{ chunk: b, world: true, instance: 0 }],
        f.target(4096),
        4096,
      ),
    /Prepare all/,
  );
  f.packer.dispose();
});

test("abandoned encoders keep pending work and newer target plans replace rather than accumulate", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  assert(finish(f.packer.prepare([{ chunk: c, world: false }], () => false)));
  f.packer.stage([{ chunk: c, world: false, instance: 0 }], t, 64);
  f.packer.encode(f.encoder);
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.encode(f.encoder);
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.stage([], t, 64);
  f.packer.submitted();
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.encode(f.encoder);
  f.packer.submitted();
  assert.equal(f.packer.snapshot().pendingTargets, 0);
  f.packer.dispose();
});

test("unreferenced chunks release their arenas while queued jobs hold referenced chunks through submission", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  assert(finish(f.packer.prepare([{ chunk: c, world: false }], () => false)));
  f.packer.stage([{ chunk: c, world: false, instance: 0 }], t, 64);
  assert(finish(f.packer.prepare([], () => false)));
  assert.equal(f.packer.snapshot().residentChunks, 1);
  f.packer.encode(f.encoder);
  f.packer.submitted();
  assert.equal(f.packer.snapshot().residentChunks, 0);
  assert.equal(f.packer.snapshot().arenaBytes, 0);
  f.packer.dispose();
  assert.equal(f.account(), 0);
});

test("arena memory and job capacity stay bounded and disposal is idempotent", async () => {
  const f = await fixture();
  const pageRecords = (1024 * 1024) / 64;
  for (let i = 0; i < 160; i++) (f.packer as any).allocate(pageRecords);
  assert.equal(f.packer.snapshot().arenaBytes, 160 * 1024 * 1024);
  assert.throws(() => (f.packer as any).allocate(1), /160 MiB/);
  f.packer.dispose();
  f.packer.dispose();
  assert.equal(f.account(), 0);
  assert.throws(() => f.packer.stage([], f.target(), 10), /closed/);
});

test("draw bounds and invalid source records reject before changing target plans", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  assert(finish(f.packer.prepare([{ chunk: c, world: false }], () => false)));
  c.draws[0].indexCount = 100;
  assert.throws(
    () => f.packer.stage([{ chunk: c, world: false, instance: 0 }], t, 64),
    /ranges/,
  );
  assert.equal(f.packer.snapshot().pendingTargets, 0);
  const invalid = chunk();
  invalid.colors = new Uint8Array(1);
  assert.throws(
    () =>
      finish(f.packer.prepare([{ chunk: invalid, world: false }], () => false)),
    /complete/,
  );
  f.packer.dispose();
});

test("output size, aliasing and pending-target limits reject before replacing working plans", async () => {
  const f = await fixture(),
    c = chunk(),
    t = f.target();
  assert(finish(f.packer.prepare([{ chunk: c, world: false }], () => false)));
  const parts = [{ chunk: c, world: false, instance: 0 }];
  f.packer.stage(parts, t, 64);
  assert.throws(() => f.packer.stage(parts, f.target(3), 64), /cannot hold/);
  assert.throws(
    () => f.packer.stage(parts, { ...t, colors: t.vertices }, 64),
    /separate/,
  );
  assert.throws(
    () => f.packer.stage(parts, { ...t, vertices: (f.packer as any).jobs }, 64),
    /separate/,
  );
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.stage(parts, f.target(), 64);
  f.packer.stage(parts, f.target(), 64);
  assert.throws(() => f.packer.stage(parts, f.target(), 64), /three pending/);
  f.packer.dispose();
});

test("restore discards staged work while retaining resident fragments for the next cast", async () => {
  const f = await fixture(),
    c = chunk(),
    target = f.target();
  const steps = f.packer.prepare([{ chunk: c, world: false }], () => false);
  while (!steps.next().done) {}
  const before = f.packer.snapshot();
  f.packer.stage([{ chunk: c, world: false, instance: 0 }], target, 64);
  f.packer.encode(f.encoder);
  f.packer.discardStaged();
  assert.equal(f.packer.snapshot().pendingTargets, 0);
  assert.equal(f.packer.snapshot().residentChunks, before.residentChunks);
  assert.equal(f.packer.snapshot().arenaBytes, before.arenaBytes);
  assert.equal(f.packer.snapshot().preparedBytes, before.preparedBytes);
  f.packer.stage([{ chunk: c, world: false, instance: 0 }], target, 64);
  assert.equal(f.packer.snapshot().pendingTargets, 1);
  f.packer.dispose();
});
test("compilation failures retain detailed diagnostics behind a plain product message", async () => {
  let pipelineRequested = false;
  const device = {
    createShaderModule() {
      return {
        async getCompilationInfo() {
          return {
            messages: [
              {
                type: "error",
                lineNum: 18,
                linePos: 12,
                message: "A reserved keyword was used.",
              },
            ],
          };
        },
      };
    },
    async createComputePipelineAsync() {
      pipelineRequested = true;
    },
  };
  await assert.rejects(
    DebrisGpuPacking.create(device as unknown as GPUDevice),
    (error: Error) => {
      assert.equal(error.message, "Prepared debris graphics could not start.");
      assert.deepEqual(error.cause, [
        { line: 18, column: 12, message: "A reserved keyword was used." },
      ]);
      return true;
    },
  );
  assert.equal(pipelineRequested, false);
});
