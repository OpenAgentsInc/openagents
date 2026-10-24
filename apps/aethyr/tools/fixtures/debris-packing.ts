import {
  DebrisGpuPacking,
  DEBRIS_PACKING_WGSL,
  type PackingPart,
  type PackingTarget,
  type PackingDraw,
} from "../../packages/renderer/debris-gpu-packing.ts";
import type { FractureChunk } from "../../packages/renderer/source-fracture.ts";

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
function chunk(count: number, seed: number): FractureChunk {
  const vertices = new Float32Array(count * 12),
    colors = new Uint8Array(count * 4),
    uv1 = new Float32Array(count * 2);
  for (let i = 0; i < count; i++) {
    vertices.set(
      [
        Math.sin(i * 0.031 + seed) * 8,
        Math.cos(i * 0.017) * 4,
        (i % 97) * 0.07 - 3,
        0.3,
        -0.4,
        0.8660254,
        (i % 13) / 12,
        (i % 17) / 16,
        -0.8,
        0.6,
        0,
        i % 2 ? -1 : 1,
      ],
      i * 12,
    );
    colors.set([i % 256, seed % 256, (i * 7) % 256, 255], i * 4);
    uv1.set([(i % 23) / 22, (i % 31) / 30], i * 2);
  }
  const draws = [];
  let first = 0;
  for (let material = 0; material < 8; material++) {
    const countHere =
      material === 7 ? count - first : Math.floor(count / 8 / 3) * 3;
    draws.push({
      material: (seed * 7 + material) % 32,
      firstIndex: first,
      indexCount: countHere,
    });
    first += countHere;
  }
  return {
    vertices,
    colors,
    uv1,
    center: [19990.123456789, -19990.987654321, 0.1234567890123],
    draws,
  };
}
function matrix(seed: number): Float32Array {
  const x = seed * 0.017,
    y = seed * -0.023,
    z = seed * 0.013,
    cx = Math.cos(x),
    sx = Math.sin(x),
    cy = Math.cos(y),
    sy = Math.sin(y),
    cz = Math.cos(z),
    sz = Math.sin(z);
  return new Float32Array([
    cz * cy,
    sz * cy,
    -sy,
    0,
    cz * sy * sx - sz * cx,
    sz * sy * sx + cz * cx,
    cy * sx,
    0,
    cz * sy * cx + sz * sx,
    sz * sy * cx - cz * sx,
    cy * cx,
    0,
    seed % 2 ? 19900 : -19900,
    seed % 3 ? -19900 : 19900,
    seed * 0.1,
    1,
  ]);
}
async function prepare(
  packer: DebrisGpuPacking,
  chunks: { chunk: FractureChunk; world: boolean }[],
) {
  const steps = packer.prepare(chunks, () => false);
  let count = 0;
  for (;;) {
    const step = steps.next();
    if (step.done) {
      assert(step.value, "Geometry preparation failed.");
      return;
    }
    if (++count % 32 === 0)
      await new Promise((resolve) => setTimeout(resolve, 0));
  }
}
function grouped(parts: readonly PackingPart[]) {
  const groups = new Map<
    number,
    { part: PackingPart; first: number; count: number }[]
  >();
  for (const part of parts)
    for (const draw of part.chunk.draws) {
      const list = groups.get(draw.material) ?? [];
      list.push({ part, first: draw.firstIndex, count: draw.indexCount });
      groups.set(draw.material, list);
    }
  return groups;
}
function compare(
  parts: readonly PackingPart[],
  data: ArrayBuffer,
  count: number,
  draws: PackingDraw[],
) {
  const vertices = new Float32Array(data, 0, count * 12),
    colorOffset = count * 48,
    colors = new Uint8Array(data, colorOffset, count * 4),
    uv1 = new Float32Array(data, colorOffset + count * 4, count * 2);
  const expectedDraws: PackingDraw[] = [];
  let output = 0,
    maxPositionError = 0,
    maxDirectionError = 0,
    transformed = 0;
  for (const [material, items] of grouped(parts))
    for (const { part, first, count: length } of items) {
      const previous = expectedDraws.at(-1);
      if (
        previous &&
        previous.material === material &&
        previous.instance === part.instance
      )
        previous.indexCount += length;
      else
        expectedDraws.push({
          material,
          instance: part.instance,
          firstIndex: output,
          indexCount: length,
        });
      for (let i = first; i < first + length; i++, output++) {
        const input = part.chunk.vertices,
          m = part.transform;
        for (let attribute = 0; attribute < 12; attribute++) {
          let value = input[i * 12 + attribute];
          if (part.world && attribute < 3)
            value = Math.fround(value + part.chunk.center[attribute]);
          const vector =
            attribute < 3
              ? 0
              : attribute >= 3 && attribute < 6
                ? 3
                : attribute >= 8 && attribute < 11
                  ? 8
                  : -1;
          if (m && vector >= 0) {
            const axis = attribute - vector;
            value = Math.fround(
              m[axis] * input[i * 12 + vector] +
                m[axis + 4] * input[i * 12 + vector + 1] +
                m[axis + 8] * input[i * 12 + vector + 2] +
                (vector === 0 ? m[axis + 12] : 0),
            );
          }
          const actual = vertices[output * 12 + attribute];
          assert(Number.isFinite(actual), "A packed GPU value is not finite.");
          const error = Math.abs(actual - value);
          if (m && vector >= 0) {
            const tolerance =
              vector === 0
                ? 0.004 + Math.abs(value) * 2e-7
                : 2e-6 + Math.abs(value) * 2e-7;
            assert(
              error <= tolerance,
              `GPU rubble error ${error} exceeds ${tolerance} at vertex ${output}, attribute ${attribute}.`,
            );
            if (vector === 0)
              maxPositionError = Math.max(maxPositionError, error);
            else maxDirectionError = Math.max(maxDirectionError, error);
          } else
            assert(
              Object.is(actual, value),
              `Copied field differs at vertex ${output}, attribute ${attribute}.`,
            );
        }
        for (let a = 0; a < 4; a++)
          assert(
            colors[output * 4 + a] === part.chunk.colors[i * 4 + a],
            "Packed vertex color changed.",
          );
        for (let a = 0; a < 2; a++)
          assert(
            Object.is(uv1[output * 2 + a], part.chunk.uv1[i * 2 + a]),
            "Packed secondary UV changed.",
          );
        if (m) transformed++;
      }
    }
  assert(output === count, "The packed vertex count changed.");
  assert(
    JSON.stringify(
      draws.map((draw) => [
        draw.material,
        draw.firstIndex,
        draw.indexCount,
        draw.instance,
      ]),
    ) ===
      JSON.stringify(
        expectedDraws.map((draw) => [
          draw.material,
          draw.firstIndex,
          draw.indexCount,
          draw.instance,
        ]),
      ),
    "The material/source draw order changed.",
  );
  return {
    count,
    draws: draws.length,
    transformed,
    copied: count - transformed,
    maxPositionError,
    maxDirectionError,
  };
}

