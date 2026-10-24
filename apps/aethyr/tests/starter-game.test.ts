import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import {
  validateCharacter,
  validateCharacterGeometry,
  type CharacterManifest,
} from "../packages/renderer/character.ts";
import {
  STARTER_CAPABILITIES,
  StarterGame,
  createStarterGameConfig,
  replayStarterGame,
  validateStarterGameConfig,
  type StarterGameEvent,
} from "../packages/runtime/starter-game.ts";
import type { CompiledScene } from "../packages/assets/src/scene.ts";

const config = () => createStarterGameConfig();

test("owned starter exposes versioned capabilities without a VM or new movement engine", () => {
  assert.deepEqual(
    STARTER_CAPABILITIES.map((capability) => [
      capability.id,
      capability.version,
    ]),
    [
      ["character.controller", 1],
      ["camera.first-third-person", 1],
      ["interaction.proximity", 1],
      ["objective.key-exit", 1],
      ["game.restart", 1],
      ["environment.sky", 1],
    ],
  );
  assert.equal(STARTER_CAPABILITIES[0].provider, "rust-kernel");
  const turned = createStarterGameConfig([10, 2, 5], [15, 2, 5]);
  assert.deepEqual(turned.key.position, [13, 2, 3]);
  assert.deepEqual(turned.exit.position, [17, 2, 7]);
});

test("key and exit interactions complete once and reset independently of authored state", () => {
  const authored = config(),
    source = JSON.stringify(authored),
    game = new StarterGame(authored);
  assert.equal(game.status(authored.spawn).nearby, null);
  const locked = game.apply({ type: "interact" }, authored.exit.position);
  assert.equal(locked.ok, false);
  if (!locked.ok) assert.equal(locked.code, "locked");
  assert.equal(game.snapshot().sequence, 0);
  assert.equal(game.status(authored.key.position).prompt, "Collect key");
  const collected = game.apply({ type: "interact" }, authored.key.position);
  assert.equal(collected.ok, true);
  assert.equal(game.status(authored.key.position).nearby, null);
  assert.equal(game.snapshot().keyCollected, true);
  assert.equal(game.status(authored.exit.position).prompt, "Unlock exit");
  assert.equal(
    game.apply({ type: "interact" }, authored.exit.position).ok,
    true,
  );
  assert.equal(game.snapshot().completed, true);
  assert.equal(game.status(authored.exit.position).target, null);
  const done = game.snapshot();
  assert.equal(
    game.apply({ type: "interact" }, authored.exit.position).ok,
    false,
  );
  assert.deepEqual(game.snapshot(), done);
  const restart = game.apply({ type: "reset" }, [4, 0, 1]);
  assert.equal(restart.ok, true);
  if (restart.ok) assert.deepEqual(restart.respawn, authored.spawn);
  assert.equal(game.snapshot().keyCollected, false);
  assert.equal(game.snapshot().completed, false);
  assert.equal(game.snapshot().resets, 1);
  assert.equal(game.snapshot().sequence, 3);
  assert.equal(JSON.stringify(authored), source);
  assert.deepEqual(game.config(), authored);
});

test("interaction uses finite three-dimensional distance and the inclusive authored range", () => {
  const authored = config(),
    game = new StarterGame(authored);
  const [x, y, z] = authored.key.position;
  assert.equal(
    game.apply({ type: "interact" }, [x, y + authored.key.radius + 0.01, z]).ok,
    false,
  );
  assert.equal(
    game.apply({ type: "interact" }, [x + authored.key.radius, y, z]).ok,
    true,
  );
  const before = game.snapshot();
  for (const invalid of [
    [NaN, 0, 0],
    [Infinity, 0, 0],
    [0, 0],
    [20_001, 0, 0],
    null,
  ]) {
    assert.equal(game.apply({ type: "reset" }, invalid as never).ok, false);
    assert.deepEqual(game.snapshot(), before);
  }
});

