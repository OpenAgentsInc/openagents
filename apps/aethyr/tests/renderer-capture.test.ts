import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import { DEFAULT_ENVIRONMENT } from "../packages/editor-services/protocol.ts";
import { environmentForCapture } from "../packages/renderer/film-frame.ts";
import {
  sourceModelSha256,
  type FilmSourceTransform,
} from "../packages/renderer/film-transforms.ts";
import { buildInstanceBounds } from "../packages/renderer/culling.ts";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { modelMatrix } from "../packages/renderer/math.ts";
import {
  PREVIEW_PROTOCOL,
  previewEventTransfers,
} from "../packages/editor-services/protocol.ts";

function fixture() {
  Object.assign(globalThis, {
    GPUBufferUsage: { COPY_DST: 1, MAP_READ: 2 },
    GPUMapMode: { READ: 1 },
  });
  const buffers: any[] = [],
    calls: any[] = [];
  const renderer = Object.create(Renderer.prototype) as any;
  const live = {
    camera: { eye: [1, 2, 3], yaw: 0, pitch: 0, up: [0, 1, 0], fov: 0.5 },
    env: { ...DEFAULT_ENVIRONMENT },
    time: 123,
  };
  Object.assign(renderer, {
    width: 2,
    height: 1,
    format: "bgra8unorm",
    lastView: live,
    capturing: false,
    localSignatures: new Map(),
    windShadowPlanner: { reset() {} },
    device: {
      queue: { onSubmittedWorkDone: async () => {} },
      createBuffer: ({ size }: { size: number }) => {
        const bytes = new Uint8Array(size);
        bytes.set([3, 2, 1, 17, 30, 20, 10, 19]);
        const buffer = {
          mapState: "unmapped",
          destroyed: false,
          mapAsync: async () => {
            buffer.mapState = "mapped";
          },
          getMappedRange: () => bytes.buffer,
          unmap: () => {
            buffer.mapState = "unmapped";
          },
          destroy: () => {
            buffer.destroyed = true;
          },
        };
        buffers.push(buffer);
        return buffer;
      },
    },
    render: (camera: unknown, env: unknown, time: number, capture: boolean) => {
      calls.push({ camera, env, time, capture });
      renderer.lastView = { camera, env, time };
      return true;
    },
  });
  return { renderer, buffers, calls, live };
}
test("actual capture returns raw RGBA, reuses staging and restores live camera/time", async () => {
  const { renderer, buffers, calls, live } = fixture();
  const camera = {
    eye: [7, 8, 9],
    yaw: 0.5,
    pitch: 0.2,
    up: [1, 0, 0],
    fov: 0.7,
  };
  const env = { ...DEFAULT_ENVIRONMENT, exposure: 2 };
  const before = structuredClone({ camera, env, live });
  for (const time of [0, 3, 0]) {
    const blob = await renderer.capture(time, { camera, env }, "rgba");
    assert.equal(blob.type, "application/octet-stream");
    assert.deepEqual(
      [...new Uint8Array(await blob.arrayBuffer())],
      [1, 2, 3, 255, 10, 20, 30, 255],
    );
    assert.equal(renderer.lastView, live);
    assert.equal(renderer.capturing, false);
  }
  assert.equal(buffers.length, 1);
  assert.equal(buffers[0].mapState, "unmapped");
  assert.deepEqual(
    calls.map((c) => c.time),
    [0, 3, 0],
  );
  assert.ok(
    calls.every(
      (c) => c.capture && c.camera !== camera && c.camera.up !== camera.up,
    ),
  );
  assert.deepEqual({ camera, env, live }, before);
});
test("actual capture rejects overlap and restores state on a render failure", async () => {
  const { renderer, live } = fixture();
  let release!: () => void;
  renderer.device.queue.onSubmittedWorkDone = () =>
    new Promise<void>((r) => (release = r));
  const first = renderer.capturePixels(0);
  await assert.rejects(
    renderer.capture(0, undefined, "rgba"),
    /current screenshot/,
  );
  await assert.rejects(renderer.capturePixels(0), /current screenshot/);
  release();
  await first;
  renderer.device.queue.onSubmittedWorkDone = async () => {};
  renderer.render = () => false;
  await assert.rejects(renderer.capturePixels(0), /could not start/);
  assert.equal(renderer.capturing, false);
  assert.equal(renderer.lastView, live);
  assert.equal(renderer.captureBuffer, undefined);
  await assert.rejects(renderer.capture(0, undefined, "unknown"), /PNG or raw/);
  assert.equal(renderer.capturing, false);
});

