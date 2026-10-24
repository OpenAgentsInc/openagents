import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { AgentClient } from "../packages/editor-services/agent-client.ts";
import {
  AGENT_PROTOCOL,
  AGENT_CAPTURE_SCHEMA,
  AGENT_LIMITS,
  agentExpectation,
  truncateAgentText,
  validateAgentAction,
  validateAgentCaptureRequest,
  validateAgentContext,
  validateAgentExpectation,
  validateAgentInspectOptions,
  type AgentAction,
  type AgentCaptureEnvelope,
  type AgentCaptureRequest,
  type AgentContext,
  type AgentDiscovery,
  type AgentInspection,
  type AgentObject,
  type AgentReceipt,
  type AgentTransport,
} from "../packages/runtime/agent-contract.ts";

const hash = (value: string | Uint8Array) =>
  createHash("sha256").update(value).digest("hex");
const zeroInput = {
  forward: 0,
  right: 0,
  up: 0,
  lookX: 0,
  lookY: 0,
  sprint: false,
  jump: false,
};
const add: AgentAction = {
  type: "command",
  command: {
    kind: "prop.add",
    assetId: "barrel",
    id: "agent-barrel",
    position: [1, 0, 2],
  },
};
function fake(sessionId = "session-1") {
  const state = {
    revision: 0,
    generation: 1,
    tick: 0,
    frame: 1,
    sequence: 0,
    shader: "initial shader",
    sourceIdentity: hash("owned source"),
    closeCount: 0,
    inspectCount: 0,
    camera: {
      eye: [0, 1.62, 4] as [number, number, number],
      target: [0, 1.62, 0] as [number, number, number],
      fov: 1,
      mode: "walk" as "walk" | "fly",
    },
  };
  const objects = new Map<string, AgentObject>(),
    results = new Map<string, AgentReceipt>(),
    executed: AgentAction[] = [];
  const context = (): AgentContext => ({
    worldId: "starter-courtyard",
    sourceIdentity: state.sourceIdentity,
    revision: state.revision,
    generation: state.generation,
    tick: state.tick,
    shaderIdentity: hash(state.shader),
  });
  const authoredIdentity = () =>
    hash(
      JSON.stringify(
        [...objects.values()].sort((a, b) => a.id.localeCompare(b.id)),
      ),
    );
  const discover = async (): Promise<AgentDiscovery> => ({
    schema: AGENT_PROTOCOL,
    sessionId,
    worldId: context().worldId,
    sourceIdentity: state.sourceIdentity,
    capabilities: ["inspect", "execute", "capture", "replay"],
    limits: AGENT_LIMITS,
  });
  const inspect: AgentTransport["inspect"] = async (options = {}) => {
    state.inspectCount++;
    const all = [...objects.values()].sort((a, b) => a.id.localeCompare(b.id)),
      cursor = options.cursor ?? 0,
      selected = all.slice(cursor, cursor + (options.limit ?? 64));
    return structuredClone({
      schema: AGENT_PROTOCOL,
      requestId: `inspect-${state.inspectCount}`,
      sessionId,
      context: context(),
      authoredIdentity: authoredIdentity(),
      objects: selected,
      totalObjects: all.length,
      truncated: cursor + selected.length < all.length,
      ...(cursor + selected.length < all.length
        ? { nextCursor: cursor + selected.length }
        : {}),
      player: { position: [0, 0, 4], yaw: 0, pitch: 0, grounded: true },
      view: state.camera,
      input: zeroInput,
      render: {
        frame: state.frame,
        instances: 5 + objects.size,
        triangles: 12 + objects.size * 12,
      },
      capabilities: ["inspect", "execute", "capture", "replay"],
      lastWorkingCode: { identity: hash(state.shader), source: state.shader },
    }) as AgentInspection;
  };
  const execute: AgentTransport["execute"] = (action, expect) => {
    const requestId = `command-${executed.length + 1}`;
    executed.push(structuredClone(action));
    let ok = true,
      message = "Applied";
    const actual = agentExpectation(context());
    if (JSON.stringify(actual) !== JSON.stringify(expect)) {
      ok = false;
      message = "The world changed. Inspect again.";
    } else if (action.type === "command") {
      const command = action.command;
      if (command.kind === "prop.add") {
        if (objects.has(command.id)) {
          ok = false;
          message = "This object ID already exists.";
        } else if (command.position[0] === 42) {
          ok = false;
          message = "Choose an open position.";
        } else {
          objects.set(command.id, {
            id: command.id,
            kind: command.assetId,
            position: [...command.position],
            yaw: 0,
            scale: command.scale ?? 1,
            collidable: true,
            halfExtents: [0.5, 0.5, 0.5],
            collisionOffset: [0, 0.5, 0],
          });
          state.revision++;
          state.generation++;
        }
      } else if (command.kind === "entity.transform") {
        const object = objects.get(command.id);
        if (!object || command.position?.[0] === 42) {
          ok = false;
          message = "Choose an existing object and an open position.";
        } else {
          if (command.position) object.position = [...command.position];
          if (command.scale) object.scale = command.scale;
          state.revision++;
          state.generation++;
        }
      } else if (command.kind === "entity.remove") {
        if (!objects.delete(command.id)) {
          ok = false;
          message = "Object missing";
        } else {
          state.revision++;
          state.generation++;
        }
      }
    } else if (action.type === "shader") {
      if (action.source.includes("INVALID")) {
        ok = false;
        message = "WGSL compilation failed";
      } else {
        state.shader = action.source;
        state.generation++;
      }
    } else if (action.type === "camera")
      state.camera = {
        eye: [...action.eye],
        target: [...action.target],
        fov: action.fov,
        mode: action.mode,
      };
    else if (action.type === "input") state.tick += action.durationMs;
    results.set(requestId, {
      requestId,
      ok,
      message,
      revision: state.revision,
      stage: ok ? "activated" : "rejected",
      ...(ok ? {} : { diagnostics: [message] }),
    });
    return requestId;
  };
  const capture: AgentTransport["capture"] = async (options) => {
    const pixels = new Uint8Array(options.width * options.height * 4);
    pixels.fill(127);
    const delta = options.camera.target.map(
        (n, i) => n - options.camera.eye[i],
      ),
      length = Math.hypot(...delta);
    return {
      schema: AGENT_CAPTURE_SCHEMA,
      requestId: `capture-${state.sequence + 1}`,
      sessionId,
      sequence: ++state.sequence,
      context: context(),
      frameBefore: state.frame++,
      frameAfter: state.frame++,
      camera: {
        ...structuredClone(options.camera),
        target: options.camera.eye.map((n, i) => n + delta[i] / length) as [
          number,
          number,
          number,
        ],
      },
      width: options.width,
      height: options.height,
      time: options.time,
      exposure: options.exposure,
      ...(options.actorReference
        ? { actors: structuredClone(options.actorReference) }
        : {}),
      pixelSha256: hash(pixels),
      pixels: pixels.buffer,
    };
  };
  const transport: AgentTransport = {
    discover,
    inspect,
    execute,
    result: async (id) => {
      const value = results.get(id);
      if (!value) throw new Error("Result unavailable");
      return structuredClone(value);
    },
    capture,
    close: () => {
      state.closeCount++;
    },
  };
  return {
    state,
    objects,
    results,
    executed,
    context,
    authoredIdentity,
    transport,
  };
}
const captureRequest = (context: AgentContext): AgentCaptureRequest => ({
  expect: agentExpectation(context),
  camera: { eye: [0, 2, 4], target: [0, 2, -10], up: [0, 1, 0], fov: 1 },
  width: 8,
  height: 6,
  time: 10,
  exposure: 1,
});