test("unsupported actions and invalid settings leave configuration and progress unchanged", () => {
  const authored = config(),
    game = new StarterGame(authored);
  game.apply({ type: "interact" }, authored.key.position);
  const before = game.snapshot();
  for (const action of [
    null,
    {},
    { type: "teleport" },
    { type: "reset", award: 20 },
  ])
    assert.equal(game.apply(action as never, authored.spawn).ok, false);
  const invalid: unknown[] = [
    { ...authored, schema: "aethyrforge.starter-game.v2" },
    {
      ...authored,
      requiredCapabilities: [
        ...authored.requiredCapabilities,
        "script.arbitrary",
      ],
    },
    {
      ...authored,
      requiredCapabilities: authored.requiredCapabilities.slice(1),
    },
    { ...authored, title: "" },
    { ...authored, extra: true },
    { ...authored, key: { ...authored.key, radius: NaN } },
    { ...authored, key: { ...authored.key, radius: 100 } },
    { ...authored, key: { ...authored.key, position: [0, Infinity, 0] } },
    { ...authored, exit: { ...authored.exit, id: authored.key.id } },
    { ...authored, exit: { ...authored.exit, id: "replacement-exit" } },
    { ...authored, id: "new-game" },
  ];
  for (const candidate of invalid) {
    assert.throws(() => game.replaceConfig(candidate));
    assert.deepEqual(game.snapshot(), before);
    assert.deepEqual(game.config(), authored);
  }
});

test("compatible authored transform updates preserve live progress and use the new range", () => {
  const authored = config(),
    game = new StarterGame(authored);
  game.apply({ type: "interact" }, authored.key.position);
  const changed = structuredClone(authored);
  changed.exit.position = [5, 0, -7];
  changed.title = "A longer escape";
  game.replaceConfig(changed);
  assert.equal(game.snapshot().keyCollected, true);
  assert.equal(game.snapshot().sequence, 1);
  assert.equal(game.status(authored.exit.position).nearby, null);
  assert.equal(game.status(changed.exit.position).nearby, "exit");
  assert.equal(
    game.apply({ type: "interact" }, changed.exit.position).ok,
    true,
  );
  changed.exit.position[0] = 10;
  assert.equal(game.config().exit.position[0], 5);
  const escaped = game.config();
  escaped.key.id = "mutated-copy";
  assert.equal(game.config().key.id, authored.key.id);
});

test("restart, checkpoint load, and deterministic replay produce the same progress", () => {
  const authored = config(),
    game = new StarterGame(authored),
    events: StarterGameEvent[] = [];
  for (const [action, player] of [
    [{ type: "interact" }, authored.key.position],
    [{ type: "interact" }, authored.exit.position],
    [{ type: "reset" }, authored.exit.position],
    [{ type: "interact" }, authored.key.position],
  ] as const) {
    const result = game.apply(action, player);
    assert.equal(result.ok, true);
    if (result.ok) events.push(result.event);
  }
  assert.deepEqual(replayStarterGame(authored, events), game.snapshot());
  const restored = new StarterGame(authored);
  restored.restore(JSON.parse(JSON.stringify(game.snapshot())));
  assert.deepEqual(restored.snapshot(), game.snapshot());
  const isolated = restored.snapshot();
  isolated.completed = true;
  assert.equal(restored.snapshot().completed, false);
  assert.throws(() => replayStarterGame(authored, [events[1], events[0]]));
  assert.throws(() => replayStarterGame(authored, [events[0], events[0]]));
  assert.throws(() =>
    replayStarterGame(authored, [{ ...events[0], outcome: "completed" }]),
  );
  assert.throws(() =>
    replayStarterGame(authored, [{ ...events[0], position: [999, 0, 999] }]),
  );
  assert.throws(() =>
    replayStarterGame({ ...authored, title: "Different" }, events),
  );
});

test("invalid and incompatible saves are rejected atomically", () => {
  const game = new StarterGame(),
    before = game.snapshot();
  for (const save of [
    { ...before, completed: true },
    { ...before, keyCollected: true },
    { ...before, sequence: -1 },
    { ...before, sequence: 1.5 },
    { ...before, sequence: 2, resets: 3 },
    { ...before, sequence: 1, resets: 1, keyCollected: true },
    { ...before, sequence: 2, resets: 1, keyCollected: true, completed: true },
    { ...before, configIdentity: "unrelated" },
    { ...before, unexpected: true },
  ]) {
    assert.throws(() => game.restore(save));
    assert.deepEqual(game.snapshot(), before);
  }
});