test("actual capture passes temporary exposure to the full render and subsequent frames inherit live exposure", async () => {
  const { renderer, calls, live } = fixture();
  const before = structuredClone(live);
  for (const exposure of [0.01, 64, undefined]) {
    await renderer.capture(
      0,
      {
        camera: live.camera,
        env: environmentForCapture(live.env, exposure),
      },
      "rgba",
    );
    assert.equal(renderer.lastView, live);
    assert.deepEqual(live, before);
  }
  assert.deepEqual(
    calls.map((call) => call.env.exposure),
    [0.01, 64, before.env.exposure],
  );
  assert.ok(calls.every((call) => call.env !== live.env));
});

test("fixed-time captures discard nearby live shadow anchors and invalidate caches again after success or failure", async () => {
  for (const failure of ["none", "render", "readback"] as const) {
    const { renderer, live } = fixture();
    const invalidations = { wind: 0, planar: 0 };
    renderer.windShadowPlanner = { reset: () => invalidations.wind++ };
    renderer.planar = { invalidate: () => invalidations.planar++ };
    renderer.shadowDirty = false;
    renderer.localSignatures.set("nearby-live-shadow", "old");
    renderer.render = () => {
      assert.equal(renderer.shadowDirty, true);
      assert.equal(renderer.localSignatures.size, 0);
      assert.deepEqual(invalidations, { wind: 1, planar: 1 });
      // A submitted capture consumes dirty state; live rendering must refit too.
      renderer.shadowDirty = false;
      renderer.localSignatures.set("capture-shadow", "temporary");
      if (failure === "readback")
        renderer.captureReadback.buffer.mapAsync = async () => {
          throw Error("readback failed");
        };
      return failure !== "render";
    };
    const pending = renderer.capturePixels(0);
    if (failure === "none") await pending;
    else
      await assert.rejects(
        pending,
        failure === "render" ? /could not start/ : /readback failed/,
      );
    assert.equal(renderer.lastView, live);
    assert.equal(renderer.capturing, false);
    assert.equal(renderer.shadowDirty, true);
    assert.equal(renderer.localSignatures.size, 0);
    assert.deepEqual(invalidations, { wind: 2, planar: 2 });
  }
});

