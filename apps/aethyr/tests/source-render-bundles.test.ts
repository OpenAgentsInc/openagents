import assert from "node:assert/strict";
import test from "node:test";
import {
  SourceRenderBundles,
  MAX_BUNDLED_SOURCE_DRAWS,
  SOURCE_BUNDLE_PAGE_DRAWS,
  type BundledSourceDraw,
  type SourceBundleBindings,
} from "../packages/renderer/source-render-bundles.ts";
import { compactInstanceDraws } from "../packages/renderer/culling.ts";
import { splitRasterOrientationDraws } from "../packages/renderer/raster-orientation.ts";

Object.assign(globalThis, { GPUBufferUsage: { INDIRECT: 1, COPY_DST: 2 } });
function fixture(draws = baseDraws(), pruneNeverVisible = true) {
  let failUpload = false,
    finishFailureIn = -1;
  const buffers: any[] = [],
    encoders: any[] = [],
    uploads: Uint32Array[] = [];
  const device = {
    queue: {
      writeBuffer(buffer: any, offset: number, data: Uint32Array) {
        if (failUpload) {
          failUpload = false;
          throw new Error("injected upload failure");
        }
        assert.equal(offset, 0);
        assert.equal(buffer.destroyed, false);
        buffer.words = data.slice();
        uploads.push(data.slice());
      },
    },
    createBuffer(descriptor: any) {
      const buffer = {
        ...descriptor,
        destroyed: false,
        destroy() {
          this.destroyed = true;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createRenderBundleEncoder(descriptor: any) {
      const calls: any[][] = [],
        record = { descriptor, calls, finished: false };
      encoders.push(record);
      return {
        ...Object.fromEntries(
          [
            "setBindGroup",
            "setVertexBuffer",
            "setIndexBuffer",
            "setPipeline",
            "drawIndexedIndirect",
          ].map((name) => [
            name,
            (...args: any[]) => {
              calls.push([name, ...args]);
            },
          ]),
        ),
        finish() {
          if (finishFailureIn === 0) {
            finishFailureIn = -1;
            throw new Error("injected finish failure");
          }
          if (finishFailureIn > 0) finishFailureIn--;
          record.finished = true;
          return { record };
        },
      };
    },
  };
  let accounted = 0;
  const table = new SourceRenderBundles(
    device as unknown as GPUDevice,
    draws,
    (delta) => {
      accounted += delta;
    },
    pruneNeverVisible,
  );
  const buffer = () => ({}) as GPUBuffer;
  const materials = [{}, {}, {}] as GPUBindGroup[],
    pipelines = [{}, {}, {}] as GPURenderPipeline[];
  const bindings: SourceBundleBindings = {
    key: "main:0:0",
    descriptor: {
      colorFormats: ["rgba16float"],
      depthStencilFormat: "depth24plus",
      sampleCount: 4,
    },
    frameGroup: {} as GPUBindGroup,
    shader: {},
    pipelineVersion: 0,
    materials,
    vertices: buffer(),
    instances: buffer(),
    colors: buffer(),
    uv1: buffer(),
    wind: buffer(),
    zeroWind: buffer(),
    indices: buffer(),
    include: () => true,
    material: (draw) => materials[draw.material],
    pipeline: (draw) => pipelines[draw.material],
  };
  return {
    table,
    bindings,
    encoders,
    buffers,
    uploads,
    account: () => accounted,
    failUpload: () => {
      failUpload = true;
    },
    failFinish: (successfulPages = 0) => {
      finishFailureIn = successfulPages;
    },
  };
}
function baseDraws(): BundledSourceDraw[] {
  return [
    {
      sourceDrawIndex: 0,
      indexCount: 12,
      firstIndex: 0,
      baseVertex: 4,
      firstInstance: 0,
      count: 2,
      material: 0,
      wind: false,
      rasterOrientation: "positive",
    },
    {
      sourceDrawIndex: 1,
      indexCount: 18,
      firstIndex: 12,
      baseVertex: -3,
      firstInstance: 2,
      count: 2,
      material: 1,
      wind: true,
      rasterOrientation: "negative",
    },
    {
      sourceDrawIndex: 2,
      indexCount: 6,
      firstIndex: 30,
      baseVertex: 9,
      firstInstance: 4,
      count: 1,
      material: 2,
      wind: false,
      rasterOrientation: "degenerate",
    },
  ];
}
function activeCommands(table: SourceRenderBundles) {
  const words = table.commandSnapshot(),
    result: number[][] = [];
  for (let i = 0; i < words.length; i += 5)
    if (words[i + 1])
      result.push([
        words[i],
        words[i + 1],
        words[i + 2],
        words[i + 3] | 0,
        words[i + 4],
      ]);
  return result;
}
function direct(draws: readonly BundledSourceDraw[]) {
  return draws
    .filter((draw) => draw.count)
    .map((draw) => [
      draw.indexCount,
      draw.count,
      draw.firstIndex,
      draw.baseVertex,
      draw.firstInstance,
    ]);
}

test("indirect command lanes match direct draws over moving views, all-dead, and restore", () => {
  const f = fixture(),
    base = baseDraws();
  for (const visible of [
    base,
    [{ ...base[1], count: 1, firstInstance: 0 }],
    [],
    [
      { ...base[0], count: 1 },
      { ...base[2], firstInstance: 1 },
    ],
    base,
  ]) {
    f.table.update(visible);
    assert.deepEqual(activeCommands(f.table), direct(visible));
  }
  assert.equal(f.table.snapshot().bytes, 60);
  assert.equal(f.account(), 60);
});

test("source orientation splitting and real instance compaction preserve indirect IDs and exact command ranges", () => {
  const models = new Float32Array(5 * 16);
  for (let i = 0; i < 5; i++) {
    models.set(
      [i % 2 ? -1 : 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, i, 0, 0, 1],
      i * 16,
    );
  }
  const draws = splitRasterOrientationDraws(models, [
    {
      firstInstance: 0,
      count: 5,
      material: 0,
      indexCount: 12,
      firstIndex: 8,
      baseVertex: 2,
      wind: true,
    },
  ]).map((draw, sourceDrawIndex) => ({ ...draw, sourceDrawIndex }));
  const f = fixture(draws);
  for (const mask of [
    [1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0],
    [1, 0, 0, 0, 1],
    [0, 0, 0, 0, 0],
  ]) {
    const compacted = compactInstanceDraws(models, draws, new Uint8Array(mask));
    f.table.update(compacted.draws);
    assert.deepEqual(activeCommands(f.table), direct(compacted.draws));
    assert.deepEqual(
      Array.from(compacted.models).filter((_, i) => i % 16 === 12),
      mask.flatMap((visible, i) => (visible ? [i] : [])),
    );
  }
});

test("one immutable bundle retains source order, mirrored pipelines, material groups, and wind bindings", () => {
  const f = fixture();
  f.table.update(baseDraws());
  const bundle = f.table.get(f.bindings);
  assert.equal(bundle.length, 1);
  const calls = f.encoders[0].calls;
  assert.deepEqual(
    calls
      .filter((call: any[]) => call[0] === "drawIndexedIndirect")
      .map((call: any[]) => call[2]),
    [0, 20, 40],
  );
  assert.deepEqual(
    calls
      .filter((call: any[]) => call[0] === "setBindGroup")
      .map((call: any[]) => call[1]),
    [0, 1, 1, 1],
  );
  assert.deepEqual(
    calls
      .filter((call: any[]) => call[0] === "setVertexBuffer" && call[1] === 4)
      .map((call: any[]) => call[2]),
    [f.bindings.zeroWind, f.bindings.wind, f.bindings.zeroWind],
  );
  for (const visible of [[], [baseDraws()[2]], baseDraws()]) {
    f.table.update(visible);
    assert.equal(f.table.get(f.bindings), bundle);
  }
  assert.equal(f.encoders.length, 1);
  assert.equal(f.table.snapshot().reuses, 3);
});

test("only changed indirect ranges upload and failed ranges leave all prior commands intact", () => {
  const f = fixture(),
    base = baseDraws();
  f.table.update(base);
  const uploads = f.uploads.length,
    commands = f.table.commandSnapshot();
  f.table.update(base);
  assert.equal(f.uploads.length, uploads);
  const invalids = [
    [base[1], base[0]],
    [base[0], base[0]],
    [{ ...base[0], material: 2 }],
    [{ ...base[0], wind: true }],
    [{ ...base[0], rasterOrientation: "negative" as const }],
    [{ ...base[0], count: -1 }],
    [{ ...base[0], count: base[0].count + 1 }],
    [{ ...base[0], firstInstance: 5, count: 1 }],
    [{ ...base[0], firstInstance: NaN }],
    [{ ...base[0], firstIndex: 3 }],
  ];
  for (const invalid of invalids) {
    assert.throws(() => f.table.update(invalid));
    assert.deepEqual(f.table.commandSnapshot(), commands);
  }
  assert.equal(f.uploads.length, uploads);
  assert.equal(f.table.snapshot().uploadBytes, uploads * 60);
});

test("shader, pipeline, frame/material group, buffer reallocation, and pass compatibility changes replace cached bundles", () => {
  const f = fixture();
  f.table.update(baseDraws());
  let previous = f.table.get(f.bindings);
  for (const key of [
    "shader",
    "frameGroup",
    "materials",
    "vertices",
    "instances",
    "colors",
    "uv1",
    "wind",
    "zeroWind",
    "indices",
  ] as const) {
    (f.bindings as any)[key] = {};
    const next = f.table.get(f.bindings);
    assert.notEqual(next, previous);
    previous = next;
  }
  f.bindings.pipelineVersion++;
  assert.notEqual(f.table.get(f.bindings), previous);
  previous = f.table.get(f.bindings);
  f.bindings.descriptor = { ...f.bindings.descriptor, sampleCount: 1 };
  assert.notEqual(f.table.get(f.bindings), previous);
  assert.equal(f.table.snapshot().bundles, 1);
});

test("pass subsets preserve order, empty subsets finish their encoder, and cache slots stay bounded", () => {
  const f = fixture();
  f.table.update(baseDraws());
  f.bindings.include = (draw) => draw.material !== 1;
  f.table.get(f.bindings);
  assert.deepEqual(
    f.encoders[0].calls
      .filter((call: any[]) => call[0] === "drawIndexedIndirect")
      .map((call: any[]) => call[2]),
    [0, 40],
  );
  f.bindings.key = "empty";
  f.bindings.include = () => false;
  assert.deepEqual(f.table.get(f.bindings), []);
  assert.equal(f.encoders[1].finished, true);
  assert.deepEqual(f.table.get(f.bindings), []);
  assert.equal(f.encoders.length, 2);
  for (let i = 2; i < 8; i++) {
    f.bindings.key = String(i);
    f.table.get(f.bindings);
  }
  f.bindings.key = "overflow";
  assert.throws(() => f.table.get(f.bindings), /too many/);
});

test("empty tables and disposal account exact GPU storage and reject later access", () => {
  const f = fixture([]);
  assert.equal(f.account(), 20);
  assert.equal(f.table.snapshot().uploadBytes, 0);
  f.table.update([]);
  assert.deepEqual(f.table.get(f.bindings), []);
  f.table.dispose();
  f.table.dispose();
  assert.equal(f.account(), 0);
  assert.equal(f.buffers[0].destroyed, true);
  assert.throws(() => f.table.update([]), /closed/);
  assert.throws(() => f.table.get(f.bindings), /closed/);
  assert.throws(() => f.table.commandSnapshot(), /closed/);
});

test("invalid complete source IDs and excessive tables fail before GPU allocation", () => {
  assert.throws(
    () => fixture([{ ...baseDraws()[0], sourceDrawIndex: 1 }]),
    /stable/,
  );
  assert.throws(
    () => fixture([{ ...baseDraws()[0], baseVertex: 2 ** 31 }]),
    /stable/,
  );
  assert.throws(
    () => fixture(Array(MAX_BUNDLED_SOURCE_DRAWS + 1).fill(baseDraws()[0])),
    /limit/,
  );
});

function recordedOffsets(encoder: any) {
  return encoder.calls
    .filter((call: any[]) => call[0] === "drawIndexedIndirect")
    .map((call: any[]) => call[2]);
}

test("admitted IDs grow monotonically, preserve source offsets, and stabilize across empty, damaged, and restored views", () => {
  const f = fixture(baseDraws(), true),
    base = baseDraws();
  assert.deepEqual(f.table.get(f.bindings), []);
  f.table.update([{ ...base[1], count: 1, firstInstance: 0 }]);
  const first = f.table.get(f.bindings);
  assert.equal(first.length, 1);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [20]);
  assert.deepEqual(f.table.commandCounts(), {
    draws: 1,
    pipelines: 1,
    materials: 1,
    windBindings: 1,
  });
  assert.deepEqual(
    f.encoders
      .at(-1)
      .calls.filter(
        (call: any[]) => call[0] === "setVertexBuffer" && call[1] === 4,
      )
      .map((call: any[]) => call[2]),
    [f.bindings.wind],
  );
  for (const visible of [[], [{ ...base[1], count: 2, firstInstance: 0 }]]) {
    f.table.update(visible);
    assert.equal(f.table.get(f.bindings), first);
    assert.deepEqual(activeCommands(f.table), direct(visible));
  }
  f.table.update([
    { ...base[0], count: 1 },
    { ...base[2], firstInstance: 1 },
  ]);
  const grown = f.table.get(f.bindings);
  assert.notEqual(grown, first);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20, 40]);
  const builds = f.table.snapshot().builds;
  for (const visible of [[], [base[2]], base]) {
    f.table.update(visible);
    assert.equal(f.table.get(f.bindings), grown);
    assert.deepEqual(activeCommands(f.table), direct(visible));
  }
  assert.equal(f.table.snapshot().builds, builds);
  assert.equal(f.table.snapshot().admittedDraws, 3);
  assert.equal(f.table.snapshot().membershipRevision, 2);
  assert.equal(f.table.snapshot().cpuMembershipBytes, 19);
});

