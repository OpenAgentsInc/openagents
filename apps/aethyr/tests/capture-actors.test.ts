import assert from "node:assert/strict";
import test from "node:test";
import {
  CAPTURE_ACTORS_SCHEMA,
  CaptureActorReferences,
  sameCaptureActorReference,
  assertNativeStartCapture,
  validateCaptureActorReferenceId,
  validateCaptureActorReferenceMetadata,
  type ActorCaptureReferenceMetadata,
} from "../packages/runtime/capture-actors.ts";

const sourceIdentity = "1".repeat(64),
  modelIdentity = "2".repeat(64),
  poseSha256 = "3".repeat(64);
const metadata = (): ActorCaptureReferenceMetadata => ({
  schema: CAPTURE_ACTORS_SCHEMA,
  id: "fixture-actor-reference",
  sourceIdentity,
  scenario: "day",
  modelIdentity,
  sourceFrame: 24,
  actorIds: [0, 17],
  count: 2,
  poseSha256,
});
const pin = (scenario: "day" | "night" = "day") => ({
  scenario,
  modelIdentity,
  sourceFrame: 24,
  actorIds: [0, 17],
  poseSha256,
  payload: { privatePalette: new Float32Array([1, 2, 3]) },
});

test("native-start setup is explicit, pristine, bounded and never admitted during measurement", () => {
  const gameplay = {
    castCount: 0,
    destroyedCount: 0,
    pendingImpacts: 0,
    pendingCollapse: 0,
    activeCast: null,
    debris: [],
    effects: [],
    enemies: { total: 2, alive: 2, killed: 0, actors: [{ id: 0 }, { id: 1 }] },
  };
  const options = {
    diagnostic: true,
    recording: false,
    preparing: false,
    gameplay,
  };
  assert.doesNotThrow(() => assertNativeStartCapture(options));
  const before = structuredClone(options);
  for (const change of [
    { diagnostic: false },
    { recording: true },
    { preparing: true },
  ])
    assert.throws(
      () => assertNativeStartCapture({ ...options, ...change }),
      /pristine diagnostic/,
    );
  for (const field of [
    "castCount",
    "destroyedCount",
    "pendingImpacts",
    "pendingCollapse",
  ])
    assert.throws(() =>
      assertNativeStartCapture({
        ...options,
        gameplay: { ...gameplay, [field]: 1 },
      }),
    );
  for (const changed of [
    { activeCast: {} },
    { debris: [{}] },
    { effects: [{}] },
    { enemies: { ...gameplay.enemies, killed: 1 } },
    { castle: { active: true } },
    { castle: { pendingImpacts: 1 } },
  ])
    assert.throws(() =>
      assertNativeStartCapture({
        ...options,
        gameplay: { ...gameplay, ...changed },
      }),
    );
  assert.deepEqual(options, before);
});
const expected = (scenario: "day" | "night" = "day") => ({
  scenario,
  modelIdentity,
  actorIds: [0, 17],
});

test("actor metadata is complete, bounded, copied, and never accepts private pose fields", () => {
  const input = metadata(),
    actual = validateCaptureActorReferenceMetadata(input);
  assert.deepEqual(actual, input);
  actual.actorIds[0] = 99;
  assert.deepEqual(input.actorIds, [0, 17]);
  for (const patch of [
    { schema: "old" },
    { id: "" },
    { id: "actor/elsewhere" },
    { id: "a".repeat(97) },
    { sourceIdentity: "wrong" },
    { modelIdentity: "F".repeat(64) },
    { poseSha256: "wrong" },
    { sourceFrame: -1 },
    { sourceFrame: 0.5 },
    { sourceFrame: Infinity },
    { sourceFrame: Number.MAX_SAFE_INTEGER + 1 },
    { scenario: "dawn" },
    { count: 1 },
    { count: "2" },
    { actorIds: [0, 0] },
    { actorIds: [-1, 17] },
    { actorIds: [0.5, 17] },
    { actorIds: [NaN, 17] },
    { actorIds: [0x100000000, 17] },
    { actorIds: Array.from({ length: 17 }, (_, i) => i), count: 17 },
    { actorIds: new Array(1), count: 1 },
    { actorIds: null },
    { palette: [1, 2, 3] },
    { basis: "live" },
    { basis: null },
    { basis: undefined },
  ])
    assert.throws(() =>
      validateCaptureActorReferenceMetadata({ ...metadata(), ...patch }),
    );
  const missing: Partial<ActorCaptureReferenceMetadata> = metadata();
  delete missing.poseSha256;
  assert.throws(() => validateCaptureActorReferenceMetadata(missing));
  for (const id of [undefined, null, 1, "white space", "💀", "a".repeat(97)])
    assert.throws(() => validateCaptureActorReferenceId(id));
  assert.equal(
    validateCaptureActorReferenceId("actor-UUID_17"),
    "actor-UUID_17",
  );
});

test("full metadata comparison rejects any changed public field or additional data", () => {
  const original = metadata();
  assert.equal(
    sameCaptureActorReference(original, structuredClone(original)),
    true,
  );
  const changed: Record<string, unknown>[] = [
    { schema: "old" },
    { id: "another-reference" },
    { sourceIdentity: "4".repeat(64) },
    { scenario: "night" },
    { modelIdentity: "4".repeat(64) },
    { sourceFrame: 25 },
    { actorIds: [17, 0] },
    { count: 1 },
    { poseSha256: "4".repeat(64) },
    { palette: [] },
    { basis: "native-start" },
  ];
  for (const patch of changed)
    assert.equal(
      sameCaptureActorReference(original, { ...original, ...patch }),
      false,
    );
  assert.equal(sameCaptureActorReference(null, original), false);
  assert.equal(sameCaptureActorReference(original, undefined), false);
});

