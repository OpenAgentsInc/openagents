import assert from "node:assert/strict";
import test from "node:test";
import { EditorService } from "./service.ts";
import { editorAtoms } from "./state.ts";
import {
  validBattleAction,
  validBattleRequest,
  validBattleStatus,
  isPreviewEvent,
  PREVIEW_PROTOCOL,
  INITIAL_STATUS,
  type PreviewMessage,
} from "./protocol.ts";

test("battle actions bound population and seeds without admitting extra controls", () => {
  for (const count of [100, 1_000, 10_000, 100_000, 200_000, 300_000, 500_000, 750_000, 1_000_000])
    for (const seed of [undefined, 0, 1, 0xffffffff])
      assert.equal(
        validBattleAction({
          op: "start",
          count,
          ...(seed === undefined ? {} : { seed }),
        }),
        true,
      );
  for (const op of ["stop", "reset", "leave", "respawn"])
    assert.equal(validBattleAction({ op }), true);
  for (const view of ["overview", "orbit", "player", "first", "third", "dolly"])
    assert.equal(validBattleAction({ op: "camera", view }), true);
  for (const action of [
    null,
    [],
    {},
    { op: "start" },
    ...[99, 1_000_001, 1.5, NaN, Infinity, "10000"].map((count) => ({
      op: "start",
      count,
    })),
    ...[-1, 0x100000000, 1.5, "1", NaN].map((seed) => ({
      op: "start",
      count: 10000,
      seed,
    })),
    { op: "start", count: 10000, unsafe: true },
    { op: "stop", count: 10000 },
    { op: "reset", seed: 1 },
    { op: "camera", view: "fly" },
    { op: "camera", view: "overview", position: [0, 0, 0] },
  ])
    assert.equal(validBattleAction(action), false);
});

test("battle entry and input accept bounded controls without admitting simulation writes", () => {
  const join = { op: "join", at: [-100, 200], team: 1, view: "first", class: "sorceror" };
  const input = { op: "input", forward: 1, turn: -.5, sprint: true, attack: true, attackKind: "ranged", lookX: .1, lookY: -.1 };
  for (const action of [{ op: "join" }, join, input,
    ...["melee", "ranged", "meteor_shower"].flatMap((attackKind) => [
      { op: "weapon", attackKind }, { ...input, attackKind },
    ])])
    assert.equal(validBattleAction(action), true);
  for (const patch of [
    { at: [0, 1, 2] }, { at: [Infinity, 0] }, { at: [20001, 0] }, { team: 2 },
    { view: "dolly" }, { class: "zombie" }, { health: 200 }, { seed: 1 },
  ]) assert.equal(validBattleAction({ ...join, ...patch }), false);
  for (const patch of [
    { forward: 1.01 }, { turn: NaN }, { lookX: .51 }, { lookY: "0" },
    { attack: 1 }, { sprint: null }, { attackKind: "meteor" }, { health: 200 },
    { position: [0, 0, 0] }, { until: 5000 },
  ]) assert.equal(validBattleAction({ ...input, ...patch }), false);
  assert.equal(validBattleAction({ op: "weapon", attackKind: "ranged", damage: 999 }), false);
  assert.equal(validBattleAction({ op: "respawn", health: 200 }), false);
  assert.equal(validBattleAction({ ...input, attackKind: "meteor_shower", target: { point: [0, 0, 0] } }), false);
});

