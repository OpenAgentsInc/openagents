import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  SpellSwarmHost,
  type SpellSwarmHostContext,
  type SpellSwarmHostOptions,
} from "../packages/renderer/spell-swarm-host.ts";

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function candidate(bytes = 256) {
  return {
    count: 100_000,
    steps: [] as { dt: number; time: number }[],
    preparations: 0,
    submissions: 0,
    renderEnabled: true,
    encodeSimulation(_encoder: unknown, dt: number, time: number) {
      this.steps.push({ dt, time });
    },
    prepareRender() {
      this.preparations++;
    },
    render() {
      return this.renderEnabled;
    },
    afterSubmit() {
      this.submissions++;
    },
    bytes,
    disposals: 0,
    dispose() {
      this.disposals++;
    },
    inspect() {
      return { bytes: this.bytes };
    },
    async probe() {
      return { finite: true };
    },
  };
}

type Candidate = ReturnType<typeof candidate>;
function fixture(options: SpellSwarmHostOptions = {}) {
  const renderer: any = new Renderer();
  const creations: {
    count: number;
    center: number[];
    result: ReturnType<typeof deferred<Candidate>>;
  }[] = [];
  const drains: ReturnType<typeof deferred<void>>[] = [];
  let deviceDisposals = 0;
  renderer.gpuBytes = 1000;
  renderer.device = {
    queue: {
      onSubmittedWorkDone() {
        const drain = deferred();
        drains.push(drain);
        return drain.promise;
      },
    },
    destroy() {
      deviceDisposals++;
    },
  };
  renderer.features = {
    createSpellSwarmHost(context: SpellSwarmHostContext) {
      return new SpellSwarmHost(context, options, ((
        _device: unknown,
        options: { count: number; center: number[] },
      ) => {
        const result = deferred<Candidate>();
        creations.push({ ...options, result });
        return result.promise;
      }) as any);
    },
  };
  return {
    renderer,
    creations,
    drains,
    deviceDisposals: () => deviceDisposals,
  };
}

const settle = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

async function activate(
  f: ReturnType<typeof fixture>,
  swarm: Candidate,
  count = 100000,
) {
  const activation = f.renderer.setSpellSwarm(count);
  f.creations.at(-1)!.result.resolve(swarm);
  await settle();
  f.drains.at(-1)!.resolve();
  await activation;
}

test("a candidate replaces the active swarm only after the old queue work drains", async () => {
  const f = fixture(),
    old = candidate(120),
    next = candidate(240);
  await activate(f, old);
  f.renderer.spellSwarmHost.time = 5;
  assert.equal(f.renderer.gpuBytes, 1120);
  const activation = f.renderer.setSpellSwarm(200000);
  assert.equal(f.renderer.spellSwarmHost.current, old);
  f.creations[1].result.resolve(next);
  await settle();
  assert.equal(f.renderer.spellSwarmHost.current, old);
  assert.equal(old.disposals, 0);
  assert.equal(next.disposals, 0);
  assert.equal(f.renderer.gpuBytes, 1120);
  f.drains[1].resolve();
  await activation;
  assert.equal(f.renderer.spellSwarmHost.current, next);
  assert.equal(f.renderer.spellSwarmHost.time, undefined);
  assert.equal(f.renderer.gpuBytes, 1240);
  assert.equal(old.disposals, 1);
  assert.equal(next.disposals, 0);
});

test("a rejected queue drain releases the candidate and preserves the active swarm", async () => {
  const f = fixture(),
    old = candidate(120),
    next = candidate(240);
  await activate(f, old);
  const activation = f.renderer.setSpellSwarm(200000);
  f.creations[1].result.resolve(next);
  await settle();
  const failure = new Error("Device queue failed.");
  f.drains[1].reject(failure);
  await assert.rejects(activation, (error) => error === failure);
  assert.equal(next.disposals, 1);
  assert.equal(old.disposals, 0);
  assert.equal(f.renderer.spellSwarmHost.current, old);
  assert.equal(f.renderer.gpuBytes, 1120);
});

