import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { WorldSession } from "../packages/runtime/world-session.ts";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";
import { FrameCompletions } from "../packages/renderer/frame-completions.ts";
import {
  EnvironmentProfiles,
  DEFAULT_ENVIRONMENT,
} from "../packages/runtime/environment.ts";
import { createStarterGameConfig } from "../packages/runtime/starter-game.ts";
import { pointerLook } from "../packages/runtime/character-driver.ts";
import type { PublishedWorldStage } from "../packages/runtime/published-world.ts";
import type { DestructibleObject } from "../packages/runtime/destructible-scene.ts";

const kernelBytes = await readFile(
  new URL("../public/kernel.wasm", import.meta.url),
);
const spawn = { position: [0, 0, 4], yaw: 0, pitch: 0, mode: "walk" };
const objects: DestructibleObject[] = [
  {
    id: 0,
    sourceId: 0,
    min: [-1, 0, -5],
    max: [1, 3, -3],
    hp: 10,
    anchored: true,
    supports: [],
  },
];
function stubRenderer() {
  const completions = new FrameCompletions();
  const pending: Array<() => void> = [];
  return {
    get frame() {
      return this.frames.length;
    },
    cullCpuMs: 0.1,
    clusterCpuMs: 0.2,
    commandEncodeMs: 0.3,
    profiling: false,
    drainCalls: 0,
    startFrameRecording(capacity: number) {
      completions.start(capacity);
    },
    stopFrameRecording() {
      completions.stop();
    },
    completedFrameSnapshot() {
      return completions.snapshot();
    },
    setBenchmarkEnabled(value: boolean) {
      this.profiling = value;
    },
    gpuSample() {
      return 1;
    },
    gpuFrameSample() {
      return 3;
    },
    async drain() {
      this.drainCalls++;
      pending.splice(0).forEach((finish) => finish());
    },
    frames: [] as { camera: unknown; environment: unknown; time: number }[],
    characters: [] as unknown[],
    enemies: [] as unknown[],
    objects: [] as { scenario: string; objects: unknown[] }[],
    gameplay: undefined as any,
    gameplayUpdates: 0,
    gameplayResets: [] as string[],
    preparations: [] as { scenario: string; nodes: number[] }[],
    masks: [] as { scenario: string; nodes: number[] }[],
    disposed: false,
    async updateAuthored() {},
    setCharacter(value: unknown) {
      this.characters.push(value);
    },
    setEnemies(actors: readonly unknown[]) {
      this.enemies = structuredClone([...actors]);
    },
    setGameplayObjects(scenario: string, values: unknown[]) {
      this.objects.push({ scenario, objects: values });
    },
    resetGameplay(scenario: string) {
      this.gameplayResets.push(scenario);
    },
    setGameplayState(_scenario: string, value: unknown) {
      this.gameplayUpdates++;
      this.gameplay = value;
    },
    resize() {},
    render(camera: unknown, environment: unknown, time: number) {
      this.frames.push({ camera: structuredClone(camera), environment, time });
      const at = performance.now();
      const ticket = completions.submitted(this.frame, at);
      if (ticket) pending.push(() => ticket(at + 2));
      return true;
    },
    benchmarkInfo() {
      return { gpuAccountedBytes: 0 };
    },
    sourceCommandCounts() {
      return { draws: 1, pipelines: 1, materials: 1, windBindings: 1 };
    },
    runtimeInspection() {
      return {
        frame: this.frames.length,
        submittedInstances: 1,
        submittedTriangles: 12,
      };
    },
    prepareGameplayDestruction() {
      return true;
    },
    async prepareGameplayDestructionAsync(
      scenario: string,
      nodes: number[],
      canceled: () => boolean,
    ) {
      this.preparations.push({ scenario, nodes: [...nodes] });
      await Promise.resolve();
      return !canceled();
    },
    gameplayFragmentMasks(scenario: string, nodes: number[]) {
      this.masks.push({ scenario, nodes: [...nodes] });
      return nodes.map((nodeId) => ({
        nodeId,
        mask: [0xffff_ffff, 0xffff_ffff] as [number, number],
      }));
    },
    castleFragmentMasks(scenario: string) {
      return this.objects
        .find((entry) => entry.scenario === scenario)!
        .objects.map(() => [0xffff_ffff, 0xffff_ffff] as [number, number]);
    },
    dispose() {
      this.disposed = true;
    },
  };
}
async function fixture(
  options: {
    room?: boolean;
    gameplay?: boolean;
    night?: boolean;
    game?: boolean;
    enemies?: boolean;
  } = {},
) {
  const renderer = stubRenderer();
  const dayGameplay = options.gameplay
    ? { objects, manifest: { targetGroups: { castle: [0] } } }
    : undefined;
  const nightGameplay = options.gameplay
    ? {
        objects: [
          ...objects,
          {
            ...objects[0],
            id: 1,
            sourceId: 1,
            min: [3, 0, -5],
            max: [5, 3, -3],
          },
        ],
        manifest: { targetGroups: { castle: [0, 1] } },
      }
    : undefined;
  const source = {
    artifact: {
      worldId: "owned-starter",
      sourceIdentity: "a".repeat(64),
      profile: options.room ? "room" : "character",
    },
    world: {
      groundY: 0,
      mode: "walk",
      spawn: structuredClone(spawn),
      entities: [],
      rooms: [],
      colliders: [],
      ...(options.game ? { game: createStarterGameConfig() } : {}),
    },
    kernelBytes: kernelBytes.slice().buffer,
    prepared: {
      renderer,
      scene: { spawn: { position: [0, 1.62, 4], target: [0, 1.62, -1] } },
      environment: {
        ...DEFAULT_ENVIRONMENT,
        mode: options.night ? "night" : "day",
      },
      profiles: new EnvironmentProfiles(),
      look: {
        galleryShots: {
          "MT-02": { scenario: "day", cameraId: "day-camera" },
          "MT-10": { scenario: "night", cameraId: "night-camera" },
        },
        scenarios: {
          day: {
            sun: null,
            cameras: [
              {
                id: "day-camera",
                position: [2, 5, 6],
                target: [0, 1, 0],
                up: [0, 1, 0],
                verticalFovDegrees: 50,
              },
            ],
          },
          night: {
            sun: null,
            cameras: [
              {
                id: "night-camera",
                position: [3, 6, 7],
                target: [0, 1, 0],
                up: [0, 1, 0],
                verticalFovDegrees: 40,
              },
            ],
          },
        },
      },
      dayGameplay,
      nightGameplay,
      ...(options.enemies
        ? {
            enemyPatrols: {
              schema: "aethyrforge.enemy-patrols.v1",
              scenarios: {
                day: [
                  {
                    id: 0,
                    route: [
                      [-3, 0, -3],
                      [-3, 0, 3],
                    ],
                    speed: 2,
                  },
                ],
                night: [
                  {
                    id: 1,
                    route: [
                      [3, 0, -3],
                      [3, 0, 3],
                    ],
                    speed: 2,
                  },
                ],
              },
            },
          }
        : {}),
    },
  } as unknown as PublishedWorldStage;
  return { session: await WorldSession.create(source), renderer, source };
}