test("Meteor status admits only bounded read-only ground targets", () => {
  const participant = {
    active: true, ready: true, class: "paladin", team: 0, position: [1, 2, 3],
    yaw: 1, pitch: .2, health: 180, maxHealth: 200, alive: true,
    attackKind: "meteor_shower", attackSequence: 4, tick: 90, droppedSeconds: 0,
  };
  const target = { point: [10, 0, 20], normal: [0, 1, 0], distance: 80 };
  const battle = { enabled: true, count: 10000, camera: "first", participant,
    stats: { meteorCooldownTicks: 252, meteorCasts: 1, meteorImpacts: 6, meteorsActive: 0, meteorHits: 8 } };
  assert.equal(validBattleStatus(battle), true);
  assert.equal(validBattleStatus({ ...battle, participant: { ...participant, target } }), true);
  for (const patch of [
    { point: [0, 0] }, { point: [0, NaN, 0] }, { point: [20001, 0, 0] },
    { normal: [0, 0, 0] }, { normal: [0, .5, 0] }, { normal: [0, 1, Infinity] },
    { normal: "up" }, { distance: -1 }, { distance: 80.001 }, { distance: Infinity },
    { distance: "1" }, { damage: 999 },
  ]) assert.equal(validBattleStatus({ ...battle, participant: { ...participant, target: { ...target, ...patch } } }), false);
  assert.equal(validBattleStatus({ ...battle, participant: { ...participant, attackKind: "meteor" } }), false);
  const event = { protocol: PREVIEW_PROTOCOL, sessionId: "active", type: "status", status: { ...INITIAL_STATUS, battle } };
  assert.equal(isPreviewEvent(event, "active"), true);
  assert.equal(isPreviewEvent(event, "other"), false);
});

test("participant summaries report acknowledged health and reject poses or invalid lives", () => {
  const participant = {
    active: true, ready: true, class: "paladin", team: 0, position: [1, 2, 3],
    yaw: 1, pitch: .2, health: 180, maxHealth: 200, alive: true,
    attackKind: "melee", attackSequence: 4, tick: 90, droppedSeconds: 0,
  };
  const battle = { enabled: true, count: 10000, camera: "first", center: [0, 0, 100], radius: 200, participant, stats: { participantEpoch: 3 } };
  assert.equal(validBattleStatus(battle), true);
  assert.equal(validBattleStatus({ ...battle, participant: { ...participant, health: 0, alive: false } }), true);
  for (const patch of [
    { health: 201 }, { health: 0 }, { alive: false }, { ready: 1 }, { team: 2 },
    { position: [0, 0, Infinity] }, { position: [0, 0, "1"] }, { pitch: NaN },
    { attackSequence: 1.5 }, { tick: -1 }, { droppedSeconds: -1 }, { pose: [] },
  ]) assert.equal(validBattleStatus({ ...battle, participant: { ...participant, ...patch } }), false);
  assert.equal(validBattleStatus({ ...battle, center: [0, 0] }), false);
  assert.equal(validBattleStatus({ ...battle, radius: Infinity }), false);
});

test("battle messages retain the authenticated bridge envelope", () => {
  const request = {
    type: "battle",
    action: { op: "start", count: 10000 },
    protocol: PREVIEW_PROTOCOL,
    sessionId: "active",
    requestId: "request",
  };
  assert.equal(validBattleRequest(request), true);
  assert.equal(
    validBattleRequest({ type: "battle", action: { op: "reset" } }),
    true,
  );
  for (const patch of [
    { sessionId: "" },
    { requestId: 1 },
    { protocol: "other" },
    { extra: true },
    { action: { op: "start", count: 1_000_001 } },
  ])
    assert.equal(validBattleRequest({ ...request, ...patch }), false);
});

