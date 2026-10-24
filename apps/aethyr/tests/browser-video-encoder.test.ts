import assert from "node:assert/strict";
import test from "node:test";
import { createBrowserVideoEncoder } from "../tools/film/browser-video-encoder.ts";

function fixture(settings: { unsupported?: boolean; mode?: string } = {}) {
  const previous = ["VideoEncoder", "VideoFrame"].map(
    (name) =>
      [name, Object.getOwnPropertyDescriptor(globalThis, name)] as const,
  );
  const frames: any[] = [],
    encoders: any[] = [],
    configs: any[] = [];
  class Frame {
    closed = 0;
    constructor(
      readonly data: ArrayBuffer,
      readonly options: any,
    ) {
      if (settings.mode === "frame-error") throw Error("frame failed");
      frames.push(this);
    }
    close() {
      this.closed++;
    }
  }
  class Encoder {
    static async isConfigSupported(config: any) {
      configs.push(config);
      return { supported: !settings.unsupported, config };
    }
    state = "unconfigured";
    closed = 0;
    pending: any;
    keys: boolean[] = [];
    constructor(readonly callbacks: any) {
      encoders.push(this);
      if (settings.mode === "constructor-callback")
        callbacks.error(Error("constructor failed"));
    }
    configure() {
      if (settings.mode === "configure-error") throw Error("configure failed");
      this.state = "configured";
      if (settings.mode === "configure-callback")
        this.callbacks.error(Error("configuration stopped"));
    }
    encode(frame: Frame, options: any) {
      this.pending = frame;
      this.keys.push(options.keyFrame);
      if (settings.mode === "encode-error") throw Error("encode failed");
      if (settings.mode === "async-error")
        queueMicrotask(() => this.callbacks.error(Error("device stopped")));
    }
    async flush() {
      if (settings.mode === "flush-error") throw Error("flush failed");
      if (["async-error", "pending"].includes(settings.mode ?? ""))
        return await new Promise<void>(() => {});
      if (settings.mode === "dropped") return;
      const output = (suffix: number) => {
        const bytes =
          settings.mode === "not-annexb"
            ? new Uint8Array([0, 0, 0, 2, 65, 9])
            : new Uint8Array([0, 0, 0, 1, 65, suffix]);
        this.callbacks.output({
          timestamp:
            this.pending.options.timestamp +
            (settings.mode === "wrong-timestamp" ? 1 : 0),
          type:
            this.keys.at(-1) && settings.mode !== "wrong-key" ? "key" : "delta",
          byteLength:
            settings.mode === "oversized" ? 16 * 1024 * 1024 + 1 : bytes.length,
          copyTo: (target: Uint8Array) => {
            if (settings.mode === "copy-error") throw Error("copy failed");
            target.set(bytes);
          },
        });
      };
      output(7);
      if (settings.mode === "two-chunks") output(8);
      if (settings.mode === "too-many-chunks")
        for (let i = 0; i < 64; i++) output(8);
    }
    close() {
      this.closed++;
      this.state = "closed";
      this.callbacks.error(Error("intentional close callback"));
    }
  }
  Object.defineProperty(globalThis, "VideoEncoder", {
    configurable: true,
    value: Encoder,
  });
  Object.defineProperty(globalThis, "VideoFrame", {
    configurable: true,
    value: Frame,
  });
  return {
    frames,
    encoders,
    configs,
    restore() {
      for (const [name, descriptor] of previous) {
        if (descriptor) Object.defineProperty(globalThis, name, descriptor);
        else Reflect.deleteProperty(globalThis, name);
      }
    },
  };
}
const options = {
  width: 2,
  height: 2,
  frameRate: { numerator: 60, denominator: 1 },
};
test("serialized factory is self-contained and encodes every held frame at exact rational timestamps", async () => {
  const f = fixture({ mode: "two-chunks" });
  try {
    const serialized = Function(
      `return (${createBrowserVideoEncoder.toString()})`,
    )() as typeof createBrowserVideoEncoder;
    const encoder = await serialized(options),
      pixels = new ArrayBuffer(16);
    for (let i = 0; i < 3; i++)
      assert.deepEqual(
        await encoder.encode(pixels, i),
        new Uint8Array([0, 0, 0, 1, 65, 7, 0, 0, 0, 1, 65, 8]),
      );
    assert.equal(pixels.byteLength, 16);
    assert.deepEqual(
      f.frames.map((frame) => frame.options.timestamp),
      [0, 16667, 33333],
    );
    assert.deepEqual(
      f.frames.map((frame) => frame.options.duration),
      [16667, 16666, 16667],
    );
    assert.ok(f.frames.every((frame) => frame.closed === 1));
    assert.equal(f.configs[0].avc.format, "annexb");
    assert.equal(f.configs[0].hardwareAcceleration, "prefer-hardware");
    assert.equal(f.configs[0].latencyMode, "realtime");
    assert.equal(f.configs[0].bitrate, 16_000_000);
    encoder.close();
    encoder.close();
    assert.equal(f.encoders[0].closed, 1);
    await assert.rejects(encoder.encode(pixels, 3), /closed/);
  } finally {
    f.restore();
  }
});
test("admission rejects missing support and malformed frames without allocating queued frames", async () => {
  const f = fixture();
  try {
    for (const bad of [
      { ...options, width: 1922 },
      { ...options, width: 3 },
      { ...options, height: 1082 },
      { ...options, frameRate: { numerator: 120, denominator: 1 } },
      { ...options, frameRate: { numerator: 60, denominator: 0 } },
      { ...options, bitrate: Infinity },
    ])
      await assert.rejects(createBrowserVideoEncoder(bad));
    assert.equal(f.encoders.length, 0);
    const encoder = await createBrowserVideoEncoder(options);
    await assert.rejects(
      encoder.encode(new ArrayBuffer(12), 0),
      /complete RGBA/,
    );
    await assert.rejects(encoder.encode(new ArrayBuffer(16), 1), /next output/);
    assert.equal(f.frames.length, 0);
    await encoder.encode(new ArrayBuffer(16), 0);
    encoder.close();
  } finally {
    f.restore();
  }
  const unsupported = fixture({ unsupported: true });
  try {
    await assert.rejects(createBrowserVideoEncoder(options), /cannot encode/);
    assert.equal(unsupported.encoders.length, 0);
  } finally {
    unsupported.restore();
  }
});
test("one flight is bounded and closing a pending flush rejects it and releases the encoder/frame", async () => {
  const f = fixture({ mode: "pending" });
  try {
    const encoder = await createBrowserVideoEncoder(options),
      first = encoder.encode(new ArrayBuffer(16), 0),
      check = assert.rejects(first, /closed/);
    await assert.rejects(
      encoder.encode(new ArrayBuffer(16), 1),
      /current video frame/,
    );
    encoder.close();
    await check;
    assert.equal(f.frames.length, 1);
    assert.equal(f.frames[0].closed, 1);
    assert.equal(f.encoders[0].closed, 1);
  } finally {
    f.restore();
  }
});
test("synchronous and asynchronous failures close resources and never silently drop or relabel a frame", async () => {
  for (const mode of [
    "configure-error",
    "encode-error",
    "flush-error",
    "async-error",
    "dropped",
    "wrong-timestamp",
    "not-annexb",
    "wrong-key",
    "oversized",
    "copy-error",
    "too-many-chunks",
    "frame-error",
    "constructor-callback",
    "configure-callback",
  ]) {
    const f = fixture({ mode });
    try {
      if (
        [
          "configure-error",
          "constructor-callback",
          "configure-callback",
        ].includes(mode)
      )
        await assert.rejects(createBrowserVideoEncoder(options));
      else {
        const encoder = await createBrowserVideoEncoder(options);
        await assert.rejects(encoder.encode(new ArrayBuffer(16), 0));
        await assert.rejects(encoder.encode(new ArrayBuffer(16), 1));
      }
      assert.equal(f.encoders[0].closed, 1, mode);
      assert.ok(
        f.frames.every((frame) => frame.closed === 1),
        mode,
      );
    } finally {
      f.restore();
    }
  }
});
test("30000/1001 frame durations have no accumulated timestamp drift", async () => {
  const f = fixture();
  try {
    const encoder = await createBrowserVideoEncoder({
      ...options,
      frameRate: { numerator: 30000, denominator: 1001 },
    });
    for (let i = 0; i < 4; i++) await encoder.encode(new ArrayBuffer(16), i);
    assert.deepEqual(
      f.frames.map((frame) => frame.options.timestamp),
      [0, 33367, 66733, 100100],
    );
    assert.deepEqual(
      f.frames.map((frame) => frame.options.duration),
      [33367, 33366, 33367, 33367],
    );
    encoder.close();
  } finally {
    f.restore();
  }
});