test("membership revision invalidates every pass slot, with independent cached command counts", () => {
  const f = fixture(baseDraws(), true),
    base = baseDraws();
  f.table.update([base[0]]);
  const main = f.table.get(f.bindings);
  f.bindings.key = "depth";
  f.bindings.include = (draw) => draw.material === 1;
  assert.deepEqual(f.table.get(f.bindings), []);
  assert.equal(f.table.commandCounts().draws, 0);
  f.table.update([base[0], base[1]]);
  const depth = f.table.get(f.bindings);
  assert.equal(depth.length, 1);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [20]);
  f.bindings.key = "main:0:0";
  f.bindings.include = () => true;
  const nextMain = f.table.get(f.bindings);
  assert.notEqual(nextMain, main);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20]);
  assert.equal(f.table.commandCounts().draws, 2);
  f.bindings.key = "depth";
  f.bindings.include = (draw) => draw.material === 1;
  assert.equal(f.table.get(f.bindings), depth);
  assert.equal(f.table.commandCounts().draws, 1);
  f.bindings.key = "main:0:0";
  f.bindings.include = () => true;
  assert.equal(f.table.get(f.bindings), nextMain);
  assert.equal(f.table.commandCounts().draws, 2);
});

test("failed upload cannot publish IDs, identical retry publishes them, and changed retry cannot admit failed-only IDs", () => {
  const f = fixture(baseDraws(), true),
    base = baseDraws();
  f.table.update([base[0]]);
  f.table.get(f.bindings);
  f.failUpload();
  assert.throws(() => f.table.update([base[1]]), /injected upload/);
  assert.equal(f.table.snapshot().admittedDraws, 1);
  assert.equal(f.table.snapshot().membershipRevision, 1);
  assert.throws(() => f.table.get(f.bindings), /Upload the complete/);
  const uploads = f.uploads.length;
  f.table.update([base[1]]);
  assert.equal(f.uploads.length, uploads + 1);
  assert.equal(f.table.snapshot().admittedDraws, 2);
  f.table.get(f.bindings);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20]);
  f.failUpload();
  assert.throws(() => f.table.update([base[2]]), /injected upload/);
  f.table.update([]);
  assert.equal(f.table.snapshot().admittedDraws, 2);
  assert.equal(f.table.snapshot().membershipRevision, 2);
  assert.deepEqual(activeCommands(f.table), []);
  f.table.get(f.bindings);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20]);
});