test("battle status is a bounded summary rather than a crowd pose payload", () => {
  const battle = {
    enabled: true,
    count: 10000,
    camera: "overview",
    stats: {
      alive: 10000,
      dead: 0,
      visible: 8192,
      triangles: 4000000,
      simulationTicks: 12,
      hits: 1,
      gpuMs: 12.5,
    },
  };
  assert.equal(validBattleStatus(battle), true);
  for (const count of [300_000, 500_000, 750_000, 1_000_000])
    assert.equal(validBattleStatus({ ...battle, count, radius: 1000 }), true);
  assert.equal(
    validBattleStatus({
      enabled: false,
      count: 0,
      camera: "player",
      stats: {},
    }),
    true,
  );
  for (const patch of [
    { enabled: 1 },
    { count: 99 },
    { count: 1_000_001 },
    { radius: 1000.01 },
    { camera: "free" },
    { stats: [] },
    { stats: { alive: -1 } },
    { stats: { gpuMs: Infinity } },
    { stats: { gpuMs: NaN } },
    { stats: { alive: Number.MAX_SAFE_INTEGER + 1 } },
    {
      stats: Object.fromEntries(
        Array.from({ length: 65 }, (_, i) => [`value${i}`, i]),
      ),
    },
    { message: "x".repeat(257) },
    { poses: [[0, 0, 0]] },
  ])
    assert.equal(validBattleStatus({ ...battle, ...patch }), false);
  const event = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "active",
    type: "status",
    status: { ...INITIAL_STATUS, battle },
  };
  assert.equal(isPreviewEvent(event, "active"), true);
  assert.equal(isPreviewEvent(event, "other"), false);
  assert.equal(
    isPreviewEvent(
      {
        ...event,
        status: { ...event.status, battle: { ...battle, count: Infinity } },
      },
      "active",
    ),
    false,
  );
});

test("battle service requests preserve the session and reject player or offline access", () => {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const previousLocation = Object.getOwnPropertyDescriptor(
    globalThis,
    "location",
  );
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { location: { origin: "http://editor.test" } },
  });
  const localLocation = { pathname: "/" };
  Object.defineProperty(globalThis, "location", {
    configurable: true,
    value: localLocation,
  });
  const sent: PreviewMessage[] = [];
  const frame = {
    src: "",
    contentWindow: { postMessage: (m: PreviewMessage) => sent.push(m) },
  } as unknown as HTMLIFrameElement;
  const service = new EditorService(frame, "http://preview.test/preview.html");
  try {
    const initial = {
      ...INITIAL_STATUS,
      camera: [6, 3, 20] as [number, number, number],
      mmorpg: { connection: "online" as const, actors: [], events: [] },
    };
    service.registry.set(editorAtoms.status, initial);
    const action = { op: "start" as const, count: 10000, seed: 1 };
    const requestId = service.battle(action);
    assert.deepEqual(sent, [
      {
        type: "battle",
        action,
        requestId,
        sessionId: service.sessionId,
        protocol: PREVIEW_PROTOCOL,
      },
    ]);
    assert.equal(service.registry.get(editorAtoms.pending), 1);
    assert.equal(service.registry.get(editorAtoms.status), initial);
    service.settle(requestId);
    const meteorId = service.battle({ op: "weapon", attackKind: "meteor_shower" });
    assert.deepEqual(sent[1], {
      type: "battle", action: { op: "weapon", attackKind: "meteor_shower" },
      requestId: meteorId, sessionId: service.sessionId, protocol: PREVIEW_PROTOCOL,
    });
    service.settle(meteorId);
    localLocation.pathname = "/player.html";
    assert.throws(
      () => service.battle({ op: "start", count: 10000 }),
      /Open the creator/,
    );
    assert.throws(
      () => service.battle({ op: "weapon", attackKind: "meteor_shower" }),
      /Open the creator/,
    );
    localLocation.pathname = "/";
    assert.throws(
      () => service.battle({ op: "start", count: 99 }),
      /100 to 1,000,000/,
    );
    assert.throws(
      () => service.battle({ op: "start", count: 1_000_001 }),
      /100 to 1,000,000/,
    );
    service.registry.set(editorAtoms.status, { ...initial, mmorpg: undefined });
    assert.throws(() => service.battle({ op: "reset" }), /Enter the world/);
    assert.throws(() => service.battle({ op: "input", forward: 0, turn: 0, sprint: false, attack: true, attackKind: "meteor_shower" }), /Enter the world/);
    assert.equal(sent.length, 2);
  } finally {
    service.close();
    if (previousWindow)
      Object.defineProperty(globalThis, "window", previousWindow);
    else Reflect.deleteProperty(globalThis, "window");
    if (previousLocation)
      Object.defineProperty(globalThis, "location", previousLocation);
    else Reflect.deleteProperty(globalThis, "location");
  }
});