test("standalone pointer input and fixed-step camera use the preview sensitivity and current Rust controller", async () => {
  const { session, source } = await fixture({ room: true });
  const reference = await RuntimeKernel.fromBytes(kernelBytes);
  assert.equal(
    reference.request<any>({
      type: "init",
      spawn: source.world!.spawn,
      entities: source.world!.entities,
      rooms: source.world!.rooms,
      colliders: source.world!.colliders,
      groundY: source.world!.groundY,
    }).ok,
    true,
  );
  assert.deepEqual(pointerLook(100, -50), { yawDelta: 0.2, pitchDelta: 0.1 });
  session.look(100, -50);
  session.input({ forward: 1 });
  let expected: any;
  for (let i = 0; i < 120; i++) {
    session.tick(1000 + i * (1000 / 60));
    expected = reference.request({
      type: "step",
      elapsed: 1 / 60,
      input: {
        forward: 1,
        right: 0,
        up: 0,
        sprint: false,
        jump: false,
        yawDelta: i === 0 ? 0.2 : 0,
        pitchDelta: i === 0 ? 0.1 : 0,
      },
    });
  }
  const actual = session.inspect();
  assert.deepEqual(actual.state.player, expected.player);
  assert.equal(actual.mode, "walk");
  assert.ok(Math.abs(actual.camera.yaw - 0.2) < 1e-5);
  assert.ok(Math.abs(actual.camera.pitch - 0.1) < 1e-5);
  session.dispose();
});

