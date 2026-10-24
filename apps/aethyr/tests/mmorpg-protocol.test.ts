import assert from "node:assert/strict";
import test from "node:test";
import {
  MMORPG_PROTOCOL,
  MMO_ICON_KEYS,
  validMmoActor,
  validMmoIconsManifest,
  validMmoServerMessage,
  validMmoSnapshot,
  validMmoUiAction,
  validMmoUiState,
  type MmoActor,
  type MmoSnapshot,
} from "../packages/runtime/mmorpg-protocol.ts";
import {
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
} from "../packages/editor-services/protocol.ts";

const actor = (): MmoActor => ({
  id: 1,
  name: "Paladin",
  class: "paladin",
  kind: "player",
  position: [6, 0, 20],
  yaw: 0,
  health: 200,
  maxHealth: 200,
  stats: {
    health: 200,
    maxHealth: 200,
    strength: 50,
    wisdomness: 5,
    benchpress: 20,
    curl: 100,
    experience: 0,
    level: 1,
  },
  state: "idle",
  actionTime: 0,
  animationTime: 0,
  alive: true,
  weapon: "weapon.sword1",
  attacks: 0,
  hits: 0,
  prediction: {
    position: [6, 0, 20],
    yaw: 0,
    verticalSpeed: 0,
    grounded: true,
    velocity: 0,
    state: "idle",
    stateTicks: 0,
    cooldownTicks: 0,
  },
  inventory: Array.from({ length: 32 }, (_, i) =>
    i === 0
      ? "weapon.axe1"
      : i === 1
        ? "weapon.hammer1"
        : i === 24
          ? "weapon.sword1"
          : null,
  ),
  inventoryVersion: 0,
});
const snapshot = (): MmoSnapshot => ({
  type: "snapshot",
  tick: 1,
  player: actor(),
  actors: [
    {
      ...actor(),
      id: 2,
      name: "Zombie",
      class: "zombie",
      kind: "npc",
      prediction: undefined,
      inventory: undefined,
      inventoryVersion: undefined,
    },
  ],
  despawned: [],
  events: [{ id: 1, tick: 1, kind: "join", actor: 1, name: "Paladin" }],
  ackInput: 0,
  lastEventId: 1,
  eventsTruncated: false,
  population: { players: 1, npcs: 8, spawners: 1 },
});
const icons = () => ({
  schema: "aethyrforge.mmorpg-ui.v1",
  icons: Object.fromEntries(
    MMO_ICON_KEYS.map((key) => [
      key,
      {
        url: `/private-assets/mmorpg-icon-${key}-${"a".repeat(64)}.png`,
        sha256: "a".repeat(64),
        byteLength: 100,
        width: 64,
        height: 64,
      },
    ]),
  ),
});

test("long-running worlds admit the actual capped Rust f32 clocks without widening prediction limits", () => {
  // Captured from the local server after an hour: 216000_f32 * (1_f32 / 60_f32).
  const capped = 3600.000244140625;
  const s = snapshot();
  s.tick = 2 ** 32 + 60;
  s.actors[0].actionTime = s.actors[0].animationTime = capped;
  assert.equal(validMmoActor(s.actors[0]), true);
  assert.equal(validMmoSnapshot(s), true);
  for (const time of [capped + 1 / 60, -1, Infinity, NaN]) {
    assert.equal(validMmoActor({ ...s.actors[0], actionTime: time }), false);
    assert.equal(validMmoActor({ ...s.actors[0], animationTime: time }), false);
  }
  assert.equal(
    validMmoActor({
      ...actor(),
      prediction: { ...actor().prediction!, stateTicks: 216_001 },
    }),
    false,
  );
});