test("public starter fixture uses bounded owned geometry and no private resources", async () => {
  const root = new URL("../public/starter/", import.meta.url);
  const scene = JSON.parse(
    await readFile(new URL("scene.json", root), "utf8"),
  ) as CompiledScene;
  assert.equal(scene.schema, "aethyrforge.private-scene.v1");
  assert.equal(scene.spawn.position[1], 1.62);
  const resources = await Promise.all(
    [scene.vertices, scene.indices, scene.vertexColors].map(async (url) => {
      assert.match(url, /^\/starter\/[a-z-]+\.bin$/);
      return readFile(new URL(`../public${url}`, import.meta.url));
    }),
  );
  assert.equal(resources[0].byteLength, scene.vertexCount * 48);
  assert.equal(resources[1].byteLength, scene.indexCount * 4);
  assert.equal(resources[2].byteLength, scene.vertexCount * 4);
  assert.ok(scene.instances.length >= 5);
  assert.equal(
    scene.materials.filter((material) => material.baseColorUrl).length,
    1,
  );
  assert.equal(scene.materials.at(-1)?.baseColorUrl, "/starter/mural.png");
  assert.ok(scene.materials.every((material) => !material.normalUrl));
  assert.ok(
    scene.instances.every(
      (instance) =>
        instance.model.length === 16 && instance.model.every(Number.isFinite),
    ),
  );
  assert.equal(scene.collisionURL, undefined);
  assert.equal(scene.characterURL, "/starter/character.json");
  const fixture = JSON.parse(
    await readFile(new URL("game.json", root), "utf8"),
  );
  assert.deepEqual(validateStarterGameConfig(fixture), config());
});

test("owned starter avatar passes the production character and skinning validators", async () => {
  const root = new URL("../public/starter/", import.meta.url);
  const manifest = validateCharacter(
    JSON.parse(
      await readFile(new URL("character.json", root), "utf8"),
    ) as CharacterManifest,
  );
  const buffers = await Promise.all(
    [manifest.geometry.vertices, manifest.geometry.indices].map(
      async (blob) => {
        assert.match(blob.url, /^\/starter\/character-[a-z]+\.bin$/);
        const data = await readFile(
          new URL(`../public${blob.url}`, import.meta.url),
        );
        assert.equal(data.byteLength, blob.byteLength);
        assert.equal(
          createHash("sha256").update(data).digest("hex"),
          blob.sha256,
        );
        return data.buffer.slice(
          data.byteOffset,
          data.byteOffset + data.byteLength,
        );
      },
    ),
  );
  validateCharacterGeometry(
    manifest,
    new Float32Array(buffers[0]),
    new Uint32Array(buffers[1]),
  );
  assert.equal(manifest.joints.length, 1);
  assert.equal(manifest.materials[0].texture, undefined);
});

test("starter day and night share geometry, markers have distinct materials, and walls have collision", async () => {
  const root = new URL("../public/starter/", import.meta.url);
  const read = async (name: string) =>
    JSON.parse(await readFile(new URL(name, root), "utf8"));
  const scene = (await read("scene.json")) as CompiledScene;
  const night = (await read("scene-night.json")) as CompiledScene;
  assert.equal(night.scenario, "night");
  assert.deepEqual({ ...night, scenario: "day" }, scene);
  const catalog = await read("catalog.json");
  assert.equal(
    scene.meshes[catalog.assets.find((asset: any) => asset.id === "key").mesh]
      .primitives[0].material,
    2,
  );
  assert.equal(
    scene.meshes[catalog.assets.find((asset: any) => asset.id === "exit").mesh]
      .primitives[0].material,
    3,
  );
  const colliders = (await read("colliders.json")) as Array<{
    id: string;
    min: number[];
    max: number[];
  }>;
  assert.ok(colliders.some((box) => box.id === "west-wall"));
  assert.ok(colliders.some((box) => box.id === "east-wall"));
  assert.ok(colliders.some((box) => box.id === "south-wall"));
  assert.ok(colliders.some((box) => box.id === "north-door-lintel"));
  assert.ok(
    colliders.every((box) =>
      box.min.every((n, i) => Number.isFinite(n) && n < box.max[i]),
    ),
  );
  const inside = (point: number[], box: { min: number[]; max: number[] }) =>
    point.every((n, i) => n > box.min[i] && n < box.max[i]);
  assert.equal(
    colliders.some((box) => inside([0, 0.8, 4], box)),
    false,
  );
  assert.equal(
    colliders.some((box) => inside([1, 0.8, -7], box)),
    false,
  );
  assert.equal(
    colliders.some((box) => inside([-3, 0.8, -7], box)),
    true,
  );
});