test("60 Hz pacing accumulates simulation time and separates paced callbacks from rejected render attempts", async () => {
  const { session, renderer } = await fixture({ room: true });
  await session.benchmark({ op: "start", targetHz: 60 });
  const requests: any[] = [];
  const request = session.kernel.request.bind(session.kernel);
  session.kernel.request = function <T = any>(message: object): T {
    if ((message as any).type === "step")
      requests.push(structuredClone(message));
    return request<T>(message);
  };
  const render = renderer.render.bind(renderer);
  let attempts = 0;
  renderer.render = (camera, env, time) =>
    ++attempts === 3 ? false : render(camera, env, time);
  session.input({ forward: 1 });
  session.tick(1000);
  session.look(100, 0);
  assert.equal(session.tick(1000 + 1000 / 120), false);
  assert.equal(requests.length, 1);
  for (let i = 2; i < 120; i++) session.tick(1000 + i * (1000 / 120));
  const result = await session.benchmark({ op: "stop" });
  assert.deepEqual(result.callbackCounts, {
    rawCallbacks: 120,
    intentionalPacedCallbacks: 60,
    suspendedCallbacks: 0,
    renderAttempts: 60,
  });
  assert.equal(result.samples.length, 60);
  assert.equal(result.samples.filter((value) => !value.submitted).length, 1);
  assert.equal(session.spans.skipped, 1);
  assert.equal(session.spans.pacedCallbacks, 60);
  assert.equal(requests.length, 60);
  assert(
    Math.abs(requests.reduce((sum, value) => sum + value.elapsed, 0) - 1) <
      1e-9,
  );
  assert.equal(requests[1].input.yawDelta, 0.2);
  assert.equal(requests[2].input.yawDelta, 0);
  assert.equal(session.inspect().state.droppedSeconds, 0);
  assert(result.samples.every((sample) => !sample.hostClampedSeconds));
  // Stop returns to the production display-paced profile.
  session.tick(2000);
  session.tick(2000 + 1000 / 120);
  assert.equal(requests.length, 62);
  await session.benchmark({ op: "start", targetHz: "display" });
  for (let i = 0; i < 120; i++) session.tick(2100 + i * (1000 / 120));
  const display = await session.benchmark({ op: "stop" });
  assert.equal(display.targetHz, "display");
  assert.deepEqual(display.callbackCounts, {
    rawCallbacks: 120,
    intentionalPacedCallbacks: 0,
    suspendedCallbacks: 0,
    renderAttempts: 120,
  });
  assert.equal(requests.length, 182);
  session.tick(3100);
  session.tick(3100 + 1000 / 120);
  assert.equal(
    requests.length,
    184,
    "Stop preserves production display timing.",
  );
  await session.benchmark({ op: "start", targetHz: 60 });
  session.start(3200);
  session.tick(3200);
  session.clearInput();
  session.tick(3201);
  assert.equal(
    requests.length,
    186,
    "Restart/input release reset the pacing deadline.",
  );
  session.dispose();
});
test("fly admission persists across ticks and reset restores walking, input and live quest separately", async () => {
  const { session, renderer } = await fixture({ game: true });
  session.setCamera([4, 8, 9], [4, 8, 0], "fly");
  session.input({ forward: 1, up: 1, jump: true });
  session.tick(1000);
  let state = session.inspect();
  assert.equal(state.mode, "fly");
  assert.equal(state.state.player.mode, "fly");
  assert.deepEqual(state.camera.eye, state.state.player.eye);
  assert.equal(state.camera.near, undefined);
  assert.equal(renderer.characters.at(-1), undefined);
  assert.throws(() => session.interact(), /Walk/);
  const revision = state.state.revision;
  session.reset();
  session.tick(1017);
  state = session.inspect();
  assert.equal(state.mode, "walk");
  assert.equal(state.state.player.mode, "walk");
  assert.deepEqual(state.state.player.position, spawn.position);
  assert.deepEqual(state.input, {
    forward: 0,
    right: 0,
    up: 0,
    sprint: false,
    jump: false,
  });
  assert.equal(state.state.revision, revision);
  assert.equal(state.game?.keyCollected, false);
  assert.equal(state.game?.resets, 1);
  session.dispose();
});

