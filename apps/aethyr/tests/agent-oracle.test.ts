import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import {
  OWNED_STARTER_LANDMARKS,
  projectOracleBox,
  verifyAgentCapture,
  verifyCausalEdit,
  verifyQuestTransition,
  type OracleBox,
  type OracleCaptureExpectation,
  type OracleCausalExpectation,
  type OraclePose,
  type OracleReport,
} from "../packages/runtime/agent-oracle.ts";
import type {
  AgentCamera,
  AgentCaptureEnvelope,
  AgentContext,
  AgentInspection,
} from "../packages/runtime/agent-contract.ts";

const camera: AgentCamera = {
  eye: [0, 0, 5],
  target: [0, 0, 0],
  fov: Math.PI / 2,
};
const context = (revision = 1): AgentContext => ({
  worldId: "owned-starter",
  sourceIdentity: "a".repeat(64),
  revision,
  generation: revision,
  tick: 10,
  shaderIdentity: "b".repeat(64),
});
const centerBox: OracleBox = { min: [-1, -1, -1], max: [1, 1, 1] };
const beforeBox: OracleBox = { min: [-2, -0.5, -0.5], max: [-1, 0.5, 0.5] };
const afterBox: OracleBox = { min: [1, -0.5, -0.5], max: [2, 0.5, 0.5] };
const beforePose: OraclePose = { position: [-1.5, 0, 0], yaw: 0, scale: 1 };
const afterPose: OraclePose = { position: [1.5, 0, 0], yaw: 0, scale: 1 };
function pixels() {
  const bytes = new Uint8Array(100 * 100 * 4);
  for (let y = 0; y < 100; y++)
    for (let x = 0; x < 100; x++) {
      const gray = (x + y) % 2 ? 35 : 60;
      bytes.set([gray, gray, gray, 255], (y * 100 + x) * 4);
    }
  return bytes;
}
function patch(
  bytes: Uint8Array,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  color: [number, number, number] = [20, 190, 50],
) {
  for (let y = y0; y < y1; y++)
    for (let x = x0; x < x1; x++) bytes.set([...color, 255], (y * 100 + x) * 4);
  return bytes;
}
function capture(
  bytes: Uint8Array = pixels(),
  revision = 1,
): AgentCaptureEnvelope {
  return {
    schema: "aethyrforge.agent.capture.v1",
    requestId: `capture-${revision}`,
    sessionId: "session-1",
    sequence: revision,
    context: context(revision),
    frameBefore: revision * 2,
    frameAfter: revision * 2 + 1,
    camera: structuredClone(camera),
    width: 100,
    height: 100,
    time: 0,
    exposure: 1,
    pixelSha256: createHash("sha256").update(bytes).digest("hex"),
    pixels: bytes.slice().buffer,
  };
}
function expectation(revision = 1): OracleCaptureExpectation {
  return {
    ...context(revision),
    camera: structuredClone(camera),
    width: 100,
    height: 100,
    time: 0,
    exposure: 1,
    sessionId: "session-1",
    landmarks: [
      {
        id: "known-green-box",
        box: centerBox,
        color: "green",
        minCoverage: 0.2,
      },
    ],
  };
}
function failed(result: OracleReport, id: string) {
  assert.equal(result.ok, false);
  assert.equal(
    result.checks.find((check) => check.id === id)?.ok,
    false,
    `Expected failed ${id}: ${JSON.stringify(result)}`,
  );
}
function inspection(
  pose: OraclePose | null,
  revision: number,
): AgentInspection {
  return {
    schema: "aethyrforge.agent.v1",
    requestId: `inspect-${revision}`,
    sessionId: "session-1",
    context: context(revision),
    authoredIdentity: `${revision}`.repeat(64),
    objects: pose
      ? [
          {
            id: "probe",
            kind: "barrel",
            ...structuredClone(pose),
            collidable: true,
          },
        ]
      : [],
    totalObjects: pose ? 1 : 0,
    truncated: false,
    player: { position: [0, 0, 5], yaw: 0, pitch: 0 },
    view: { ...structuredClone(camera), mode: "fly" },
    input: { forward: 0, right: 0, up: 0, lookX: 0, lookY: 0, sprint: false },
    render: {
      frame: 10 + revision,
      instances: 20 + Number(!!pose),
      triangles: 200 + Number(!!pose) * 12,
    },
    capabilities: [],
  };
}
function causal(): {
  before: AgentCaptureEnvelope;
  after: AgentCaptureEnvelope;
  expected: OracleCausalExpectation;
} {
  // Literal test raster positions have an independently calculated perspective location.
  return {
    before: capture(patch(pixels(), 28, 45, 40, 55), 1),
    after: capture(patch(pixels(), 60, 45, 72, 55), 2),
    expected: {
      entityId: "probe",
      beforeBox,
      afterBox,
      beforePose,
      afterPose,
      beforeInspection: inspection(beforePose, 1),
      afterInspection: inspection(afterPose, 2),
      emptyCapture: capture(),
    },
  };
}