test("agent text limits preserve complete UTF-8 characters and replace lone surrogates", () => {
  const encode = new TextEncoder();
  assert.equal(truncateAgentText("ASCII text", 5), "ASCII");
  assert.equal(truncateAgentText("éab", 1), "");
  assert.equal(truncateAgentText("éab", 3), "éa");
  assert.equal(truncateAgentText("🎃a", 3), "");
  assert.equal(truncateAgentText("🎃a", 4), "🎃");
  assert.equal(truncateAgentText("x\ud800y", 3), "x");
  assert.equal(truncateAgentText("x\ud800y", 4), "x�");
  assert.equal(truncateAgentText("\udc00", 3), "�");
  assert.equal(truncateAgentText("\ufeffa", 3), "\ufeff");
  assert.equal(truncateAgentText("text", 0), "");
  assert.equal(truncateAgentText("é🎃text", 100), "é🎃text");
  for (const limit of [1, 2, 3, 4, 511, 512]) {
    const bounded = truncateAgentText("é🎃x".repeat(100), limit);
    assert.ok(encode.encode(bounded).byteLength <= limit);
    assert.equal(bounded.includes("�"), false);
  }
  for (const limit of [-1, 1.5, Infinity])
    assert.throws(() => truncateAgentText("text", limit), /byte limit/);
});