test("invalid input/look/camera/time cannot change accepted controls, player or render state", async () => {
  const { session, renderer } = await fixture({ room: true });
  session.input({ forward: 0.5 });
  const before = session.inspect();
  for (const input of [
    { forward: NaN },
    { right: 2 },
    { sprint: 1 },
    { jump: undefined },
    { other: 1 },
  ])
    assert.throws(() => session.input(input as never));
  assert.throws(() => session.look(0, Infinity));
  assert.throws(() => session.setCamera([0, 1, 0], [0, 1, 0]));
  assert.throws(() => session.perspective("third"));
  assert.throws(() => session.tick(NaN));
  assert.throws(() => session.render(Infinity));
  assert.deepEqual(session.inspect().input, before.input);
  assert.deepEqual(session.inspect().state.player, before.state.player);
  assert.equal(renderer.frames.length, 0);
  session.camera.up = [1, 0, 0];
  session.render(0);
  assert.deepEqual(session.camera.up, [1, 0, 0]);
  session.dispose();
  assert.equal(session.tick(1000), false);
  assert.equal(session.render(0), false);
  assert.throws(() => session.setCamera([0, 1, 3], [0, 1, 0]), /closed/);
});

test("initial gameplay follows the chosen night scenario and serializes/cancels asynchronous cast preparation", async () => {
  const { session, renderer } = await fixture({ gameplay: true, night: true });
  assert.equal(session.inspect().state.gameplay.objectCount, 2);
  assert.equal(renderer.objects[0].scenario, "night");
  assert.equal(renderer.objects[0].objects.length, 2);
  let release!: () => void, entered!: () => void, canceled!: () => boolean;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const preparingVisuals = new Promise<void>((resolve) => {
    entered = resolve;
  });
  renderer.prepareGameplayDestructionAsync = async (_mode, _nodes, check) => {
    canceled = check;
    entered();
    await gate;
    return !check();
  };
  const pending = session.cast("destroy_castle");
  const rejected = assert.rejects(pending, /canceled/);
  await assert.rejects(session.cast("meteor_shower"), /finish preparing/);
  await preparingVisuals;
  session.cancelCast();
  assert.equal(canceled(), true);
  await assert.rejects(session.cast("meteor_shower"), /finish preparing/);
  release();
  await rejected;
  assert.equal(session.inspect().state.gameplay.castCount, 0);
  await session.cast("lightning_bolt");
  assert.equal(session.inspect().state.gameplay.castCount, 1);
  session.dispose();
});