test("independent projection locates a known perspective box, clips edges, and rejects invalid cameras", () => {
  assert.deepEqual(projectOracleBox(camera, 100, 100, centerBox), {
    minX: 37,
    minY: 37,
    maxX: 63,
    maxY: 63,
  });
  assert.deepEqual(
    projectOracleBox({ ...camera, target: [0, 0, 4] }, 100, 100, centerBox),
    projectOracleBox(camera, 100, 100, centerBox),
  );
  assert.equal(
    projectOracleBox(camera, 100, 100, { min: [-1, -1, 6], max: [1, 1, 8] }),
    null,
  );
  assert.equal(
    projectOracleBox({ ...camera, target: [0, 0, 5] }, 100, 100, centerBox),
    null,
  );
  assert.equal(
    projectOracleBox({ ...camera, up: [0, 0, 1] }, 100, 100, centerBox),
    null,
  );
  assert.equal(projectOracleBox(camera, Infinity, 100, centerBox), null);
  const crossing = projectOracleBox(camera, 100, 100, {
    min: [-1, -1, 4.8],
    max: [1, 1, 5.1],
  });
  assert.deepEqual(crossing, { minX: 0, minY: 0, maxX: 100, maxY: 100 });
});

test("capture pins identity and actual camera while accepting equivalent target distance", async () => {
  const good = capture(patch(pixels(), 38, 38, 62, 62));
  good.camera.target = [0, 0, 4];
  assert.equal((await verifyAgentCapture(good, expectation())).ok, true);
  for (const key of [
    "sourceIdentity",
    "revision",
    "generation",
    "shaderIdentity",
  ] as const) {
    const bad = structuredClone(good);
    if (key === "revision" || key === "generation") bad.context[key]++;
    else bad.context[key] = "c".repeat(64);
    failed(await verifyAgentCapture(bad, expectation()), `identity.${key}`);
  }
  const wrongCamera = structuredClone(good);
  wrongCamera.camera.eye[0] = 10;
  failed(await verifyAgentCapture(wrongCamera, expectation()), "camera");
  const wrongUp = structuredClone(good);
  wrongUp.camera.up = [1, 0, 0];
  failed(await verifyAgentCapture(wrongUp, expectation()), "camera");
  const staleFrame = structuredClone(good);
  staleFrame.frameAfter = staleFrame.frameBefore;
  failed(await verifyAgentCapture(staleFrame, expectation()), "fresh-frame");
  failed(
    await verifyAgentCapture(good, { ...expectation(), previousSequence: 1 }),
    "sequence",
  );
});

