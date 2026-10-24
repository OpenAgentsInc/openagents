import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import type { CompiledScene } from "../packages/assets/src/scene.ts";

class Device {
  queue = { onSubmittedWorkDone: async () => {} };
  destroys = 0;
  listeners: Array<(event: { error: { message: string } }) => void> = [];
  resolveLoss!: (info: { reason: string; message: string }) => void;
  lost = new Promise<{ reason: string; message: string }>((resolve) => {
    this.resolveLoss = resolve;
  });
  addEventListener(_name: string, listener: (typeof this.listeners)[number]) {
    this.listeners.push(listener);
  }
  error(message: string) {
    this.listeners.forEach((listener) => listener({ error: { message } }));
  }
  destroy() {
    this.destroys++;
    this.error("Late validation error during shutdown");
    this.resolveLoss({ reason: "destroyed", message: "Device was destroyed." });
  }
}

async function fixture(
  initialize: (this: any) => Promise<void>,
  run: (fixture: {
    device: Device;
    create: (report: (message: string) => void) => Promise<Renderer>;
    unconfigures: () => number;
  }) => Promise<void>,
  configure: () => void = () => {},
) {
  const device = new Device(),
    savedNavigator = Object.getOwnPropertyDescriptor(globalThis, "navigator"),
    savedUsage = Object.getOwnPropertyDescriptor(globalThis, "GPUTextureUsage"),
    prototype = Renderer.prototype as any,
    savedInitialize = prototype.initialize;
  let unconfigures = 0;
  const context = {
    configure,
    unconfigure() {
      unconfigures++;
    },
  };
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: {
      gpu: {
        requestAdapter: async () => ({
          features: new Set(),
          requestDevice: async () => device,
        }),
        getPreferredCanvasFormat: () => "bgra8unorm",
      },
    },
  });
  Object.defineProperty(globalThis, "GPUTextureUsage", {
    configurable: true,
    value: { RENDER_ATTACHMENT: 16, COPY_SRC: 1 },
  });
  prototype.initialize = initialize;
  try {
    await run({
      device,
      create: (report) =>
        Renderer.create(
          { getContext: () => context } as unknown as OffscreenCanvas,
          {} as CompiledScene,
          report,
        ),
      unconfigures: () => unconfigures,
    });
  } finally {
    prototype.initialize = savedInitialize;
    for (const [key, saved] of [
      ["navigator", savedNavigator],
      ["GPUTextureUsage", savedUsage],
    ] as const) {
      if (saved) Object.defineProperty(globalThis, key, saved);
      else Reflect.deleteProperty(globalThis, key);
    }
  }
}

// A lost promise resolves after destroy; drain it before inspecting reported status.
const settle = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

test("initialization failure survives cleanup and late graphics notifications", async () => {
  const failure = new Error("A required material could not open."),
    reports: string[] = [];
  let renderer: any;
  await fixture(
    async function () {
      renderer = this;
      throw failure;
    },
    async ({ create, device, unconfigures }) => {
      await assert.rejects(
        create((message) => reports.push(message)),
        (e) => e === failure,
      );
      await settle();
      device.error("A delayed GPU error");
      renderer.dispose();
      assert.equal(device.destroys, 1);
      assert.equal(unconfigures(), 1);
      assert.deepEqual(reports, []);
      assert.equal(renderer.lost, false);
      assert.equal(renderer.render({}, {}, 0), false);
      assert.doesNotThrow(() => renderer.resize(100, 100));
    },
  );
});

test("intentional repeated shutdown is silent and prevents queued shader activation", async () => {
  const reports: string[] = [];
  await fixture(
    async function () {},
    async ({ create, device, unconfigures }) => {
      const renderer = await create((message) => reports.push(message)),
        pendingShader = renderer.replaceShader("Queued before shutdown");
      renderer.dispose();
      renderer.dispose();
      await settle();
      assert.equal(device.destroys, 1);
      assert.equal(unconfigures(), 1);
      assert.equal(await pendingShader, false);
      assert.equal(renderer.lost, false);
      assert.deepEqual(reports, []);
      assert.equal(
        await renderer.replaceShader("A disposed renderer has no shader work."),
        false,
      );
    },
  );
});

test("unexpected live device errors and loss still reach the caller", async () => {
  const reports: string[] = [];
  await fixture(
    async function () {},
    async ({ create, device }) => {
      const renderer = await create((message) => reports.push(message));
      device.error("Invalid binding");
      device.resolveLoss({ reason: "unknown", message: "Driver stopped." });
      await settle();
      assert.equal(renderer.lost, true);
      assert.deepEqual(reports, [
        "Graphics: Invalid binding",
        "Graphics device stopped: Driver stopped.",
      ]);
      renderer.dispose();
      await settle();
      assert.equal(reports.length, 2);
    },
  );
});

test("canvas setup failure releases the device and preserves its own error", async () => {
  const failure = new Error("Canvas configuration failed.");
  await fixture(
    async function () {
      assert.fail("Initialization must not start after canvas setup fails.");
    },
    async ({ create, device, unconfigures }) => {
      const reports: string[] = [];
      await assert.rejects(
        create((message) => reports.push(message)),
        (e) => e === failure,
      );
      await settle();
      assert.equal(device.destroys, 1);
      assert.equal(unconfigures(), 1);
      assert.deepEqual(reports, []);
    },
    () => {
      throw failure;
    },
  );
});

test("a failing resource finalizer cannot replace the startup failure or leak its device", async () => {
  const failure = new Error("Original startup failure.");
  await fixture(
    async function () {
      this.planar = {
        dispose() {
          throw new Error("Cleanup failed.");
        },
      };
      throw failure;
    },
    async ({ create, device, unconfigures }) => {
      const reports: string[] = [];
      await assert.rejects(
        create((message) => reports.push(message)),
        (e) => e === failure,
      );
      await settle();
      assert.equal(device.destroys, 1);
      assert.equal(unconfigures(), 1);
      assert.deepEqual(reports, []);
    },
  );
});