/** Standalone compute/readback qualification; no scene, canvas, animation, or render callbacks. */
export async function runDebrisPackingGpuProbe() {
  assert(navigator.gpu, "WebGPU is unavailable.");
  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: "high-performance",
  });
  assert(adapter, "A WebGPU adapter is unavailable.");
  const device = await adapter.requestDevice(),
    errors: string[] = [],
    buffers: GPUBuffer[] = [];
  let accounted = 0;
  device.addEventListener("uncapturederror", (event) =>
    errors.push(event.error.message),
  );
  device.pushErrorScope("validation");
  let packer: DebrisGpuPacking | undefined;
  const results: unknown[] = [];
  const target = (count: number): PackingTarget => {
    const buffer = (size: number) => {
      const value = device.createBuffer({
        size,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      });
      buffers.push(value);
      return value;
    };
    return {
      vertices: buffer(count * 48),
      colors: buffer(count * 4),
      uv1: buffer(count * 8),
    };
  };
  async function read(
    parts: PackingPart[],
    destination: PackingTarget,
    capacity: number,
    abandon = false,
  ) {
    const count = parts.reduce(
      (n, part) =>
        n + part.chunk.draws.reduce((sum, draw) => sum + draw.indexCount, 0),
      0,
    );
    const staged = packer!.stage(parts, destination, capacity);
    assert(staged, "The fixture unexpectedly used CPU fallback.");
    if (abandon) {
      packer!.encode(device.createCommandEncoder());
      assert(
        packer!.snapshot().pendingTargets > 0,
        "Abandoned encoding consumed its packing plan.",
      );
    }
    const encoder = device.createCommandEncoder();
    packer!.encode(encoder);
    const readback = device.createBuffer({
      size: Math.max(4, count * 60),
      usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
    });
    buffers.push(readback);
    if (count) {
      encoder.copyBufferToBuffer(
        destination.vertices,
        0,
        readback,
        0,
        count * 48,
      );
      encoder.copyBufferToBuffer(
        destination.colors,
        0,
        readback,
        count * 48,
        count * 4,
      );
      encoder.copyBufferToBuffer(
        destination.uv1,
        0,
        readback,
        count * 52,
        count * 8,
      );
    }
    device.queue.submit([encoder.finish()]);
    packer!.submitted();
    await readback.mapAsync(GPUMapMode.READ);
    const bytes = readback.getMappedRange().slice(0);
    readback.unmap();
    return compare(parts, bytes, count, staged.draws);
  }
  try {
    packer = await DebrisGpuPacking.create(device, (delta) => {
      accounted += delta;
    });
    const pages = [chunk(196608, 1), chunk(196608, 2), chunk(196608, 3)];
    await prepare(
      packer,
      pages.map((chunk) => ({ chunk, world: true })),
    );
    assert(
      packer.snapshot().arenaBytes > 32 * 1024 * 1024,
      "The page-split test did not cross an arena page.",
    );
    results.push({
      case: "bit-exact-world-copy-page-and-material-splits-abandoned-retry",
      ...(await read(
        [0, 2].map((index) => ({
          chunk: pages[index],
          world: true,
          instance: 0,
        })),
        target(393216),
        393216,
        true,
      )),
    });
    packer.retain([]);
    assert(
      packer.snapshot().arenaBytes === 0,
      "Clearing unused geometry retained its arenas.",
    );
    const moving = chunk(1023, 4),
      rubble = chunk(384, 5),
      whole = chunk(831, 6),
      tail = chunk(1086, 7);
    const resident = [
      { chunk: moving, world: false },
      { chunk: rubble, world: false },
      { chunk: whole, world: true },
      { chunk: tail, world: true },
    ];
    await prepare(packer, resident);
    const standingParts = Array.from({ length: 256 }, (_, index) => ({
      chunk: index === 255 ? tail : whole,
      world: true,
      instance: 0,
    }));
    const movingParts = Array.from({ length: 256 }, (_, instance) => ({
      chunk: moving,
      world: false,
      instance,
    }));
    const rubbleParts = Array.from({ length: 1024 }, (_, index) => ({
      chunk: rubble,
      world: false,
      instance: 0,
      transform: matrix(index),
    }));
    results.push({
      case: "standing-capacity-212991",
      ...(await read(standingParts, target(212992), 212992)),
    });
    results.push({
      case: "256-moving-bodies-261888-bit-exact",
      ...(await read(movingParts, target(262144), 262144)),
    });
    results.push({
      case: "1024-persistent-bodies-393216-transformed",
      ...(await read(rubbleParts, target(393216), 393216)),
    });
    const before = packer.snapshot().residentChunks,
      cancelled = chunk(4096, 8);
    let stop = false;
    const steps = packer.prepare(
      [...resident, { chunk: cancelled, world: false }],
      () => stop,
    );
    assert(
      steps.next().done === false,
      "Cancellation did not enter a new chunk.",
    );
    stop = true;
    assert(
      steps.next().value === false,
      "Cancellation accepted a partial chunk.",
    );
    assert(
      packer.snapshot().residentChunks === before,
      "Cancellation changed resident geometry.",
    );
    results.push({
      case: "retry-after-cancel",
      ...(await read(
        [{ chunk: moving, world: false, instance: 0 }],
        target(1023),
        1023,
      )),
    });
    const emptyTarget = target(3);
    const empty = packer.stage([], emptyTarget, 3);
    assert(empty?.draws.length === 0, "An empty plan emitted draws.");
    const emptyEncoder = device.createCommandEncoder();
    packer.encode(emptyEncoder);
    device.queue.submit([emptyEncoder.finish()]);
    packer.submitted();
    assert(
      packer.snapshot().pendingTargets === 0,
      "An empty packing plan stayed pending.",
    );
    const stats = packer.snapshot();
    assert(
      stats.cpuFallbacks === 0,
      "A GPU qualification case used CPU fallback.",
    );
    await device.queue.onSubmittedWorkDone();
    const validation = await device.popErrorScope();
    assert(!validation, validation?.message ?? "GPU validation failed.");
    assert(!errors.length, errors.join("\n"));
    packer.dispose();
    packer.dispose();
    assert(accounted === 0, "Disposal retained accounted GPU memory.");
    const shaderSha256 = Array.from(
      new Uint8Array(
        await crypto.subtle.digest(
          "SHA-256",
          new TextEncoder().encode(DEBRIS_PACKING_WGSL),
        ),
      ),
    )
      .map((value) => value.toString(16).padStart(2, "0"))
      .join("");
    return {
      schema: "aethyrforge.debris-packing-gpu-probe.v1",
      ok: true,
      adapter: {
        vendor: adapter.info.vendor,
        architecture: adapter.info.architecture,
        description: adapter.info.description,
      },
      shaderSha256,
      scope: "standalone-compute-readback-no-rendering",
      copiedRecords: "bit-exact",
      rubbleReference: "original-JSf64-expression-then-f32-store",
      positionTolerance: { absolute: 0.004, relative: 2e-7 },
      directionTolerance: { absolute: 2e-6, relative: 2e-7 },
      results,
      stats,
      errors,
    };
  } finally {
    packer?.dispose();
    for (const buffer of buffers) buffer.destroy();
    device.destroy();
  }
}