test("forged hashes and newly hashed black, white, missing, and wrong-camera pixels fail independently", async () => {
  const good = capture(patch(pixels(), 38, 38, 62, 62));
  const forged = structuredClone(good);
  new Uint8Array(forged.pixels)[0]++;
  failed(await verifyAgentCapture(forged, expectation()), "pixel-sha256");
  for (const value of [0, 255]) {
    const flat = new Uint8Array(100 * 100 * 4).fill(value);
    for (let i = 3; i < flat.length; i += 4) flat[i] = 255;
    const actual = await verifyAgentCapture(capture(flat), expectation());
    assert.equal(
      actual.checks.find((check) => check.id === "pixel-sha256")?.ok,
      true,
    );
    failed(actual, "luma.range");
  }
  const missing = await verifyAgentCapture(capture(), expectation());
  assert.equal(
    missing.checks.find((check) => check.id === "pixel-sha256")?.ok,
    true,
  );
  failed(missing, "landmark.known-green-box");
  // Correct requested metadata cannot make green geometry in a different view pass.
  const wrongPixels = await verifyAgentCapture(
    capture(patch(pixels(), 5, 5, 20, 20)),
    expectation(),
  );
  assert.equal(
    wrongPixels.checks.find((check) => check.id === "camera")?.ok,
    true,
  );
  failed(wrongPixels, "landmark.known-green-box");
});

test("capture camera proof includes vignette settings and preserves omitted legacy defaults", async () => {
  const good = capture(patch(pixels(), 38, 38, 62, 62)),
    expected = expectation(),
    vignette = {
      shape: "cosine-fourth" as const,
      intensity: 0.7,
      blendWeight: 1,
    };
  good.camera.vignette = null;
  assert.equal((await verifyAgentCapture(good, expected)).ok, true);
  good.camera.vignette = vignette;
  failed(await verifyAgentCapture(good, expected), "camera");
  expected.camera = { ...expected.camera, vignette };
  assert.equal((await verifyAgentCapture(good, expected)).ok, true);
  for (const value of [
    null,
    { ...vignette, intensity: 0.2 },
    { ...vignette, blendWeight: 0 },
  ]) {
    const bad = structuredClone(good);
    bad.camera.vignette = value;
    failed(await verifyAgentCapture(bad, expected), "camera");
  }
  const malformed = structuredClone(good);
  (malformed.camera.vignette as any).intensity = 2;
  failed(await verifyAgentCapture(malformed, expected), "camera");
});

test("capture and causal evidence cannot silently substitute another pinned actor pose", async () => {
  const image = capture(patch(pixels(), 38, 38, 62, 62)),
    expected = expectation();
  const actors = {
    schema: "aethyrforge.capture-actors.v1" as const,
    id: "actors-1",
    sourceIdentity: image.context.sourceIdentity,
    scenario: "day" as const,
    modelIdentity: "a".repeat(64),
    sourceFrame: 3,
    actorIds: [0, 1],
    count: 2,
    poseSha256: "b".repeat(64),
  };
  image.actors = actors;
  failed(await verifyAgentCapture(image, expected), "actor-reference");
  expected.actorReference = structuredClone(actors);
  assert.equal((await verifyAgentCapture(image, expected)).ok, true);
  for (const field of [
    "poseSha256",
    "modelIdentity",
    "sourceIdentity",
  ] as const) {
    const changed = structuredClone(image);
    changed.actors![field] = "c".repeat(64);
    failed(await verifyAgentCapture(changed, expected), "actor-reference");
  }
});

test("capture validates raw byte count, alpha, requested settings and owns pixels across crypto await", async () => {
  const good = capture(patch(pixels(), 38, 38, 62, 62));
  const badLength = structuredClone(good);
  badLength.pixels = new ArrayBuffer(4);
  failed(await verifyAgentCapture(badLength, expectation()), "rgba-length");
  const transparent = new Uint8Array(good.pixels.slice(0));
  transparent[3] = 0;
  failed(
    await verifyAgentCapture(capture(transparent), expectation()),
    "alpha",
  );
  failed(await verifyAgentCapture(good, { ...expectation(), time: 1 }), "time");
  failed(
    await verifyAgentCapture(good, { ...expectation(), exposure: 2 }),
    "exposure",
  );
  failed(
    await verifyAgentCapture(good, { ...expectation(), requestId: "other" }),
    "identity.requestId",
  );
  const noContext = structuredClone(good);
  delete (noContext as Partial<AgentCaptureEnvelope>).context;
  failed(await verifyAgentCapture(noContext, expectation()), "context");
  const pending = verifyAgentCapture(good, expectation());
  new Uint8Array(good.pixels).fill(0);
  assert.equal((await pending).ok, true);
});