test("failed recording never publishes an old membership bundle and retries all new source IDs", () => {
  const f = fixture(baseDraws(), true),
    base = baseDraws();
  f.table.update([base[0]]);
  const old = f.table.get(f.bindings);
  const builds = f.table.snapshot().builds;
  f.table.update([base[1]]);
  f.failFinish();
  assert.throws(() => f.table.get(f.bindings), /injected finish/);
  assert.equal(f.table.snapshot().builds, builds);
  const next = f.table.get(f.bindings);
  assert.notEqual(next, old);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20]);
  f.bindings.pipelineVersion++;
  const pipeline = f.bindings.pipeline;
  f.bindings.pipeline = () => {
    throw new Error("injected pipeline failure");
  };
  assert.throws(() => f.table.get(f.bindings), /injected pipeline/);
  f.bindings.pipeline = pipeline;
  assert.notEqual(f.table.get(f.bindings), next);
});

test("invalid visible identity cannot alter admission or commands; invisible callbacks are not used as validators", () => {
  const f = fixture(baseDraws(), true),
    base = baseDraws();
  f.table.update([base[1]]);
  const commands = f.table.commandSnapshot();
  assert.throws(
    () => f.table.update([{ ...base[0], material: 99 }]),
    /retain their geometry/,
  );
  assert.deepEqual(f.table.commandSnapshot(), commands);
  assert.equal(f.table.snapshot().admittedDraws, 1);
  const invoked: number[] = [];
  f.bindings.include = (draw) => {
    invoked.push(draw.sourceDrawIndex);
    return true;
  };
  f.table.get(f.bindings);
  assert.deepEqual(invoked, [1]);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [20]);
});