test("agent v1 validates copied explicit actions, context expectations, and bounded tool inputs", () => {
  assert.deepEqual(validateAgentAction(add), add);
  const copy = validateAgentAction(add);
  if (copy.type === "command" && copy.command.kind === "prop.add")
    copy.command.position[0] = 100;
  assert.equal(
    add.type === "command" &&
      add.command.kind === "prop.add" &&
      add.command.position[0],
    1,
  );
  for (const action of [
    {
      type: "command",
      command: {
        kind: "asset.add",
        assetId: "barrel",
        count: 1,
        placement: "ahead",
      },
    },
    { type: "command", command: { kind: "text", text: "add a barrel" } },
    {
      type: "command",
      command: {
        kind: "prop.add",
        assetId: "../secret",
        id: "a",
        position: [0, 0, 0],
      },
    },
    { type: "command", command: { kind: "entity.transform", id: "a" } },
    {
      type: "command",
      command: {
        kind: "entity.transform",
        id: "a",
        position: [Infinity, 0, 0],
      },
    },
    {
      type: "command",
      command: {
        kind: "prop.add",
        assetId: "barrel",
        id: "a",
        position: [0, 0, 0],
        scale: 21,
      },
    },
    { type: "shader", source: "é".repeat(65_537) },
    { type: "input", input: zeroInput, durationMs: 0 },
    { type: "input", input: zeroInput, durationMs: 1001 },
    { type: "input", input: { ...zeroInput, forward: 2 }, durationMs: 10 },
    { type: "input", input: { ...zeroInput, lookX: Infinity }, durationMs: 10 },
    { type: "input", input: zeroInput, durationMs: 10, bypass: true },
    { type: "camera", eye: [0, 0, 0], target: [0, 0, 0], fov: 1, mode: "fly" },
    {
      type: "camera",
      eye: [0, 0, 0],
      target: [0, 0, -1],
      up: [0, 0, 1],
      fov: 1,
      mode: "fly",
    },
    { type: "game", action: "win" },
  ])
    assert.throws(() => validateAgentAction(action));
  for (const kind of ["prop.add", "entity.transform"] as const) {
    for (const position of [
      [100_001, 0, 0],
      [Infinity, 0, 0],
    ])
      assert.throws(
        () =>
          validateAgentAction({
            type: "command",
            command: {
              kind,
              ...(kind === "prop.add" ? { assetId: "barrel" } : {}),
              id: "a",
              position,
            },
          }),
        /finite position coordinates.*world bounds/,
      );
  }
  assert.throws(
    () =>
      validateAgentAction({
        type: "command",
        command: { kind: "entity.transform", id: "../a", position: [0, 0, 0] },
      }),
    /valid object ID/,
  );
  assert.throws(
    () =>
      validateAgentAction({
        type: "command",
        command: { kind: "entity.transform", id: "a" },
      }),
    /explicit position or size/,
  );
  const context = fake().context();
  assert.deepEqual(validateAgentContext(context), context);
  assert.throws(() =>
    validateAgentExpectation({ ...agentExpectation(context), tick: 1 }),
  );
  assert.throws(() => validateAgentContext({ ...context, revision: NaN }));
  assert.throws(() => validateAgentInspectOptions({ limit: 65 }));
  assert.throws(() => validateAgentInspectOptions({ cursor: -1 }));
  assert.throws(() =>
    validateAgentCaptureRequest({
      ...captureRequest(context),
      time: undefined,
    }),
  );
  assert.throws(() =>
    validateAgentCaptureRequest({ ...captureRequest(context), width: 4096 }),
  );
});