test("causal move checks expected poses, source context, real geometry, and both projected regions", () => {
  const { before, after, expected } = causal();
  assert.equal(verifyCausalEdit(before, after, expected).ok, true);
  const badPose = structuredClone(expected);
  badPose.afterInspection.objects[0].position[0] = 99;
  failed(verifyCausalEdit(before, after, badPose), "causal.after-pose");
  const badContext = structuredClone(after);
  badContext.context.revision = 99;
  failed(
    verifyCausalEdit(before, badContext, expected),
    "causal.capture-context",
  );
  const noGeometry = structuredClone(expected);
  noGeometry.afterInspection.render.triangles = 0;
  failed(
    verifyCausalEdit(before, after, noGeometry),
    "causal.rendered-geometry",
  );
  const duplicate = structuredClone(expected);
  duplicate.afterInspection.objects.push(duplicate.afterInspection.objects[0]);
  failed(verifyCausalEdit(before, after, duplicate), "causal.after-pose");
});

test("fresh metadata and recomputed hashes cannot bless a missing prop or stale pre-move image", async () => {
  const { before, after, expected } = causal();
  const missing = capture(pixels(), 2);
  const stale = capture(new Uint8Array(before.pixels), 2);
  for (const wrong of [missing, stale]) {
    const metadata = await verifyAgentCapture(wrong, {
      ...expectation(2),
      landmarks: [],
    });
    assert.equal(metadata.ok, true);
    failed(verifyCausalEdit(before, wrong, expected), "causal.after-occupied");
  }
  failed(verifyCausalEdit(before, stale, expected), "causal.pixels");
  const unrelated = capture(patch(pixels(), 3, 3, 20, 20), 2);
  failed(verifyCausalEdit(before, unrelated, expected), "causal.after-region");
  assert.equal(verifyCausalEdit(before, after, expected).ok, true);
});

test("causal add/remove require actual submitted count changes and complete absence inspection", () => {
  const { after, expected } = causal();
  const empty = capture(pixels(), 1),
    emptyInspection = inspection(null, 1);
  const add = {
    ...expected,
    beforeBox: null,
    beforePose: null,
    beforeInspection: emptyInspection,
  };
  assert.equal(verifyCausalEdit(empty, after, add).ok, true);
  const noSourceGeometry = structuredClone(add);
  noSourceGeometry.beforeInspection.render.instances = 0;
  noSourceGeometry.beforeInspection.render.triangles = 0;
  assert.equal(verifyCausalEdit(empty, after, noSourceGeometry).ok, true);
  const fakeCounts = structuredClone(add);
  fakeCounts.afterInspection.render = { ...fakeCounts.beforeInspection.render };
  failed(
    verifyCausalEdit(empty, after, fakeCounts),
    "causal.rendered-geometry",
  );
  const partial = structuredClone(add);
  partial.beforeInspection.totalObjects = 1;
  failed(verifyCausalEdit(empty, after, partial), "causal.before-pose");
  const removed = capture(pixels(), 3);
  assert.equal(
    verifyCausalEdit(after, removed, {
      entityId: "probe",
      beforeBox: afterBox,
      afterBox: null,
      beforePose: afterPose,
      afterPose: null,
      beforeInspection: inspection(afterPose, 2),
      afterInspection: inspection(null, 3),
      emptyCapture: empty,
    }).ok,
    true,
  );
});

test("absence uses complete authored enumeration independently of truncated read-only geometry", () => {
  const { after, expected } = causal();
  const empty = capture(pixels(), 1),
    complete = inspection(null, 1);
  complete.truncated = true;
  complete.readOnlyTotal = 8_096;
  complete.readOnlyObjects = [
    {
      id: "source-0",
      kind: "source-mesh",
      position: [0, 0, 0],
      editable: false,
    },
  ];
  const add = {
    ...expected,
    beforeBox: null,
    beforePose: null,
    beforeInspection: complete,
  };
  assert.equal(verifyCausalEdit(empty, after, add).ok, true);
  const incomplete = structuredClone(add);
  incomplete.beforeInspection.totalObjects = 2;
  incomplete.beforeInspection.truncated = false;
  failed(verifyCausalEdit(empty, after, incomplete), "causal.before-pose");
  const paged = structuredClone(add);
  paged.beforeInspection.nextCursor = 1;
  failed(verifyCausalEdit(empty, after, paged), "causal.before-pose");
  const duplicate = structuredClone(add);
  const anchor = { ...inspection(beforePose, 1).objects[0], id: "anchor" };
  duplicate.beforeInspection.objects = [anchor, anchor];
  duplicate.beforeInspection.totalObjects = 2;
  failed(verifyCausalEdit(empty, after, duplicate), "causal.before-pose");
});