test("native-start basis is explicit, retained by the registry, and distinct from legacy last-live references", () => {
  const legacy = metadata(),
    canonical = { ...metadata(), basis: "native-start" as const };
  const old = validateCaptureActorReferenceMetadata(legacy);
  assert.equal(Object.hasOwn(old, "basis"), false);
  const admitted = validateCaptureActorReferenceMetadata(canonical);
  assert.equal(admitted.basis, "native-start");
  assert.equal(sameCaptureActorReference(admitted, canonical), true);
  assert.equal(sameCaptureActorReference(legacy, admitted), false);
  assert.equal(sameCaptureActorReference(admitted, legacy), false);
  const registry = new CaptureActorReferences({
    sourceIdentity,
    incarnation: {},
  });
  const live = registry.pin(pin());
  assert.equal(Object.hasOwn(live, "basis"), false);
  const native = registry.pin({ ...pin(), basis: "native-start" });
  assert.equal(native.basis, "native-start");
  assert.equal(
    registry.resolve(native.id, expected()).metadata.basis,
    "native-start",
  );
  assert.throws(() => registry.resolve(live.id, expected()));
  assert.throws(() => registry.pin({ ...pin(), basis: "live" as never }));
  assert.deepEqual(registry.resolve(native.id, expected()).metadata, native);
});

test("host-only lookup requires the same world incarnation, scenario, model and ordered native actor IDs", () => {
  const incarnation = {};
  const registry = new CaptureActorReferences<{ privatePalette: Float32Array }>(
    { sourceIdentity, incarnation },
  );
  const request = pin(),
    ref = registry.pin(request);
  const resolved = registry.resolve(ref.id, expected());
  assert.equal(resolved.payload, request.payload);
  assert.deepEqual(resolved.metadata, ref);
  resolved.metadata.actorIds[0] = 99;
  ref.actorIds[0] = 99;
  request.actorIds[0] = 99;
  assert.deepEqual(
    registry.resolve(ref.id, expected()).metadata.actorIds,
    [0, 17],
  );
  for (const query of [
    expected("night"),
    { ...expected(), modelIdentity: "4".repeat(64) },
    { ...expected(), actorIds: [17, 0] },
    { ...expected(), actorIds: [0] },
    { ...expected(), actorIds: [0, 18] },
    { ...expected(), actorIds: [0, 17, 18] },
  ])
    assert.throws(() => registry.resolve(ref.id, query));
  for (const options of [
    { sourceIdentity, incarnation: {} },
    { sourceIdentity: "4".repeat(64), incarnation },
  ])
    assert.throws(() =>
      new CaptureActorReferences(options).resolve(ref.id, expected()),
    );
});

test("one pin per scenario replaces old references, invalid pins leave current references intact", () => {
  const registry = new CaptureActorReferences({
    sourceIdentity,
    incarnation: {},
  });
  const day = registry.pin(pin()),
    night = registry.pin(pin("night"));
  const secondDay = registry.pin({ ...pin(), sourceFrame: 100 });
  assert.notEqual(day.id, secondDay.id);
  assert.throws(() => registry.resolve(day.id, expected()));
  assert.equal(
    registry.resolve(secondDay.id, expected()).metadata.sourceFrame,
    100,
  );
  assert.deepEqual(
    registry.resolve(night.id, expected("night")).metadata,
    night,
  );
  assert.throws(() => registry.pin({ ...pin(), actorIds: [0, 0] }));
  assert.deepEqual(
    registry.resolve(secondDay.id, expected()).metadata,
    secondDay,
  );
  for (let index = 0; index < 100; index++) registry.pin(pin());
  assert.throws(() => registry.resolve(secondDay.id, expected()));
  assert.deepEqual(
    registry.resolve(night.id, expected("night")).metadata,
    night,
  );
});

test("clear/dispose invalidate references without reusing IDs, including no-actor scenes", () => {
  const registry = new CaptureActorReferences({
    sourceIdentity,
    incarnation: {},
  });
  const empty = registry.pin({ ...pin(), actorIds: [] });
  assert.equal(empty.count, 0);
  assert.deepEqual(
    registry.resolve(empty.id, { ...expected(), actorIds: [] }).metadata
      .actorIds,
    [],
  );
  assert.throws(() => registry.resolve(empty.id, expected()));
  registry.clear();
  assert.throws(() =>
    registry.resolve(empty.id, { ...expected(), actorIds: [] }),
  );
  const next = registry.pin(pin());
  assert.notEqual(empty.id, next.id);
  registry.dispose();
  registry.dispose();
  assert.throws(() => registry.resolve(next.id, expected()));
  assert.throws(() => registry.pin(pin()));
  assert.throws(() => registry.clear());
  assert.throws(
    () =>
      new CaptureActorReferences({ sourceIdentity: "wrong", incarnation: {} }),
  );
  assert.throws(
    () =>
      new CaptureActorReferences({
        sourceIdentity,
        incarnation: null as never,
      }),
  );
});