test("inspection accepts neutral MMO attack and dance flags after focus clears controls", async () => {
  const source = fake();
  const original = source.transport.inspect;
  let observed: unknown = { ...zeroInput, attack: false, dance: false };
  source.transport.inspect = async (options) => ({
    ...(await original(options)),
    input: observed as AgentInspection["input"],
  });
  const client = new AgentClient(source.transport);
  try {
    const neutral = await client.inspect({ limit: 16 });
    assert.deepEqual(neutral.input, {
      ...zeroInput,
      attack: false,
      dance: false,
    });
    assert.equal(
      source.executed.length,
      0,
      "Reading controls must not execute an action",
    );
    observed = { ...zeroInput, attack: true, dance: true };
    assert.deepEqual((await client.inspect()).input, observed);
    for (const field of ["jump", "attack", "dance"])
      for (const value of [1, "false", null, [], {}]) {
        observed = { ...zeroInput, [field]: value };
        await assert.rejects(client.inspect(), /bounded movement input/);
      }
    observed = { ...zeroInput, attack: false, dance: false, bypass: true };
    await assert.rejects(client.inspect(), /supported agent tool fields/);
  } finally {
    await client.close();
  }
});

test("discovery and bounded inspection expose the real context and enforce revision freshness", async () => {
  const f = fake(),
    client = new AgentClient(f.transport);
  assert.equal((await client.discover()).schema, AGENT_PROTOCOL);
  const inspected = await client.inspect({ limit: 1, expectedRevision: 0 });
  assert.equal(inspected.context.tick, 0);
  assert.equal(inspected.authoredIdentity, f.authoredIdentity());
  await assert.rejects(
    client.inspect({ expectedRevision: 99 }),
    /expected revision/,
  );
  const broken = new AgentClient({
    ...f.transport,
    inspect: async () => ({
      ...(await f.transport.inspect()),
      objects: new Array(65).fill({}),
    }),
  });
  await assert.rejects(broken.inspect(), /oversized/);
  await client.close();
  await broken.close();
});

test("capture validates and copies optional vignette without widening camera actions", async () => {
  const f = fake(),
    request = captureRequest(f.context()),
    vignette = {
      shape: "cosine-fourth" as const,
      intensity: 0.6,
      blendWeight: 1,
    };
  request.camera.vignette = vignette;
  const checked = validateAgentCaptureRequest(request);
  assert.deepEqual(checked.camera.vignette, vignette);
  checked.camera.vignette!.intensity = 0;
  assert.equal(vignette.intensity, 0.6);
  for (const candidate of [
    { ...vignette, shape: "unknown" },
    { ...vignette, intensity: NaN },
    { ...vignette, intensity: -0.1 },
    { ...vignette, blendWeight: 1.1 },
    { ...vignette, extra: true },
  ])
    assert.throws(() =>
      validateAgentCaptureRequest({
        ...request,
        camera: { ...request.camera, vignette: candidate },
      }),
    );
  assert.throws(() =>
    validateAgentAction({ type: "camera", mode: "fly", ...request.camera }),
  );
  const client = new AgentClient(f.transport);
  await client.discover();
  assert.deepEqual((await client.capture(request)).camera.vignette, vignette);
  request.camera.vignette = null;
  assert.equal((await client.capture(request)).camera.vignette, null);
  delete request.camera.vignette;
  assert.equal((await client.capture(request)).camera.vignette, undefined);
  await client.close();
  for (const candidate of [null, { ...vignette, intensity: 0.2 }]) {
    const bad = fake(),
      broken = new AgentClient({
        ...bad.transport,
        capture: async (value) => ({
          ...(await bad.transport.capture(value)),
          camera: { ...value.camera, vignette: candidate },
        }),
      });
    await broken.discover();
    await assert.rejects(
      broken.capture({
        ...captureRequest(bad.context()),
        camera: { ...request.camera, vignette },
      }),
      /different camera/,
    );
    await broken.close();
  }
});