test("idle Rust steps preserve gameplay while avoiding repeated visual synchronization", async () => {
  const { session, renderer } = await fixture({ gameplay: true, room: true });
  assert.equal(renderer.gameplayUpdates, 1);
  const authored = session.inspect().state;
  for (let frame = 0; frame < 120; frame++)
    session.tick(1000 + frame * (1000 / 60));
  const idle = session.inspect().state;
  assert.equal(renderer.gameplayUpdates, 1);
  assert.ok(idle.tick >= 120);
  assert.equal(idle.gameplay.objectCount, authored.gameplay.objectCount);
  assert.deepEqual(idle.entities, authored.entities);
  assert.deepEqual(idle.gameplay.destroyed, []);
  await session.cast("lightning_bolt");
  const activeStart = renderer.gameplayUpdates;
  assert.ok(
    renderer.gameplay.effects.length || renderer.gameplay.debris.length,
  );
  for (let frame = 120; frame < 130; frame++)
    session.tick(1000 + frame * (1000 / 60));
  assert.equal(renderer.gameplayUpdates, activeStart + 10);
  assert.equal(session.inspect().state.gameplay.castCount, 1);
  assert.deepEqual(renderer.gameplay, session.inspect().state.gameplay);
  session.dispose();
});

test("camera changes and disposal invalidate pending preparation without applying its cast", async () => {
  for (const operation of ["camera", "reset", "dispose"] as const) {
    const { session, renderer } = await fixture({ gameplay: true });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    renderer.prepareGameplayDestructionAsync = async () => {
      await gate;
      return true;
    };
    const pending = session.cast("destroy_castle");
    const rejected = assert.rejects(pending, /canceled/);
    if (operation === "camera") session.setCamera([0, 8, 4], [0, 8, 0], "fly");
    else if (operation === "reset") session.reset();
    else session.dispose();
    release();
    await rejected;
    assert.equal(
      session.kernel.request<any>({ type: "gameplay_snapshot" }).gameplay
        .castCount,
      0,
    );
    session.dispose();
  }
});

test("every standalone spell awaits fragment preparation and supplies real64-bit masks to Rust", async () => {
  for (const ability of [
    "lightning_bolt",
    "thunderbolt",
    "meteor_shower",
    "destroy_castle",
  ] as const) {
    const { session, renderer } = await fixture({ gameplay: true, room: true });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const prepare = renderer.prepareGameplayDestructionAsync.bind(renderer);
    renderer.prepareGameplayDestructionAsync = async (
      mode,
      nodes,
      canceled,
    ) => {
      await gate;
      return prepare(mode, nodes, canceled);
    };
    const pending = session.cast(ability);
    assert.equal(session.inspect().state.gameplay.castCount, 0);
    assert.deepEqual(renderer.masks, []);
    session.input({ forward: 1 });
    session.tick(1000);
    assert.ok(session.inspect().state.player.position[2] < spawn.position[2]);
    release();
    const cast = await pending;
    assert.equal(cast.ok, true);
    assert.equal(cast.cast.ability, ability);
    assert.equal(session.inspect().state.gameplay.castCount, 1);
    assert.equal(renderer.preparations.length, 1);
    assert.deepEqual(renderer.masks, renderer.preparations);
    if (ability === "lightning_bolt") {
      assert.equal(cast.gameplay.debris.length, 64);
      assert.ok(
        cast.gameplay.debris.every((body: any) => body.subdivisions === 4),
      );
    }
    session.dispose();
  }
});

test("standalone players install the scenario's patrols and draw actual kernel movement without authored edits", async () => {
  for (const night of [false, true]) {
    const { session, renderer } = await fixture({
      gameplay: true,
      enemies: true,
      night,
    });
    const before = session.inspect().state;
    const visualUpdates = renderer.gameplayUpdates;
    assert.equal(before.gameplay.enemies.actors[0].id, night ? 1 : 0);
    assert.equal(renderer.enemies.length, 1);
    for (let i = 0; i < 90; i++) session.tick(1000 + i * (1000 / 60));
    const after = session.inspect().state;
    assert(after.gameplay.enemies.actors[0].animationTime > 0.1);
    assert.notDeepEqual(
      after.gameplay.enemies.actors[0].position,
      before.gameplay.enemies.actors[0].position,
    );
    assert.deepEqual(renderer.enemies, after.gameplay.enemies.actors);
    assert.equal(renderer.gameplayUpdates, visualUpdates);
    assert.equal(after.revision, before.revision);
    session.restoreGameplay();
    const restored = session.inspect().state;
    assert.deepEqual(restored.gameplay.enemies, before.gameplay.enemies);
    assert.deepEqual(renderer.enemies, restored.gameplay.enemies.actors);
    assert.equal(renderer.gameplayUpdates, visualUpdates + 1);
    assert.equal(restored.revision, before.revision);
    session.dispose();
  }
});