test("source player, inventory, prediction and all server message variants are admitted", () => {
  const s = snapshot();
  assert.equal(validMmoActor(s.player), true);
  assert.equal(validMmoSnapshot(s), true);
  assert.equal(
    validMmoServerMessage({
      type: "welcome",
      protocol: MMORPG_PROTOCOL,
      playerId: 1,
      world: {
        seed: 1,
        worldScale: 1 / 6,
        sourceCommit: "547884332ca650abe96264f7230702d36481b9bc",
        tickRate: 60,
        snapshotRate: 10,
        release: "quick-mmorpg-v1",
        spawn: [6, 0, 20],
        yaw: 0,
      },
      snapshot: s,
    }),
    true,
  );
  assert.equal(
    validMmoServerMessage({
      type: "receipt",
      requestId: "swap-1",
      ok: true,
      duplicate: false,
      inventoryVersion: 1,
    }),
    true,
  );
  assert.equal(
    validMmoServerMessage({
      type: "error",
      message: "The world is full. Try again soon.",
    }),
    true,
  );
});
test("snapshots refuse duplicate or oversized actors, invented items and invalid state", () => {
  const bad: unknown[] = [
    { ...snapshot(), actors: [actor()] },
    { ...snapshot(), actors: [snapshot().actors[0], snapshot().actors[0]] },
    {
      ...snapshot(),
      actors: Array.from({ length: 65 }, (_, i) => ({
        ...snapshot().actors[0],
        id: i + 2,
      })),
    },
    { ...snapshot(), player: { ...actor(), prediction: undefined } },
    { ...snapshot(), player: { ...actor(), inventory: Array(31).fill(null) } },
    {
      ...snapshot(),
      player: { ...actor(), inventory: Array(32).fill("weapon.admin") },
    },
    { ...snapshot(), player: { ...actor(), position: [NaN, 0, 0] } },
    {
      ...snapshot(),
      player: { ...actor(), stats: { ...actor().stats, health: 199 } },
    },
    { ...snapshot(), player: { ...actor(), health: 0, alive: true } },
    {
      ...snapshot(),
      player: {
        ...actor(),
        prediction: { ...actor().prediction, stateTicks: 216001 },
      },
    },
    { ...snapshot(), tick: Number.MAX_SAFE_INTEGER + 1 },
    {
      ...snapshot(),
      events: [{ id: 2, tick: 1, kind: "attack" }],
      lastEventId: 1,
    },
    {
      ...snapshot(),
      events: [
        { id: 1, tick: 1, kind: "attack" },
        { id: 1, tick: 1, kind: "attack" },
      ],
    },
    { ...snapshot(), population: { players: 33, npcs: 8, spawners: 1 } },
  ];
  for (const value of bad) assert.equal(validMmoSnapshot(value), false);
});
test("UI actions grant only guest join, exact slot swap, bounded chat and respawn", () => {
  for (const action of [
    { type: "join", name: "Kiki", class: "paladin" },
    { type: "join", name: "魔法使い", class: "sorceror" },
    { type: "chat", text: "Hello!" },
    { type: "swap", version: 3, from: 0, to: 24 },
    { type: "respawn" },
  ])
    assert.equal(validMmoUiAction(action), true);
  for (const action of [
    null,
    { type: "join", name: " " },
    { type: "join", name: "x", class: "zombie" },
    { type: "join", name: "x".repeat(65) },
    { type: "chat", text: "\n/kill" },
    { type: "chat", text: " " },
    { type: "swap", version: -1, from: 0, to: 24 },
    { type: "swap", version: 0, from: 0, to: 32 },
    { type: "swap", version: 0, from: 24, to: 24 },
    { type: "swap", version: 0, from: 0.5, to: 24 },
    { type: "respawn", health: 1000 },
    { type: "input", attack: true },
  ])
    assert.equal(validMmoUiAction(action), false);
});
test("UI imagery stays on the private asset route with bounded image descriptors", () => {
  assert.equal(validMmoIconsManifest(icons()), true);
  for (const url of [
    "https://evil.test/image.png",
    "/private-assets/../config.png",
    "javascript:alert(1)",
    "/private-assets/image.svg",
    "/private-assets/x.png?redirect=1",
  ]) {
    const value = icons();
    value.icons.sword.url = url;
    assert.equal(validMmoIconsManifest(value), false);
  }
  const value = icons();
  value.icons.sword.byteLength = 33 * 1024 * 1024;
  assert.equal(validMmoIconsManifest(value), false);
});
test("editor status admits bounded MMO state and refuses malformed server data", () => {
  const s = snapshot();
  const mmorpg = {
    connection: "online",
    player: s.player,
    actors: s.actors,
    events: s.events,
    icons: icons(),
    population: s.population,
  };
  assert.equal(validMmoUiState(mmorpg), true);
  const event = (state: unknown) =>
    isPreviewEvent(
      {
        protocol: PREVIEW_PROTOCOL,
        sessionId: "session",
        type: "status",
        status: { ...INITIAL_STATUS, mmorpg: state },
      },
      "session",
    );
  assert.equal(event(mmorpg), true);
  assert.equal(
    event({ ...mmorpg, player: { ...s.player, health: -1 } }),
    false,
  );
  assert.equal(
    event({ ...mmorpg, actors: Array(65).fill(s.actors[0]) }),
    false,
  );
  assert.equal(
    event({
      ...mmorpg,
      events: Array.from({ length: 257 }, (_, i) => ({
        id: i,
        tick: 1,
        kind: "attack",
      })),
    }),
    false,
  );
  assert.equal(
    event({ connection: "login", actors: [], events: [], icons: icons() }),
    true,
  );
});