test("accepted edits and confirmed failures record receipts, identities, repair steps, and measured elapsed time", async () => {
  const f = fake();
  let now = 0;
  const client = new AgentClient(f.transport, { now: () => ++now });
  const added = await client.execute(add);
  assert.equal(added.receipt.ok, true);
  assert.equal(added.before.revision, 0);
  assert.equal(added.after.revision, 1);
  assert.equal(added.elapsedMs, 1);
  const rejected = await client.execute({
    type: "command",
    command: {
      kind: "entity.transform",
      id: "agent-barrel",
      position: [42, 0, 2],
    },
  });
  assert.equal(rejected.receipt.ok, false);
  assert.equal(rejected.beforeAuthoredIdentity, rejected.afterAuthoredIdentity);
  assert.match(rejected.receipt.diagnostics![0], /open position/);
  const repaired = await client.execute({
    type: "command",
    command: {
      kind: "entity.transform",
      id: "agent-barrel",
      position: [5, 0, 2],
    },
  });
  assert.equal(repaired.receipt.ok, true);
  await client.execute({ type: "shader", source: "green shader" });
  const invalid = await client.execute({
    type: "shader",
    source: "INVALID shader",
  });
  assert.equal(invalid.receipt.ok, false);
  assert.equal(invalid.before.shaderIdentity, invalid.after.shaderIdentity);
  const log = client.exportLog();
  assert.equal(log.entries.length, 5);
  assert.equal(log.entries.filter((entry) => !entry.receipt.ok).length, 2);
  log.entries[0].receipt.message = "edited outside client";
  assert.notEqual(
    client.exportLog().entries[0].receipt.message,
    "edited outside client",
  );
  await client.close();
});

test("stale expected context is rejected through the same transport without mutating the document", async () => {
  const f = fake(),
    client = new AgentClient(f.transport),
    expected = agentExpectation(f.context());
  await client.execute(add);
  const rejected = await client.execute(
    { type: "command", command: { kind: "entity.remove", id: "agent-barrel" } },
    expected,
  );
  assert.equal(rejected.receipt.ok, false);
  assert.equal(f.objects.size, 1);
  assert.equal(rejected.beforeAuthoredIdentity, rejected.afterAuthoredIdentity);
  await client.close();
});

test("restart replay preserves the raw failure log while applying only accepted stable actions in a fresh session", async () => {
  const original = fake("original"),
    client = new AgentClient(original.transport);
  await client.execute(add);
  await client.execute({
    type: "command",
    command: {
      kind: "entity.transform",
      id: "agent-barrel",
      position: [42, 0, 2],
    },
  });
  await client.execute({
    type: "command",
    command: {
      kind: "entity.transform",
      id: "agent-barrel",
      position: [5, 0, 2],
    },
  });
  await client.execute({ type: "shader", source: "green shader" });
  await client.execute({ type: "shader", source: "INVALID shader" });
  const portable = JSON.parse(JSON.stringify(client.exportLog()));
  await client.close();
  const restarted = fake("new-session"),
    next = new AgentClient(restarted.transport);
  const replayed = await next.replay(portable);
  assert.equal(replayed.length, 3);
  assert.equal(restarted.executed.length, 3);
  assert.equal(portable.entries.length, 5);
  assert.equal(restarted.authoredIdentity(), original.authoredIdentity());
  assert.equal(
    restarted.context().shaderIdentity,
    original.context().shaderIdentity,
  );
  assert(replayed.every((entry) => entry.receipt.ok));
  await next.close();
});