async function transformedFixture() {
  const fixtureValue = fixture(),
    { renderer } = fixtureValue;
  const scene = {
    scenario: "day",
    meshes: [
      { id: "wheel", bounds: { min: [-2, -1, -0.1], max: [2, 1, 0.1] } },
    ],
    instances: [
      { mesh: 0, model: modelMatrix([10, 4, -2]) },
      { mesh: 0, model: modelMatrix([20, 4, -2]) },
    ],
  } as CompiledScene;
  const models = new Float32Array(scene.instances.flatMap((i) => i.model)),
    bounds = buildInstanceBounds(scene),
    instances = { data: new Float32Array(models) },
    variant = {
      scene,
      models,
      bounds,
      instances,
      draws: [{ firstInstance: 0, count: 2, material: 1 }],
      wind: { moving: new Uint8Array(2) },
    };
  const invalidations = { planar: 0, shadow: 0 };
  renderer.device.queue.writeBuffer = (
    buffer: any,
    offset: number,
    data: Float32Array,
  ) => buffer.data.set(data, offset / 4);
  Object.assign(renderer, {
    variants: new Map([["day", variant]]),
    instances,
    draws: [{ material: 1 }],
    lamps: [[1, 2, 3]],
    triangles: 10,
    geometryVersion: 7,
    shadowDirty: false,
    localSignatures: new Map([["light", "old"]]),
    windShadowPlanner: { reset: () => invalidations.shadow++ },
    planar: { invalidate: () => invalidations.planar++ },
    culledSources: new Map([["camera", { signature: "old", instances: {} }]]),
    frameScenes: new Map([["day", { version: 7 }]]),
  });
  const transform: FilmSourceTransform = {
    mode: "day",
    instanceIndex: 0,
    meshId: "wheel",
    baseModelSha256: await sourceModelSha256(scene.instances[0].model),
    pivot: [0, 0, 0],
    axis: [0, 0, 1],
    angleRadians: Math.PI / 2,
  };
  return { ...fixtureValue, variant, transform, invalidations };
}
test("actual capture applies source transforms to CPU/GPU/bounds and invalidates every view, then restores on success and readback failure", async () => {
  const { renderer, variant, transform, invalidations, live } =
    await transformedFixture();
  const original = {
    models: new Float32Array(variant.models),
    bounds: structuredClone(variant.bounds),
    scene: structuredClone(variant.scene),
    draws: renderer.draws,
  };
  let rendered = 0;
  renderer.render = () => {
    rendered++;
    assert.deepEqual(variant.instances.data, variant.models);
    assert.ok(Math.abs(variant.models[0]) < 1e-6);
    assert.equal(variant.models[1], 1);
    assert.equal(variant.bounds.aabbs[0], 9);
    assert.equal(renderer.geometryVersion, 8);
    assert.equal(renderer.localSignatures.size, 0);
    assert.equal(renderer.culledSources.get("camera").signature, "");
    assert.equal(renderer.frameScenes.get("day").version, -1);
    assert.ok(renderer.shadowDirty);
    renderer.culledSources.get("camera").signature = "capture pose";
    // A real render refreshes the resident core to the temporary film version.
    renderer.frameScenes.get("day").version = 8;
    return true;
  };
  const restored = () => {
    assert.deepEqual(variant.models, original.models);
    assert.deepEqual(variant.instances.data, original.models);
    assert.deepEqual(variant.bounds, original.bounds);
    assert.deepEqual(variant.scene, original.scene);
    assert.equal(renderer.geometryVersion, 7);
    assert.equal(renderer.draws, original.draws);
    assert.equal(renderer.lastView, live);
    assert.equal(renderer.culledSources.get("camera").signature, "");
    assert.equal(renderer.frameScenes.get("day").version, -1);
    assert.equal(renderer.capturing, false);
  };
  await renderer.capture(0, undefined, "rgba", [transform]);
  restored();
  await renderer.capturePixels(0, undefined, [transform]);
  restored();
  renderer.captureReadback.buffer.mapAsync = async () => {
    throw Error("readback stopped");
  };
  await assert.rejects(
    renderer.capture(3, undefined, "rgba", [transform]),
    /readback stopped/,
  );
  restored();
  assert.equal(rendered, 3);
  assert.equal(invalidations.planar, 6);
  assert.equal(invalidations.shadow, 6);
});
test("600 mocked GPU readbacks reuse staging, match legacy RGBA and transfer twice without constructing any Blob or canvas", async () => {
  const { renderer, buffers, live } = fixture(),
    blob = await renderer.capture(0, undefined, "rgba"),
    expected = new Uint8Array(await blob.arrayBuffer()),
    oldBlob = Object.getOwnPropertyDescriptor(globalThis, "Blob"),
    oldCanvas = Object.getOwnPropertyDescriptor(globalThis, "OffscreenCanvas");
  Object.defineProperty(globalThis, "Blob", {
    configurable: true,
    value: class {
      constructor() {
        throw Error("Raw capture created a Blob");
      }
    },
  });
  Object.defineProperty(globalThis, "OffscreenCanvas", {
    configurable: true,
    value: class {
      constructor() {
        throw Error("Raw capture created an image canvas");
      }
    },
  });
  try {
    for (let i = 0; i < 600; i++) {
      const pixels: ArrayBuffer = await renderer.capturePixels(i % 4),
        worker = {
          type: "capturePixels",
          protocol: PREVIEW_PROTOCOL,
          sessionId: "owned",
          requestId: String(i),
          pixels,
        },
        preview = structuredClone(worker, {
          transfer: previewEventTransfers(worker),
        }),
        editor = structuredClone(preview, {
          transfer: previewEventTransfers(preview),
        });
      assert.equal(pixels.byteLength, 0);
      assert.equal(preview.pixels.byteLength, 0);
      assert.deepEqual(new Uint8Array(editor.pixels), expected);
      assert.equal(renderer.lastView, live);
      assert.equal(renderer.capturing, false);
    }
    assert.equal(buffers.length, 1);
    assert.equal(buffers[0].mapState, "unmapped");
  } finally {
    if (oldBlob) Object.defineProperty(globalThis, "Blob", oldBlob);
    else Reflect.deleteProperty(globalThis, "Blob");
    if (oldCanvas)
      Object.defineProperty(globalThis, "OffscreenCanvas", oldCanvas);
    else Reflect.deleteProperty(globalThis, "OffscreenCanvas");
  }
});
test("source admission and upload failure cannot leave a partial transform or hide the original error", async () => {
  const { renderer, variant, transform } = await transformedFixture();
  const models = new Float32Array(variant.models),
    bounds = structuredClone(variant.bounds);
  variant.wind.moving[0] = 1;
  await assert.rejects(
    renderer.capture(0, undefined, "rgba", [transform]),
    /fixed source mesh/,
  );
  variant.wind.moving[0] = 0;
  variant.draws[0].firstInstance = 1;
  await assert.rejects(
    renderer.capture(0, undefined, "rgba", [transform]),
    /drawn/,
  );
  variant.draws[0].firstInstance = 0;
  await assert.rejects(
    renderer.capture(0, undefined, "rgba", [
      { ...transform, baseModelSha256: "0".repeat(64) },
    ]),
    /changed/,
  );
  renderer.device.queue.writeBuffer = () => {
    throw Error("original upload error");
  };
  await assert.rejects(
    renderer.capture(0, undefined, "rgba", [transform]),
    /original upload error/,
  );
  assert.deepEqual(variant.models, models);
  assert.deepEqual(variant.bounds, bounds);
  assert.equal(renderer.capturing, false);
});