test("standalone observations freeze the completion range before drain and preserve optional timing scope", async () => {
  const { session, renderer } = await fixture();
  await session.benchmark({ op: "start", capacity: 10, instrumented: true });
  session.tick(1000);
  session.tick(1017);
  const pending = await session.benchmark({ op: "read" });
  assert.equal(pending.samples.length, 2);
  assert.equal(pending.completedFrames.length, 0);
  const stopped = await session.benchmark({ op: "stop", drain: true });
  assert.equal(stopped.recording, false);
  assert.deepEqual(stopped.completionRange, { firstFrame: 1, lastFrame: 2 });
  assert.deepEqual(
    stopped.completedFrames.map((value) => value.frame),
    [1, 2],
  );
  assert.equal(stopped.samples[0].fullGpuMs, 3);
  assert.equal(renderer.drainCalls, 1);
  session.tick(1034);
  const later = await session.benchmark({ op: "read", drain: true });
  assert.equal(later.samples.length, 2);
  assert.deepEqual(later.completionRange, stopped.completionRange);
  await session.benchmark({ op: "start", instrumented: false });
  session.tick(1051);
  const off = await session.benchmark({ op: "stop", drain: true });
  assert.equal(off.samples[0].gpuMs, null);
  assert.equal(off.samples[0].fullGpuMs, null);
  assert.equal(off.samples[0].stepMs, null);
  assert.equal(off.samples[0].bridgeEncodeMs, null);
  session.dispose();
});

test("source-only player reference cameras preserve their lens and require restoration before scenario changes", async () => {
  const { session } = await fixture({ gameplay: true });
  const before = session.inspect();
  session.scenario("night");
  assert.deepEqual(session.inspect().state.player, before.state.player);
  assert.equal(session.inspect().state.gameplay.objectCount, 2);
  const shot = session.referenceShot("MT-10");
  assert.equal(shot.scenario, "night");
  assert.ok(Math.abs(shot.camera.fov! - (40 * Math.PI) / 180) < 1e-9);
  assert.deepEqual(shot.camera.up, [0, 1, 0]);
  assert.equal(session.inspect().mode, "fly");
  const accepted = session.inspect();
  assert.throws(
    () => session.referenceShot("unpublished-camera"),
    /source reference/,
  );
  assert.deepEqual(session.inspect().camera, accepted.camera);
  // A live prepared cast prevents switching source collision/gameplay ownership.
  const casting = session.cast("meteor_shower");
  assert.throws(() => session.scenario("day"), /Restore/);
  session.cancelCast();
  await assert.rejects(casting, /canceled/);
  session.restoreGameplay();
  session.scenario("day");
  assert.equal(session.inspect().state.gameplay.objectCount, 1);
  session.dispose();
});