test("replay refuses a different source, preexisting draft, or initial shader before dispatching actions", async () => {
  const f = fake(),
    original = new AgentClient(f.transport);
  await original.execute(add);
  const log = original.exportLog();
  await original.close();
  for (const mode of ["source", "draft", "shader"] as const) {
    const target = fake(mode);
    if (mode === "source") target.state.sourceIdentity = hash("another source");
    if (mode === "draft")
      target.objects.set("existing", {
        id: "existing",
        kind: "barrel",
        position: [0, 0, 0],
        yaw: 0,
        scale: 1,
        collidable: false,
      });
    if (mode === "shader") target.state.shader = "another initial shader";
    const client = new AgentClient(target.transport);
    await assert.rejects(client.replay(log), /initial world/);
    assert.equal(target.executed.length, 0);
    await client.close();
  }
});

test("capture validates context, normalized camera orientation, monotonic sequence, frame progress, and pixel identity", async () => {
  const f = fake(),
    client = new AgentClient(f.transport);
  await client.discover();
  const first = await client.capture(captureRequest(f.context()));
  assert.equal(first.pixels.byteLength, 8 * 6 * 4);
  assert.equal(first.camera.target[2], 3);
  assert.equal(first.frameAfter > first.frameBefore, true);
  await client.capture(captureRequest(f.context()));
  for (const mode of [
    "context",
    "camera",
    "pixels",
    "sequence",
    "frame",
    "session",
  ] as const) {
    const fresh = fake(`capture-${mode}`);
    const broken = new AgentClient({
      ...fresh.transport,
      capture: async (request) => {
        const value = await fresh.transport.capture(request);
        if (mode === "context") value.context.revision++;
        if (mode === "camera") value.camera.eye[0]++;
        if (mode === "pixels") new Uint8Array(value.pixels)[0] ^= 1;
        if (mode === "sequence") value.sequence = 0;
        if (mode === "frame") value.frameAfter = value.frameBefore;
        if (mode === "session") value.sessionId = "wrong-session";
        return value;
      },
    });
    await broken.discover();
    await assert.rejects(broken.capture(captureRequest(fresh.context())));
    await broken.close();
  }
  await client.close();
});

test("capture pins complete actor reference metadata and rejects foreign or relabeled poses", async () => {
  const f = fake(),
    client = new AgentClient(f.transport);
  const actorReference = {
    schema: "aethyrforge.capture-actors.v1" as const,
    id: "actors-1",
    sourceIdentity: f.context().sourceIdentity,
    scenario: "day" as const,
    modelIdentity: "a".repeat(64),
    sourceFrame: 3,
    actorIds: [0, 1],
    count: 2,
    poseSha256: "b".repeat(64),
  };
  const request = { ...captureRequest(f.context()), actorReference };
  await client.discover();
  assert.deepEqual((await client.capture(request)).actors, actorReference);
  assert.throws(
    () =>
      validateAgentCaptureRequest({
        ...request,
        actorReference: { ...actorReference, sourceIdentity: "f".repeat(64) },
      }),
    /world source/,
  );
  for (const mode of ["missing", "pose", "model", "ids"] as const) {
    const broken = new AgentClient({
      ...f.transport,
      capture: async (options) => {
        const result = await f.transport.capture(options);
        if (mode === "missing") delete result.actors;
        if (mode === "pose") result.actors!.poseSha256 = "c".repeat(64);
        if (mode === "model") result.actors!.modelIdentity = "c".repeat(64);
        if (mode === "ids") result.actors!.actorIds.reverse();
        return result;
      },
    });
    await broken.discover();
    await assert.rejects(broken.capture(request), /stale or incomplete/);
    await broken.close();
  }
  await client.close();
});