test("failed preparation leaves the active swarm and its memory accounting intact", async () => {
  const f = fixture(),
    old = candidate(120);
  await activate(f, old);
  const activation = f.renderer.setSpellSwarm(200000);
  const failure = new Error("Shader preparation failed.");
  f.creations[1].result.reject(failure);
  await assert.rejects(activation, (error) => error === failure);
  assert.equal(f.renderer.spellSwarmHost.current, old);
  assert.equal(old.disposals, 0);
  assert.equal(f.renderer.gpuBytes, 1120);
  assert.equal(f.drains.length, 1);
});

test("an older late creation cannot replace a newer swarm", async () => {
  const f = fixture(),
    old = candidate(120),
    next = candidate(240);
  const first = f.renderer.setSpellSwarm(10000);
  const second = f.renderer.setSpellSwarm(100000);
  f.creations[1].result.resolve(next);
  await settle();
  f.drains[0].resolve();
  await second;
  f.creations[0].result.resolve(old);
  await first;
  assert.equal(f.renderer.spellSwarmHost.current, next);
  assert.equal(old.disposals, 1);
  assert.equal(next.disposals, 0);
  assert.equal(f.renderer.gpuBytes, 1240);
  assert.equal(f.drains.length, 1);
});

test("turning a swarm off cancels a candidate waiting for the queue", async () => {
  const f = fixture(),
    old = candidate(120),
    next = candidate(240);
  await activate(f, old);
  const activation = f.renderer.setSpellSwarm(200000);
  f.creations[1].result.resolve(next);
  await settle();
  await f.renderer.setSpellSwarm(0);
  assert.equal(f.renderer.spellSwarmInfo(), null);
  assert.equal(await f.renderer.probeSpellSwarm(), null);
  assert.equal(f.renderer.gpuBytes, 1000);
  assert.equal(old.disposals, 1);
  f.drains[1].resolve();
  await activation;
  assert.equal(next.disposals, 1);
  assert.equal(f.renderer.gpuBytes, 1000);
  await f.renderer.setSpellSwarm(0);
  assert.equal(old.disposals, 1);
});

test("turning a swarm off cancels creation before any queue drain begins", async () => {
  const f = fixture(),
    next = candidate(240);
  const activation = f.renderer.setSpellSwarm(100000);
  await f.renderer.setSpellSwarm(0);
  f.creations[0].result.resolve(next);
  await activation;
  assert.equal(next.disposals, 1);
  assert.equal(f.drains.length, 0);
  assert.equal(f.renderer.spellSwarmInfo(), null);
  assert.equal(f.renderer.gpuBytes, 1000);
});

test("renderer shutdown retires active and later-created candidates once", async () => {
  const f = fixture(),
    old = candidate(120),
    next = candidate(240);
  await activate(f, old);
  const activation = f.renderer.setSpellSwarm(200000);
  f.renderer.dispose();
  assert.equal(old.disposals, 1);
  assert.equal(f.deviceDisposals(), 1);
  f.creations[1].result.resolve(next);
  await activation;
  assert.equal(next.disposals, 1);
  assert.equal(f.drains.length, 1);
  assert.equal(f.renderer.spellSwarmInfo(), null);
  f.renderer.dispose();
  assert.equal(old.disposals, 1);
  assert.equal(next.disposals, 1);
  await assert.rejects(f.renderer.setSpellSwarm(100000), /Reopen/);
});

test("device loss during a queue drain prevents candidate activation", async () => {
  const f = fixture(),
    next = candidate(240);
  const activation = f.renderer.setSpellSwarm(100000);
  f.creations[0].result.resolve(next);
  await settle();
  f.renderer.lost = true;
  f.drains[0].resolve();
  await activation;
  assert.equal(next.disposals, 1);
  assert.equal(f.renderer.spellSwarmInfo(), null);
  assert.equal(f.renderer.gpuBytes, 1000);
  await assert.rejects(f.renderer.setSpellSwarm(100000), /Reopen/);
});