test("all-table control preserves immutable zero commands while reporting real admission separately", () => {
  const f = fixture(baseDraws(), false),
    base = baseDraws();
  f.table.update([base[1]]);
  const bundle = f.table.get(f.bindings);
  assert.deepEqual(recordedOffsets(f.encoders.at(-1)), [0, 20, 40]);
  f.table.update(base);
  assert.equal(f.table.get(f.bindings), bundle);
  assert.equal(f.table.snapshot().policy, "all-source-ids");
  assert.equal(f.table.snapshot().admittedDraws, 3);
  assert.equal(f.table.commandCounts().draws, 3);
});

function pagedDraws(count: number): BundledSourceDraw[] {
  return Array.from({ length: count }, (_, index) => ({
    ...baseDraws()[index % 3],
    sourceDrawIndex: index,
    firstIndex: index * 3,
    firstInstance: index,
    indexCount: 3,
    count: 1,
  }));
}
function bundleOffsets(bundles: readonly GPURenderBundle[]) {
  return bundles.flatMap((bundle: any) => recordedOffsets(bundle.record));
}

test("sparse pages preserve global source order and a newly admitted ID rebuilds only its page", () => {
  const size = SOURCE_BUNDLE_PAGE_DRAWS,
    base = pagedDraws(size * 4 + 3),
    f = fixture(base),
    ids = [0, size - 1, size, size * 2, base.length - 1];
  f.table.update(ids.map((index) => base[index]));
  const first = f.table.get(f.bindings);
  assert.equal(first.length, 4);
  assert.ok(Object.isFrozen(first));
  assert.deepEqual(
    bundleOffsets(first),
    ids.map((id) => id * 20),
  );
  assert.deepEqual(f.table.snapshot().pageRevisions, [1, 1, 1, 0, 1]);
  assert.equal(f.table.snapshot().cpuMembershipBytes, base.length * 5 + 5 * 4);
  assert.equal(f.table.snapshot().pageCount, 5);
  assert.equal(f.table.snapshot().pageBundles, 4);
  const nextIds = [...ids, size + 1].sort((a, b) => a - b);
  f.table.update(nextIds.map((index) => base[index]));
  const next = f.table.get(f.bindings);
  assert.notEqual(next, first);
  assert.equal(next[0], first[0]);
  assert.notEqual(next[1], first[1]);
  assert.equal(next[2], first[2]);
  assert.equal(next[3], first[3]);
  assert.deepEqual(
    bundleOffsets(next),
    nextIds.map((id) => id * 20),
  );
  assert.equal(f.table.snapshot().builds, 5);
  assert.equal(f.table.snapshot().slotUpdates, 2);
  assert.equal(f.table.snapshot().pageReuses, 3);
  assert.ok(recordedOffsets(f.encoders.at(-1)).length <= size);
  for (const visible of [[], ids.map((id) => base[id]), base]) {
    // The full table adds the previously absent fourth page only on its first visit.
    f.table.update(visible);
    assert.deepEqual(activeCommands(f.table), direct(visible));
    if (visible !== base) assert.equal(f.table.get(f.bindings), next);
  }
  const full = f.table.get(f.bindings);
  assert.deepEqual(
    bundleOffsets(full),
    base.map((_, id) => id * 20),
  );
  assert.equal(f.table.snapshot().pageBundles, 5);
  assert.equal(f.table.commandCounts().draws, base.length);
});