test("a result timeout leaves the action unresolved until receipt recovery also completes its log entry", async () => {
  const f = fake();
  let fail = true;
  const client = new AgentClient({
    ...f.transport,
    result: async (id) => {
      if (fail) throw new Error("Timed out; outcome unknown");
      return f.transport.result(id);
    },
  });
  await assert.rejects(client.execute(add), /outcome unknown/);
  assert.equal(f.objects.size, 1);
  await assert.rejects(
    client.execute(add),
    /Resolve the previous action result/,
  );
  assert.throws(() => client.exportLog(), /Resolve the pending action/);
  fail = false;
  const outcome = await client.result("command-1");
  assert.equal(outcome.ok, true);
  assert.equal(client.exportLog().entries.length, 1);
  await client.execute({
    type: "command",
    command: { kind: "entity.remove", id: "agent-barrel" },
  });
  assert.equal(f.objects.size, 0);
  await client.close();
});

test("result and session mismatches never count as a verified action", async () => {
  const f = fake();
  const wrong = new AgentClient({
    ...f.transport,
    result: async (id) => ({
      ...(await f.transport.result(id)),
      requestId: "another-request",
    }),
  });
  await assert.rejects(wrong.execute(add), /mismatched command result/);
  assert.throws(() => wrong.exportLog(), /Resolve the pending action/);
  await wrong.close();
  const changed = fake(),
    second = new AgentClient(changed.transport);
  await second.discover();
  changed.transport.inspect = async () => ({
    ...(await f.transport.inspect()),
    sessionId: "other-session",
  });
  await assert.rejects(second.inspect(), /session changed/);
  await second.close();
});

test("log entry and byte limits reject further actions before mutation dispatch", async () => {
  const f = fake(),
    client = new AgentClient(f.transport);
  for (let i = 0; i < 64; i++)
    await client.execute({ type: "input", input: zeroInput, durationMs: 1 });
  const count = f.executed.length;
  await assert.rejects(
    client.execute({ type: "input", input: zeroInput, durationMs: 1 }),
    /log limit/,
  );
  assert.equal(f.executed.length, count);
  assert.equal(client.exportLog().entries.length, 64);
  await client.close();
  const big = fake(),
    bounded = new AgentClient(big.transport);
  let stopped = false;
  for (let i = 0; i < 25; i++) {
    const before = big.executed.length;
    try {
      await bounded.execute({
        type: "shader",
        source: String(i).padEnd(60_000, "x"),
      });
    } catch (error) {
      assert.match(String(error), /log limit/);
      assert.equal(big.executed.length, before);
      stopped = true;
      break;
    }
  }
  assert(stopped);
  assert(
    Buffer.byteLength(JSON.stringify(bounded.exportLog())) <=
      AGENT_LIMITS.logBytes,
  );
  await bounded.close();
});

test("malformed replay and unsupported local actions do not reach the authority transport", async () => {
  const f = fake(),
    client = new AgentClient(f.transport);
  await assert.rejects(
    client.execute({
      type: "command",
      command: { kind: "text", text: "do anything" },
    } as never),
  );
  await assert.rejects(client.replay({ schema: "other", entries: [] }));
  assert.equal(f.executed.length, 0);
  await client.close();
});

test("closing releases pending requests and the adapter exactly once without an untracked mutation", async () => {
  const f = fake(),
    waiting = new Promise<AgentInspection>(() => {});
  const client = new AgentClient({
    ...f.transport,
    inspect: () => {
      f.state.inspectCount++;
      return waiting;
    },
  });
  const pending = Array.from({ length: 16 }, () => client.inspect());
  const settled = Promise.allSettled(pending);
  await assert.rejects(client.inspect(), /current agent requests/);
  assert.equal(f.state.inspectCount, 16);
  await client.close();
  await client.close();
  assert((await settled).every((result) => result.status === "rejected"));
  assert.equal(f.state.closeCount, 1);
  await assert.rejects(client.execute(add), /new agent client/);
  await assert.rejects(client.inspect(), /new agent client/);
  assert.equal(f.executed.length, 0);
});
