import assert from "node:assert/strict";
import test from "node:test";
import { EditorService } from "../packages/editor-services/service.ts";
import {
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
} from "../packages/editor-services/protocol.ts";
import { editorAtoms } from "../packages/editor-services/state.ts";

test("service navigation, capture errors, cancellation and film acknowledgements work without downloads", async () => {
  const oldWindow = Object.getOwnPropertyDescriptor(globalThis, "window"),
    oldDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  const listeners = new Map<string, Set<(event: any) => void>>(),
    sent: any[] = [];
  let downloads = 0;
  const windowStub = {
    location: { origin: "http://editor.test" },
    addEventListener: (name: string, fn: (event: any) => void) => {
      const held = listeners.get(name) ?? new Set();
      held.add(fn);
      listeners.set(name, held);
    },
    removeEventListener: (name: string, fn: (event: any) => void) =>
      listeners.get(name)?.delete(fn),
  };
  const contentWindow = {
    postMessage: (message: unknown) => sent.push(message),
  };
  const frame = {
    src: "",
    contentWindow,
    addEventListener: () => {},
    removeEventListener: () => {},
    focus: () => {},
  };
  Object.defineProperty(globalThis, "window", {
    value: windowStub,
    configurable: true,
  });
  Object.defineProperty(globalThis, "document", {
    value: {
      createElement: () => {
        downloads++;
        throw Error("An unrequested download occurred");
      },
    },
    configurable: true,
  });
  const service = new EditorService(
    frame as unknown as HTMLIFrameElement,
    "http://preview.test/preview.html",
  );
  const emit = (data: Record<string, unknown>) => {
    for (const fn of listeners.get("message") ?? [])
      fn({
        origin: "http://preview.test",
        source: contentWindow,
        data: {
          protocol: PREVIEW_PROTOCOL,
          sessionId: service.sessionId,
          ...data,
        },
      });
  };
  try {
    service.start();
    await new Promise<void>((r) => setImmediate(r));
    assert.ok(listeners.get("message")?.size);
    const beforeNavigation = sent.length;
    for (const perspective of ["overhead", null, 1])
      assert.equal(service.perspective(perspective as any), undefined);
    for (const distance of [2.49, 40.01, NaN, Infinity])
      assert.equal(service.orbitDistance(distance), undefined);
    assert.equal(sent.length, beforeNavigation);
    assert.equal(service.perspective("first"), sent.at(-1).requestId);
    assert.equal(sent.at(-1).type, "perspective");
    assert.equal(sent.at(-1).perspective, "first");
    assert.equal(service.registry.get(editorAtoms.status).character, undefined);
    assert.equal(service.orbitDistance(6), sent.at(-1).requestId);
    assert.equal(sent.at(-1).type, "orbitDistance");
    assert.equal(sent.at(-1).distance, 6);
    const beforeSwarm = sent.length;
    for (const count of [-1, 1, 200001, NaN, "100000"])
      assert.equal(service.spellSwarm(count as never), undefined);
    assert.equal(sent.length, beforeSwarm);
    for (const count of [0, 10000, 100000, 200000] as const) {
      const requestId = service.spellSwarm(count);
      assert.equal(sent.at(-1).type, "spellSwarm");
      assert.equal(sent.at(-1).count, count);
      assert.equal(sent.at(-1).requestId, requestId);
      emit({
        type: "receipt",
        requestId,
        ok: true,
        message: "Applied",
        revision: 0,
      });
    }
    emit({
      type: "status",
      status: {
        ...INITIAL_STATUS,
        view: { eye: [0, 1.62, 0], target: [0, 1.62, -1], mode: "walk" },
        character: {
          perspective: "first",
          position: [0, 0, 0],
          velocity: [0, 0, 0],
          grounded: true,
          animation: "idle",
          orbitDistance: 6,
        },
      },
    });
    assert.equal(service.registry.get(editorAtoms.mode), "walk");
    assert.equal(
      service.registry.get(editorAtoms.status).character?.perspective,
      "first",
    );
    const beforeCamera = sent.length;
    assert.equal(service.camera("MT-04"), undefined);
    assert.equal(sent.length, beforeCamera);
    const cameraRequest = service.camera("MT-03");
    assert.equal(cameraRequest, sent.at(-1).requestId);
    emit({
      type: "receipt",
      requestId: cameraRequest,
      ok: true,
      message: "Camera MT-03",
      revision: 0,
    });
    const options = {
      time: 0,
      exposure: 16,
      transforms: [
        {
          mode: "day" as const,
          instanceIndex: 0,
          meshId: "owned-wheel",
          baseModelSha256: "0".repeat(64),
          pivot: [0, 0, 0] as [number, number, number],
          axis: [0, 0, 1] as [number, number, number],
          angleRadians: 0.3,
        },
      ],
      camera: {
        eye: [0, 1, 0] as [number, number, number],
        target: [0, 1, -1] as [number, number, number],
        up: [1, 0, 0] as [number, number, number],
        fov: 0.5,
      },
      format: "rgba" as const,
    };
    const blob = new Blob([new Uint8Array(8)], {
      type: "application/octet-stream",
    });
    const first = service.captureBlob(options);
    const request = sent.at(-1);
    assert.deepEqual(
      {
        ...request,
        protocol: undefined,
        sessionId: undefined,
        requestId: undefined,
      },
      {
        type: "capture",
        ...options,
        protocol: undefined,
        sessionId: undefined,
        requestId: undefined,
      },
    );
    await assert.rejects(service.captureBlob(), /current screenshot/);
    emit({ type: "capture", requestId: request.requestId, blob });
    assert.equal(await first, blob);
    assert.equal(downloads, 0);
    const beforeInvalid = sent.length;
    await assert.rejects(
      service.captureBlob({ exposure: 64.1 }),
      /frame exposure/,
    );
    assert.equal(sent.length, beforeInvalid);
    const { format: _format, ...pixelOptions } = options;
    await assert.rejects(service.capturePixels({ format: "rgba" } as any));
    assert.equal(sent.length, beforeInvalid);
    const oldBlob = Object.getOwnPropertyDescriptor(globalThis, "Blob");
    Object.defineProperty(globalThis, "Blob", {
      configurable: true,
      value: class {
        constructor() {
          throw Error("Pixel service created a Blob");
        }
      },
    });
    try {
      const raw = service.capturePixels(pixelOptions),
        rawRequest = sent.at(-1),
        pixels = new Uint8Array([1, 2, 3, 255]).buffer;
      assert.equal(rawRequest.type, "capturePixels");
      assert.equal(rawRequest.exposure, 16);
      assert.deepEqual(rawRequest.transforms, pixelOptions.transforms);
      await assert.rejects(service.captureBlob(), /current screenshot/);
      await assert.rejects(service.capturePixels(), /current screenshot/);
      emit({ type: "capturePixels", requestId: rawRequest.requestId, pixels });
      assert.equal(await raw, pixels);
      const cancelledPixels = service.capturePixels(),
        cancelledId = sent.at(-1).requestId,
        check = assert.rejects(cancelledPixels, /cancelled/);
      service.cancelCapture();
      await check;
      emit({ type: "capturePixels", requestId: cancelledId, pixels });
      assert.equal(downloads, 0);
    } finally {
      if (oldBlob) Object.defineProperty(globalThis, "Blob", oldBlob);
      else Reflect.deleteProperty(globalThis, "Blob");
    }
    const failed = service.captureBlob();
    const failedId = sent.at(-1).requestId;
    const rejection = assert.rejects(failed, /owned failure/);
    emit({ type: "error", requestId: failedId, message: "owned failure" });
    await rejection;
    emit({ type: "capture", requestId: failedId, blob });
    assert.equal(downloads, 0);
    const cancelled = service.captureBlob();
    const cancelId = sent.at(-1).requestId;
    const cancelledCheck = assert.rejects(cancelled, /cancelled/);
    service.cancelCapture();
    await cancelledCheck;
    emit({ type: "capture", requestId: cancelId, blob });
    assert.equal(downloads, 0);
    const begin = service.beginFilmCapture({ width: 1280, height: 720 });
    const beginId = sent.at(-1).requestId;
    assert.equal(sent.at(-1).type, "beginFilmCapture");
    emit({
      type: "receipt",
      requestId: beginId,
      ok: true,
      message: "Film capture ready",
      revision: 0,
    });
    await begin;
    const end = service.endFilmCapture();
    const endId = sent.at(-1).requestId;
    assert.equal(sent.at(-1).type, "endFilmCapture");
    emit({
      type: "receipt",
      requestId: endId,
      ok: true,
      message: "Film capture finished",
      revision: 0,
    });
    await end;
    const final = service.captureBlob();
    const closeCheck = assert.rejects(final, /world closed/);
    service.close();
    await closeCheck;
    assert.equal(downloads, 0);
  } finally {
    service.close();
    await new Promise<void>((r) => setImmediate(r));
    if (oldWindow) Object.defineProperty(globalThis, "window", oldWindow);
    else Reflect.deleteProperty(globalThis, "window");
    if (oldDocument) Object.defineProperty(globalThis, "document", oldDocument);
    else Reflect.deleteProperty(globalThis, "document");
  }
});