test("page membership publishes only after upload and a changed retry cannot admit failed-only IDs", () => {
  const size = SOURCE_BUNDLE_PAGE_DRAWS,
    base = pagedDraws(size * 4),
    f = fixture(base);
  f.table.update([base[0]]);
  const first = f.table.get(f.bindings);
  f.failUpload();
  assert.throws(() => f.table.update([base[size], base[size * 3]]), /upload/);
  assert.deepEqual(f.table.snapshot().pageRevisions, [1, 0, 0, 0]);
  assert.throws(() => f.table.get(f.bindings), /Upload the complete/);
  f.table.update([base[size * 3]]);
  const next = f.table.get(f.bindings);
  assert.deepEqual(f.table.snapshot().pageRevisions, [1, 0, 0, 1]);
  assert.equal(next[0], first[0]);
  assert.deepEqual(bundleOffsets(next), [0, size * 3 * 20]);
  assert.equal(f.table.snapshot().admittedDraws, 2);
  assert.deepEqual(activeCommands(f.table), direct([base[size * 3]]));
});

test("failed later-page finish cannot publish partial replacements and retries every changed page", () => {
  const size = SOURCE_BUNDLE_PAGE_DRAWS,
    base = pagedDraws(size * 3),
    f = fixture(base),
    ids = [0, size, size * 2];
  f.table.update(ids.map((id) => base[id]));
  const original = f.table.get(f.bindings);
  const visible = ids.flatMap((id) => [base[id], base[id + 1]]);
  f.table.update(visible);
  f.failFinish(1);
  assert.throws(() => f.table.get(f.bindings), /finish failure/);
  assert.equal(f.table.snapshot().slotUpdates, 1);
  assert.equal(f.table.snapshot().slots[0].revision, 1);
  assert.equal(f.table.snapshot().pageBundles, 3);
  assert.deepEqual(
    bundleOffsets(original),
    ids.map((id) => id * 20),
  );
  const retried = f.table.get(f.bindings);
  assert.equal(retried.length, 3);
  assert.ok(retried.every((bundle, page) => bundle !== original[page]));
  assert.deepEqual(
    bundleOffsets(retried),
    visible.map((draw) => draw.sourceDrawIndex * 20),
  );
  assert.equal(f.table.snapshot().builds, 7);
  assert.equal(f.table.snapshot().slotUpdates, 2);
  assert.equal(f.table.get(f.bindings), retried);
  f.bindings.pipelineVersion++;
  const resourcesReplaced = f.table.get(f.bindings);
  assert.ok(
    resourcesReplaced.every((bundle, page) => bundle !== retried[page]),
  );
  assert.equal(f.table.snapshot().builds, 10);
});

test("eight pass slots may each retain multiple fixed pages while a ninth still fails closed", () => {
  const base = pagedDraws(SOURCE_BUNDLE_PAGE_DRAWS * 3 + 1),
    f = fixture(base);
  f.table.update(base);
  for (let slot = 0; slot < 8; slot++) {
    f.bindings.key = String(slot);
    const bundles = f.table.get(f.bindings);
    assert.equal(bundles.length, 4);
    assert.deepEqual(
      bundleOffsets(bundles),
      base.map((_, id) => id * 20),
    );
  }
  assert.equal(f.table.snapshot().bundles, 8);
  assert.equal(f.table.snapshot().pageBundles, 32);
  f.bindings.key = "overflow";
  assert.throws(() => f.table.get(f.bindings), /too many/);
});