test("restore reuses gameplay authority and topology, clears effects, and restores collision only when the player is clear", async () => {
  const { session, renderer, source } = await fixture({
    gameplay: true,
    room: true,
  });
  assert.equal(
    session.kernel.request<any>({
      type: "set_static_collision",
      triangles: [
        [
          [-2, 0, -4],
          [2, 0, -4],
          [2, 3, -4],
        ],
        [
          [-2, 0, -4],
          [2, 3, -4],
          [-2, 3, -4],
        ],
      ],
      owners: [0, 0],
    }).ok,
    true,
  );
  await session.cast("lightning_bolt");
  assert.equal(session.inspect().state.gameplay.destroyedCount, 1);
  const ray = { type: "query_camera", from: [0, 1, 0], to: [0, 1, -8] };
  assert.equal(session.kernel.request<any>(ray).fraction, 1);
  session.setCamera([0, 1.62, -4], [0, 1.62, -5], "walk");
  session.input({ forward: 0.5 });
  const before = session.inspect();
  const updatesBefore = renderer.gameplayUpdates;
  const topologyUpdatesBefore = renderer.objects.length;
  const requests: any[] = [];
  const request = session.kernel.request.bind(session.kernel);
  session.kernel.request = function <T = any>(message: object): T {
    requests.push(message);
    return request<T>(message);
  };
  assert.throws(() => session.restoreGameplay(), /Step away/);
  assert.deepEqual(
    requests.map((value) => value.type),
    ["gameplay_reset"],
  );
  const denied = session.inspect();
  assert.deepEqual(denied.state.player, before.state.player);
  assert.deepEqual(denied.state.gameplay, before.state.gameplay);
  assert.deepEqual(denied.input, before.input);
  assert.equal(renderer.gameplayUpdates, updatesBefore);
  assert.equal(renderer.gameplayResets.length, 0);
  assert.equal(session.kernel.request<any>(ray).fraction, 1);
  session.setCamera([0, 1.62, 0], [0, 1.62, -5], "walk");
  const clear = session.inspect();
  requests.length = 0;
  session.restoreGameplay();
  assert.deepEqual(
    requests.map((value) => value.type),
    ["gameplay_reset"],
  );
  const restored = session.inspect();
  assert.deepEqual(restored.state.player, clear.state.player);
  assert.equal(restored.state.revision, clear.state.revision);
  assert.deepEqual(restored.state.entities, clear.state.entities);
  assert.equal(
    restored.state.gameplay.objectCount,
    clear.state.gameplay.objectCount,
  );
  assert.ok(restored.state.gameplay.version > clear.state.gameplay.version);
  for (const field of ["destroyed", "debris", "rubble", "effects", "damage"])
    assert.deepEqual(restored.state.gameplay[field], [], field);
  assert.equal(restored.state.gameplay.castCount, 0);
  assert.equal(restored.state.gameplay.pendingImpacts, 0);
  assert.equal(restored.state.gameplay.activeCast, null);
  assert.equal(restored.input.forward, 0);
  assert.deepEqual(renderer.gameplay, restored.state.gameplay);
  assert.equal(renderer.objects.length, topologyUpdatesBefore);
  assert.deepEqual(renderer.gameplayResets, ["day"]);
  assert.equal(
    renderer.objects.at(-1)!.objects,
    source.prepared!.dayGameplay!.objects,
  );
  assert.ok(session.kernel.request<any>(ray).fraction < 1);
  await session.cast("lightning_bolt");
  assert.equal(session.inspect().state.gameplay.castCount, 1);
  assert.equal(session.inspect().state.gameplay.destroyedCount, 1);
  session.dispose();
});

test("restore cannot commit while a spell is still preparing", async () => {
  const { session, renderer } = await fixture({ gameplay: true, room: true });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  renderer.prepareGameplayDestructionAsync = async () => {
    await gate;
    return true;
  };
  const pending = session.cast("lightning_bolt");
  const before = session.inspect();
  assert.throws(() => session.restoreGameplay(), /finish preparing/);
  assert.deepEqual(session.inspect().state.gameplay, before.state.gameplay);
  release();
  await pending;
  assert.equal(session.inspect().state.gameplay.castCount, 1);
  session.dispose();
});

test("published-player frame recording uses actual entry time independently of the scheduled frame timestamp", async () => {
  const { session } = await fixture();
  await session.benchmark({ op: "start", instrumented: false });
  const before = performance.now();
  session.tick(1000);
  const after = performance.now();
  const result = await session.benchmark({ op: "stop", drain: true });
  assert.equal(result.callbackClock, "entry");
  assert.equal(result.samples.length, 1);
  assert.equal(result.samples[0].at, 1000);
  assert(
    result.samples[0].callbackEnteredAt! >= before &&
      result.samples[0].callbackEnteredAt! <= after,
  );
  assert.equal(result.samples[0].stepMs, null);
  session.dispose();
});