test("quest oracle independently rejects wrong phases, acceptance and authored state changes", () => {
  const base = { beforeRevision: 7, afterRevision: 7 };
  for (const transition of [
    {
      action: "collect-key",
      beforePhase: "find-key",
      afterPhase: "find-exit",
      accepted: true,
    },
    {
      action: "unlock-exit",
      beforePhase: "find-exit",
      afterPhase: "complete",
      accepted: true,
    },
    {
      action: "reset",
      beforePhase: "complete",
      afterPhase: "find-key",
      accepted: true,
    },
    {
      action: "locked-exit",
      beforePhase: "find-key",
      afterPhase: "find-key",
      accepted: false,
    },
    {
      action: "out-of-range",
      beforePhase: "find-exit",
      afterPhase: "find-exit",
      accepted: false,
    },
    {
      action: "finished-interact",
      beforePhase: "complete",
      afterPhase: "complete",
      accepted: false,
    },
  ] as const)
    assert.equal(verifyQuestTransition({ ...base, ...transition }).ok, true);
  failed(
    verifyQuestTransition({
      ...base,
      action: "collect-key",
      beforePhase: "find-key",
      afterPhase: "complete",
      accepted: true,
    }),
    "quest.phase",
  );
  failed(
    verifyQuestTransition({
      ...base,
      action: "locked-exit",
      beforePhase: "find-key",
      afterPhase: "find-key",
      accepted: true,
    }),
    "quest.accepted",
  );
  failed(
    verifyQuestTransition({
      ...base,
      action: "reset",
      beforePhase: "complete",
      afterPhase: "find-key",
      accepted: true,
      afterRevision: 8,
    }),
    "quest.authored-revision",
  );
});

test("owned landmark specification matches independently pinned source geometry and color", async () => {
  assert.deepEqual(OWNED_STARTER_LANDMARKS[0], {
    id: "exit-lintel",
    color: "green",
    box: { min: [0.9, 1.9, -3.15], max: [3.1, 2.1, -2.85] },
  });
  assert.equal(
    projectOracleBox(
      { eye: [0, 5, 7], target: [0, 1, -2], fov: 0.9 },
      512,
      320,
      OWNED_STARTER_LANDMARKS[0].box,
    ) !== null,
    true,
  );
  const source = JSON.parse(
    await readFile(
      new URL("../public/starter/scene.json", import.meta.url),
      "utf8",
    ),
  );
  for (const landmark of OWNED_STARTER_LANDMARKS) {
    const instance = source.instances.find(
      (value: { actor: string }) => value.actor === landmark.id,
    );
    assert.ok(instance, `${landmark.id} remains part of the owned fixture`);
    assert.equal(instance.mesh, 0);
    assert.deepEqual(instance.materialOverrides, [3]);
    const model = instance.model as number[];
    // Fixture landmarks are axis-aligned unit boxes; this assertion pins that assumption.
    for (const index of [1, 2, 3, 4, 6, 7, 8, 9, 11])
      assert.equal(model[index], 0);
    for (let axis = 0; axis < 3; axis++) {
      assert.ok(
        Math.abs(
          model[12 + axis] - model[axis * 5] / 2 - landmark.box.min[axis],
        ) < 1e-12,
      );
      assert.ok(
        Math.abs(
          model[12 + axis] + model[axis * 5] / 2 - landmark.box.max[axis],
        ) < 1e-12,
      );
    }
  }
  assert.deepEqual(source.materials[3].baseColorFactor, [0.12, 0.7, 0.3, 1]);
});