test("swarm activation snapshots the current camera and anchors above the town", async () => {
  const f = fixture(),
    next = candidate();
  f.renderer.lastView = {
    camera: { eye: [2, 3, 4], yaw: Math.PI / 2, pitch: 0 },
  };
  const activation = f.renderer.setSpellSwarm(100000);
  assert.deepEqual(f.creations[0].center, [
    20,
    12,
    4 - Math.cos(Math.PI / 2) * 18,
  ]);
  f.renderer.lastView.camera.eye[0] = 50;
  assert.equal(f.creations[0].center[0], 20);
  f.creations[0].result.resolve(next);
  await settle();
  f.drains[0].resolve();
  await activation;
  assert.deepEqual(f.renderer.spellSwarmInfo(), { bytes: next.bytes });
  assert.deepEqual(await f.renderer.probeSpellSwarm(), { finite: true });
});

test("unsupported counts and missing features cannot allocate or drain GPU work", async () => {
  const f = fixture();
  for (const count of [-1, 1, 9999, 100001, NaN, Infinity])
    await assert.rejects(f.renderer.setSpellSwarm(count), /Choose/);
  assert.equal(f.creations.length, 0);
  assert.equal(f.drains.length, 0);
  f.renderer.features = {};
  f.renderer.spellSwarmHost = undefined;
  await assert.rejects(f.renderer.setSpellSwarm(100000), /Town editor/);
  assert.equal(f.renderer.gpuBytes, 1000);
});

test("host prepares initial and repeated captures without stepping the flock", async () => {
  const f = fixture(),
    swarm = candidate();
  await activate(f, swarm);
  const host = f.renderer.spellSwarmHost as SpellSwarmHost;
  const encoder = {} as GPUCommandEncoder;
  host.prepareRender(encoder, 100, true);
  host.prepareRender(encoder, 101, true);
  assert.equal(swarm.preparations, 2);
  assert.equal(swarm.steps.length, 0);
  host.prepareRender(encoder, 0, false);
  host.prepareRender(encoder, 0.01, false);
  assert.deepEqual(swarm.steps, [
    { dt: 1 / 60, time: 0 },
    { dt: 0.01, time: 0.01 },
  ]);
  host.prepareRender(encoder, 1000, true);
  host.prepareRender(encoder, 0.02, false);
  assert.deepEqual(swarm.steps.at(-1), { dt: 0.01, time: 0.02 });
  assert.equal(swarm.preparations, 6);
  f.renderer.dispose();
});

test("host counts complete geometry and delegates submission only while active", async () => {
  const f = fixture(),
    swarm = candidate();
  await activate(f, swarm);
  const host = f.renderer.spellSwarmHost as SpellSwarmHost;
  const geometry = { instances: 4, triangles: 12, drawCalls: 2 };
  assert.equal(
    host.render(
      {} as GPURenderPassEncoder,
      new Float32Array(16),
      [0, 1, 2],
      0,
      geometry,
    ),
    true,
  );
  assert.deepEqual(geometry, {
    instances: 100004,
    triangles: 31000012,
    drawCalls: 3,
  });
  host.afterSubmit();
  assert.equal(swarm.submissions, 1);
  swarm.renderEnabled = false;
  assert.equal(
    host.render(
      {} as GPURenderPassEncoder,
      new Float32Array(16),
      [0, 1, 2],
      0,
      geometry,
    ),
    false,
  );
  assert.deepEqual(geometry, {
    instances: 100004,
    triangles: 31000012,
    drawCalls: 3,
  });
  await f.renderer.setSpellSwarm(0);
  host.prepareRender({} as GPUCommandEncoder, 10, false);
  host.afterSubmit();
  assert.equal(swarm.preparations, 0);
  assert.equal(swarm.submissions, 1);
  assert.equal(f.renderer.gpuBytes, 1000);
  f.renderer.dispose();
});

test("the internal orientation experiment forwards through the editor host only", async () => {
  const f = fixture({ orientationPreparation: "compute" }),
    swarm = candidate();
  await activate(f, swarm);
  assert.equal(
    (f.creations[0] as unknown as { orientationPreparation: string })
      .orientationPreparation,
    "compute",
  );
  f.renderer.dispose();
});
