import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";
import { mmorpgHeight } from "../packages/runtime/mmorpg-terrain.ts";
import {
  MmorpgSession,
  type MmoControls,
} from "../packages/runtime/mmorpg-session.ts";
import {
  MMORPG_PROTOCOL,
  validMmoServerMessage,
  validMmoUiState,
  type MmoActor,
  type MmoPrediction,
  type MmoSnapshot,
  type MmoUiState,
} from "../packages/runtime/mmorpg-protocol.ts";

const wasm = readFile(new URL("../public/kernel.wasm", import.meta.url));
const scale = 1 / 6;
const source = "547884332ca650abe96264f7230702d36481b9bc";
const idle: MmoControls = {
  forward: 0,
  turn: 0,
  sprint: false,
  attack: false,
  dance: false,
};
const walking: MmoControls = { ...idle, forward: 1 };
const initial = (): MmoPrediction => ({
  position: [0, Math.fround(mmorpgHeight(0, 0)), 0],
  yaw: 0,
  verticalSpeed: 0,
  grounded: true,
  velocity: 0,
  state: "idle",
  stateTicks: 0,
  cooldownTicks: 0,
});
function actor(prediction = initial(), id = 1): MmoActor {
  return {
    id,
    name: id === 1 ? "Kiki" : "Chris",
    class: "paladin",
    kind: "player",
    position: [...prediction.position],
    yaw: prediction.yaw,
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
    state: prediction.state,
    actionTime: prediction.stateTicks / 60,
    animationTime: prediction.stateTicks / 60,
    alive: true,
    weapon: "weapon.sword1",
    attacks: 0,
    hits: 0,
    ...(id === 1
      ? {
          prediction,
          inventory: Array.from({ length: 32 }, (_, i) =>
            i === 0
              ? ("weapon.axe1" as const)
              : i === 24
                ? ("weapon.sword1" as const)
                : null,
          ),
          inventoryVersion: 0,
        }
      : {}),
  };
}
function snapshot(
  tick = 0,
  prediction = initial(),
  patch: Partial<MmoSnapshot> = {},
): MmoSnapshot {
  return {
    type: "snapshot",
    tick,
    player: actor(prediction),
    actors: [],
    despawned: [],
    events: [],
    ackInput: 0,
    lastEventId: 0,
    eventsTruncated: false,
    population: { players: 1, npcs: 0, spawners: 0 },
    ...patch,
  };
}
function welcome(s = snapshot(), worldPatch: Record<string, unknown> = {}) {
  // This is serde_json's actual shortest f32 spelling for Rust's 1.0_f32 / 6.0.
  return {
    type: "welcome",
    protocol: MMORPG_PROTOCOL,
    playerId: 1,
    world: {
      seed: 1,
      worldScale: 0.16666667,
      sourceCommit: source,
      tickRate: 60,
      snapshotRate: 10,
      release: "quick-mmorpg-v1",
      spawn: [0, 0, 0],
      yaw: 0,
      ...worldPatch,
    },
    snapshot: s,
  };
}
class FakeSocket {
  readyState = 0;
  bufferedAmount = 0;
  sent: Record<string, any>[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((message: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send(text: string) {
    this.sent.push(JSON.parse(text));
  }
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  message(value: unknown) {
    this.onmessage?.({ data: JSON.stringify(value) });
  }
  raw(data: unknown) {
    this.onmessage?.({ data });
  }
  close() {
    this.readyState = 3;
    this.onclose?.();
  }
}
async function newKernel() {
  const kernel = await RuntimeKernel.fromBytes(await wasm);
  const result = kernel.request({
    type: "init",
    entities: [],
    colliders: [],
    rooms: [],
    groundY: 0,
    spawn: { position: [0, 0, 0], yaw: 0, pitch: 0, mode: "walk" },
  });
  assert.equal(
    result.ok,
    true,
    "the real engine must initialize before MMO prediction",
  );
  return kernel;
}
async function harness() {
  const kernel = await newKernel(),
    sockets: FakeSocket[] = [],
    published: MmoUiState[] = [],
    requests: Record<string, any>[] = [];
  let now = 0;
  const session = new MmorpgSession({
    url: "ws://test.invalid/mmorpg",
    worldScale: scale,
    release: "quick-mmorpg-v1",
    icons: undefined,
    kernel: {
      request(message) {
        requests.push(message);
        return kernel.request(message);
      },
    },
    changed: (state) => published.push(state),
    now: () => now,
    socket: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
  });
  return {
    kernel,
    session,
    sockets,
    published,
    requests,
    at: (ms: number) => {
      now = ms;
    },
    async join(value = welcome()) {
      const joined = session.action(
        { type: "join", name: "Kiki", class: "paladin" },
        "join-1",
      );
      const socket = sockets.at(-1)!;
      socket.open();
      socket.message(value);
      await joined;
      return socket;
    },
  };
}

test("a real f32 Rust welcome initializes actual Wasm prediction after guest login", async () => {
  const h = await harness();
  try {
    assert.equal(h.session.state.connection, "login");
    const socket = await h.join();
    assert.deepEqual(socket.sent[0], {
      type: "join",
      name: "Kiki",
      class: "paladin",
    });
    assert.equal(h.session.state.connection, "online");
    assert.deepEqual(h.session.step(0, idle)!.prediction, initial());
    assert.ok(h.requests.some((r) => r.type === "mmorpg_prediction_init"));
    assert.equal(
      h.kernel.request({ type: "snapshot" }).prediction.state,
      "idle",
    );
  } finally {
    h.session.disconnect();
  }
});
test("an hour-old server welcome joins with its actual rounded actor clocks", async () => {
  const h = await harness();
  try {
    const older = actor(initial(), 2);
    older.actionTime = older.animationTime = 3600.000244140625;
    const wire = welcome(snapshot(313_504, initial(), { actors: [older] }));
    assert.equal(validMmoServerMessage(wire), true);
    await h.join(wire);
    assert.equal(h.session.state.connection, "online");
    assert.equal(h.session.state.actors[0].animationTime, 3600.000244140625);
    assert.equal(
      h.session.step(1 / 60, idle)?.actors[0].actionTime,
      3600.000244140625,
    );
    h.at(200);
    const frame = h.session.step(1 / 60, idle)!;
    assert.equal(
      validMmoUiState(h.session.presentation(frame.local, frame.actors)),
      true,
    );
  } finally {
    h.session.disconnect();
  }
});

test("failed joins preserve server refusals and version errors instead of a generic disconnect", async () => {
  for (const [wire, expected] of [
    [
      { type: "error", message: "The world is full. Try again soon." },
      "The world is full. Try again soon.",
    ],
    [
      welcome(snapshot(), { release: "another-world" }),
      "The world and character versions do not match.",
    ],
  ] as const) {
    const h = await harness();
    try {
      const joined = h.session.action({ type: "join", name: "Kiki" }, "join");
      const rejected = assert.rejects(
        joined,
        (error: Error) => error.message === expected,
      );
      h.sockets[0].open();
      h.sockets[0].message(wire);
      await rejected;
      assert.equal(h.session.state.message, expected);
      assert.equal(h.session.state.connection, "error");
      assert.equal(h.sockets[0].readyState, 3);
    } finally {
      h.session.disconnect();
    }
  }
});

test("wrong world identity, malformed wire data and snapshots before welcome fail closed", async () => {
  for (const value of [
    welcome(snapshot(), { seed: 2 }),
    welcome(snapshot(), { worldScale: 0.2 }),
    welcome(snapshot(), { sourceCommit: "a".repeat(40) }),
    welcome(snapshot(), { release: "other-world" }),
    { ...welcome(), protocol: "future" },
    snapshot(),
  ]) {
    const h = await harness();
    const joining = h.session.action({ type: "join", name: "Kiki" }, "join");
    const rejected = assert.rejects(joining);
    h.sockets[0].open();
    h.sockets[0].message(value);
    await rejected;
    assert.equal(h.session.state.connection, "error");
    assert.equal(h.session.step(1 / 60, walking), undefined);
    h.session.disconnect();
  }
  for (const value of [
    "{bad",
    new Uint8Array(4),
    " ".repeat(1024 * 1024 + 1),
  ]) {
    const h = await harness(),
      joining = h.session.action({ type: "join", name: "Kiki" }, "join");
    const rejected = assert.rejects(joining);
    h.sockets[0].open();
    h.sockets[0].raw(value);
    await rejected;
    assert.equal(h.session.state.connection, "error");
    h.session.disconnect();
  }
});
test("prediction uses actual Rust movement and replays only input newer than the server acknowledgement", async () => {
  const h = await harness(),
    reference = await newKernel();
  try {
    const socket = await h.join();
    assert.equal(
      reference.request({
        type: "mmorpg_prediction_init",
        worldScale: scale,
        class: "paladin",
        state: initial(),
      }).ok,
      true,
    );
    const authoritative = [initial()];
    for (let i = 1; i <= 6; i++) {
      h.at((i * 1000) / 60);
      const result = h.session.step(1 / 60, walking)!;
      const expected = reference.request({
        type: "mmorpg_prediction_step",
        input: walking,
        ticks: 1,
      }).prediction as MmoPrediction;
      authoritative.push(expected);
      assert.deepEqual(result.prediction, expected);
    }
    assert.ok(authoritative[6].position[2] < 0);
    const inputPackets = socket.sent.filter((v) => v.type === "input");
    assert.ok(
      inputPackets.length < 6 && inputPackets.length >= 2,
      "network controls are throttled independently of 60Hz prediction",
    );
    socket.message(snapshot(6, authoritative[1], { ackInput: 1 }));
    assert.deepEqual(h.session.step(0, idle)!.prediction, authoritative[6]);
    const replay = h.requests
      .filter((r) => r.type === "mmorpg_prediction_reconcile")
      .at(-1)!;
    assert.equal(
      replay.inputs.reduce(
        (n: number, input: { ticks: number }) => n + input.ticks,
        0,
      ),
      5,
    );
    socket.message(snapshot(7, authoritative[6], { ackInput: 6 }));
    assert.deepEqual(h.session.step(0, idle)!.prediction, authoritative[6]);
    assert.equal(
      h.requests.filter((r) => r.type === "mmorpg_prediction_reconcile").at(-1)!
        .inputs.length,
      0,
    );
    socket.message(snapshot(6, initial()));
    assert.equal(
      h.session.tick,
      7,
      "old snapshots cannot roll the authoritative state back",
    );
  } finally {
    h.session.disconnect();
  }
});
test("actual Wasm actions predict attack and dance, then authoritative death stops held movement", async () => {
  const h = await harness();
  try {
    const socket = await h.join();
    h.at(17);
    assert.equal(
      h.session.step(1 / 60, { ...idle, attack: true })!.local.state,
      "attack",
    );
    const attack = socket.sent.at(-1)!;
    assert.equal(attack.attack, true);
    const dead = {
      ...initial(),
      position: [2, 0, 3] as [number, number, number],
      state: "death" as const,
    };
    const deadPlayer = {
      ...actor(dead),
      health: 0,
      stats: { ...actor().stats, health: 0 },
      alive: false,
    };
    socket.message(
      snapshot(10, dead, { player: deadPlayer, ackInput: attack.sequence }),
    );
    for (let i = 0; i < 6; i++) {
      h.at(100 + i * 17);
      const frame = h.session.step(1 / 60, { ...walking, attack: true })!;
      assert.deepEqual(frame.prediction.position, [2, 0, 3]);
      assert.equal(frame.local.state, "death");
    }
    assert.equal(socket.sent.at(-1)!.forward, 0);
    assert.equal(socket.sent.at(-1)!.attack, false);
    const respawn = h.session.action({ type: "respawn" }, "respawn-1");
    socket.message({
      type: "receipt",
      requestId: "respawn-1",
      ok: true,
      duplicate: false,
    });
    await respawn;
    assert.equal(
      h.session.state.player!.alive,
      false,
      "a command receipt cannot revive the player before a snapshot",
    );
    socket.message(
      snapshot(20, initial(), {
        ackInput: socket.sent.filter((p) => p.type === "input").at(-1)!
          .sequence,
      }),
    );
    h.at(300);
    const frame = h.session.step(1 / 60, { ...idle, dance: true })!;
    assert.equal(frame.local.alive, true);
    assert.equal(frame.local.state, "dance");
  } finally {
    h.session.disconnect();
  }
});
test("remote interpolation uses the buffered timeline and the short arc around pi", async () => {
  const h = await harness();
  const older = actor({ ...initial(), yaw: Math.PI - 0.1 }, 2);
  try {
    const socket = await h.join(
      welcome(snapshot(0, initial(), { actors: [older] })),
    );
    h.at(100);
    const newer = {
      ...older,
      position: [10, 0, 0] as [number, number, number],
      yaw: -Math.PI + 0.1,
      animationTime: 0.1,
      actionTime: 0.1,
    };
    socket.message(snapshot(6, initial(), { actors: [newer] }));
    h.at(150);
    const remote = h.session.step(0, idle)!.actors[0];
    assert.equal(remote.position[0], 5);
    assert.ok(Math.abs(Math.cos(remote.yaw) + 1) < 1e-6);
    assert.ok(Math.abs(remote.animationTime - 0.05) < 1e-6);
    h.at(200);
    socket.message(snapshot(12, initial(), { despawned: [2] }));
    h.at(300);
    assert.deepEqual(h.session.step(0, idle)!.actors, []);
  } finally {
    h.session.disconnect();
  }
});
test("chat and inventory commands await matching receipts and never invent authoritative inventory", async () => {
  const h = await harness();
  try {
    const socket = await h.join();
    const inventory = structuredClone(h.session.state.player!.inventory);
    const swap = h.session.action(
      { type: "swap", version: 0, from: 0, to: 24 },
      "swap-1",
    );
    assert.deepEqual(socket.sent.at(-1), {
      type: "swap",
      version: 0,
      from: 0,
      to: 24,
      requestId: "swap-1",
    });
    assert.deepEqual(h.session.state.player!.inventory, inventory);
    await assert.rejects(
      h.session.action({ type: "swap", version: 0, from: 1, to: 24 }, "swap-1"),
      /last action/,
    );
    socket.message({
      type: "receipt",
      requestId: "not-ours",
      ok: true,
      duplicate: false,
    });
    socket.message({
      type: "receipt",
      requestId: "swap-1",
      ok: false,
      duplicate: false,
      message: "The inventory changed. Try that move again.",
    });
    await assert.rejects(swap, /inventory changed/);
    assert.deepEqual(h.session.state.player!.inventory, inventory);
    const chat = h.session.action({ type: "chat", text: "Hello!" }, "chat-1");
    socket.message({
      type: "receipt",
      requestId: "chat-1",
      ok: true,
      duplicate: false,
    });
    await chat;
    socket.message({
      type: "receipt",
      requestId: "chat-1",
      ok: true,
      duplicate: true,
    });
    assert.equal(h.session.state.events.length, 0);
    const changed = actor();
    [changed.inventory![0], changed.inventory![24]] = [
      changed.inventory![24],
      changed.inventory![0],
    ];
    changed.inventoryVersion = 1;
    socket.message(
      snapshot(6, initial(), {
        player: changed,
        events: [
          {
            id: 1,
            tick: 6,
            kind: "chat",
            actor: 1,
            name: "Kiki",
            text: "Hello!",
          },
        ],
        lastEventId: 1,
      }),
    );
    assert.equal(h.session.state.events.length, 1);
    assert.equal(h.session.state.player!.inventoryVersion, 1);
    assert.equal(h.session.state.player!.inventory![24], "weapon.axe1");
  } finally {
    h.session.disconnect();
  }
});
test("receipt timeouts stay retryable and disconnect rejects every outstanding action", async (t) => {
  const h = await harness();
  t.mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const socket = await h.join();
    const timed = assert.rejects(
      h.session.action({ type: "chat", text: "Hello" }, "timeout"),
      /did not answer/,
    );
    t.mock.timers.tick(8000);
    await timed;
    assert.equal(h.session.state.connection, "online");
    const a = assert.rejects(
      h.session.action({ type: "chat", text: "Hello" }, "a"),
      /connection closed/,
    );
    const b = assert.rejects(
      h.session.action({ type: "respawn" }, "b"),
      /connection closed/,
    );
    socket.close();
    await Promise.all([a, b]);
    assert.equal(h.session.state.connection, "disconnected");
    assert.equal(h.session.step(1 / 60, walking), undefined);
  } finally {
    h.session.disconnect();
    t.mock.timers.reset();
  }
});
test("rejoin discards old socket messages and reinitializes prediction and events", async () => {
  const h = await harness();
  try {
    const first = await h.join();
    first.message(
      snapshot(6, initial(), {
        events: [{ id: 1, tick: 6, kind: "chat", text: "Old visit" }],
        lastEventId: 1,
      }),
    );
    const joining = h.session.action(
      { type: "join", name: "Chris", class: "sorceror" },
      "join-2",
    );
    const second = h.sockets[1];
    second.open();
    first.message(snapshot(100, initial()));
    assert.equal(h.session.state.connection, "connecting");
    const local = { ...actor(), class: "sorceror" as const, name: "Chris" };
    second.message(welcome(snapshot(0, initial(), { player: local })));
    await joining;
    assert.equal(h.session.state.player!.class, "sorceror");
    assert.equal(h.session.tick, 0);
    assert.equal(h.session.state.events.length, 0);
    first.message({ type: "error", message: "Old socket" });
    assert.equal(h.session.state.connection, "online");
    assert.equal(h.session.step(1 / 60, walking)!.local.class, "sorceror");
  } finally {
    h.session.disconnect();
  }
});
test("event history is deduplicated and bounded while unacknowledged prediction cannot grow forever", async () => {
  const h = await harness();
  try {
    const socket = await h.join();
    const events = Array.from({ length: 300 }, (_, i) => ({
      id: i + 1,
      tick: 1,
      kind: "chat" as const,
      text: `Line ${i + 1}`,
    }));
    const value = snapshot(1, initial(), { events, lastEventId: 300 });
    assert.equal(validMmoServerMessage(value), true);
    socket.message(value);
    assert.equal(h.session.state.events.length, 256);
    assert.equal(h.session.state.events[0].id, 45);
    socket.message(
      snapshot(2, initial(), {
        events: [
          events[299],
          { id: 301, tick: 2, kind: "join", name: "Chris" },
        ],
        lastEventId: 301,
      }),
    );
    assert.equal(h.session.state.events.length, 256);
    assert.equal(h.session.state.events.at(-1)!.id, 301);
    for (let i = 1; i <= 121; i++) {
      h.at((i * 1000) / 60);
      h.session.step(1 / 60, walking);
    }
    assert.equal(h.session.state.connection, "disconnected");
    assert.match(h.session.state.message!, /stopped answering/);
    assert.equal(h.session.step(1 / 60, walking), undefined);
  } finally {
    h.session.disconnect();
  }
});
test("a stalled join times out without leaving a live connection or accepting delayed welcome", async (t) => {
  const h = await harness();
  t.mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const joining = assert.rejects(
      h.session.action({ type: "join", name: "Kiki" }, "join"),
    );
    const socket = h.sockets[0];
    socket.open();
    t.mock.timers.tick(10_000);
    await joining;
    assert.equal(socket.readyState, 3);
    assert.equal(h.session.state.connection, "error");
    assert.match(h.session.state.message!, /too long/);
    socket.message(welcome());
    assert.equal(h.session.state.connection, "error");
  } finally {
    h.session.disconnect();
    t.mock.timers.reset();
  }
});

test("snapshots cannot replace the joined player's identity or character class", async () => {
  for (const patch of [{ id: 999 }, { class: "sorceror" as const }]) {
    const h = await harness();
    try {
      const socket = await h.join();
      const changed = snapshot(6, initial(), {
        player: { ...actor(), ...patch },
      });
      assert.equal(
        validMmoServerMessage(changed),
        true,
        "the envelope is structurally valid; session identity still needs checking",
      );
      socket.message(changed);
      assert.equal(h.session.state.connection, "error");
      assert.equal(h.session.step(1 / 60, walking), undefined);
    } finally {
      h.session.disconnect();
    }
  }
});
