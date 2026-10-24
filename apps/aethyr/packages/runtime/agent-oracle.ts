import type {
  AgentCamera,
  AgentCaptureCamera,
  AgentCaptureEnvelope,
  AgentExpectation,
  AgentInspection,
  AgentVector,
} from "./agent-contract.ts";
import { validateAgentCaptureCamera } from "./agent-contract.ts";
import {
  sameCaptureActorReference,
  type ActorCaptureReferenceMetadata,
} from "./capture-actors.ts";

export interface OracleCheck {
  id: string;
  ok: boolean;
  detail: string;
  actual?: number | string | boolean | null;
  expected?: number | string | boolean | null;
}
export interface OracleReport {
  ok: boolean;
  checks: OracleCheck[];
  metrics: Record<string, number>;
}
export interface OracleBox {
  min: AgentVector;
  max: AgentVector;
}
/** Pixel bounds are clamped to the image; maxX and maxY are exclusive. */
export interface OracleRect {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}
export interface OracleLandmark {
  id: string;
  box: OracleBox;
  color: "gold" | "green" | "brown" | "any";
  minPixels?: number;
  minCoverage?: number;
}
export interface OracleCaptureExpectation extends AgentExpectation {
  actorReference?: ActorCaptureReferenceMetadata;
  width: number;
  height: number;
  camera: AgentCaptureCamera;
  time: number;
  exposure: number;
  worldId?: string;
  sessionId?: string;
  requestId?: string;
  previousSequence?: number;
  landmarks?: readonly OracleLandmark[];
  minLumaRange?: number;
  minLumaStdDev?: number;
}
export interface OraclePose {
  position: AgentVector;
  yaw: number;
  scale: number;
}
export interface OracleCausalExpectation {
  entityId: string;
  beforeBox: OracleBox | null;
  afterBox: OracleBox | null;
  beforePose: OraclePose | null;
  afterPose: OraclePose | null;
  beforeInspection: AgentInspection;
  afterInspection: AgentInspection;
  /** A fixed-camera empty control, captured before adding the visible prop. */
  emptyCapture?: AgentCaptureEnvelope;
  minChangedPixels?: number;
  minRegionCoverage?: number;
}

/** Owned fixture geometry, independent of its renderer and generated captures. */
export const OWNED_STARTER_LANDMARKS: readonly OracleLandmark[] = [
  {
    id: "exit-lintel",
    box: { min: [0.9, 1.9, -3.15], max: [3.1, 2.1, -2.85] },
    color: "green",
  },
  {
    id: "exit-left-post",
    box: { min: [0.91, 0, -3.15], max: [1.09, 2, -2.85] },
    color: "green",
  },
];

const finiteVector = (v: AgentVector) =>
  Array.isArray(v) && v.length === 3 && v.every(Number.isFinite);
const minus = (a: AgentVector, b: AgentVector): AgentVector => [
  a[0] - b[0],
  a[1] - b[1],
  a[2] - b[2],
];
const dot = (a: AgentVector, b: AgentVector) =>
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: AgentVector, b: AgentVector): AgentVector => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
function unit(v: AgentVector): AgentVector | null {
  if (!finiteVector(v)) return null;
  const length = Math.hypot(...v);
  return length > 1e-9 ? (v.map((n) => n / length) as AgentVector) : null;
}
function basis(camera: AgentCamera) {
  if (
    !camera ||
    !finiteVector(camera.eye) ||
    !finiteVector(camera.target) ||
    !Number.isFinite(camera.fov) ||
    camera.fov < 0.15 ||
    camera.fov > 2.5
  )
    return null;
  const forward = unit(minus(camera.target, camera.eye));
  const up = unit(camera.up ?? [0, 1, 0]);
  if (!forward || !up) return null;
  const right = unit(cross(forward, up));
  return right ? { forward, right, up: cross(right, forward) } : null;
}
const near = (a: number, b: number, epsilon = 1e-5) =>
  Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= epsilon;
const vectorNear = (a: AgentVector, b: AgentVector, epsilon = 1e-5) =>
  finiteVector(a) &&
  finiteVector(b) &&
  a.every((n, i) => near(n, b[i], epsilon));
function sameCamera(a: AgentCaptureCamera, b: AgentCaptureCamera) {
  try {
    validateAgentCaptureCamera(a);
    validateAgentCaptureCamera(b);
  } catch {
    return false;
  }
  const first = basis(a),
    second = basis(b);
  return (
    !!first &&
    !!second &&
    vectorNear(a.eye, b.eye) &&
    vectorNear(first.forward, second.forward) &&
    vectorNear(first.up, second.up) &&
    near(a.fov, b.fov) &&
    a.vignette?.shape === b.vignette?.shape &&
    a.vignette?.intensity === b.vignette?.intensity &&
    a.vignette?.blendWeight === b.vignette?.blendWeight
  );
}
const dimensions = (width: number, height: number) =>
  Number.isInteger(width) &&
  Number.isInteger(height) &&
  width >= 1 &&
  height >= 1 &&
  width <= 1920 &&
  height <= 1080;
const validBox = (box: OracleBox) =>
  !!box &&
  finiteVector(box.min) &&
  finiteVector(box.max) &&
  box.min.every((n, i) => n < box.max[i]);

/** Analytic perspective projection; deliberately does not import renderer math. */
export function projectOracleBox(
  camera: AgentCamera,
  width: number,
  height: number,
  box: OracleBox,
): OracleRect | null {
  const axes = basis(camera);
  if (!axes || !dimensions(width, height) || !validBox(box)) return null;
  const points: AgentVector[] = [];
  for (let bits = 0; bits < 8; bits++) {
    const relative = minus(
      [
        bits & 1 ? box.max[0] : box.min[0],
        bits & 2 ? box.max[1] : box.min[1],
        bits & 4 ? box.max[2] : box.min[2],
      ],
      camera.eye,
    );
    points.push([
      dot(relative, axes.right),
      dot(relative, axes.up),
      dot(relative, axes.forward),
    ]);
  }
  // Clip the twelve box edges to the camera near plane before dividing by depth.
  const clipped = points.filter((point) => point[2] >= 0.1);
  for (let corner = 0; corner < 8; corner++)
    for (const bit of [1, 2, 4]) {
      if (corner & bit) continue;
      const a = points[corner],
        b = points[corner | bit];
      if (a[2] < 0.1 === b[2] < 0.1) continue;
      const t = (0.1 - a[2]) / (b[2] - a[2]);
      clipped.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), 0.1]);
    }
  if (!clipped.length) return null;
  const tan = Math.tan(camera.fov / 2),
    aspect = width / height;
  const xs = clipped.map(
    ([x, , depth]) => (width * (1 + x / (depth * tan * aspect))) / 2,
  );
  const ys = clipped.map(
    ([, y, depth]) => (height * (1 - y / (depth * tan))) / 2,
  );
  const rect = {
    minX: Math.max(0, Math.floor(Math.min(...xs))),
    minY: Math.max(0, Math.floor(Math.min(...ys))),
    maxX: Math.min(width, Math.ceil(Math.max(...xs))),
    maxY: Math.min(height, Math.ceil(Math.max(...ys))),
  };
  return rect.minX < rect.maxX && rect.minY < rect.maxY ? rect : null;
}
function report(): OracleReport {
  return { ok: true, checks: [], metrics: {} };
}
function check(
  result: OracleReport,
  id: string,
  ok: boolean,
  detail: string,
  actual?: OracleCheck["actual"],
  expected?: OracleCheck["expected"],
) {
  result.checks.push({
    id,
    ok,
    detail,
    ...(actual !== undefined ? { actual } : {}),
    ...(expected !== undefined ? { expected } : {}),
  });
  result.ok &&= ok;
}
function rgba(capture: AgentCaptureEnvelope): Uint8Array | null {
  if (
    !capture ||
    !dimensions(capture.width, capture.height) ||
    !(capture.pixels instanceof ArrayBuffer) ||
    capture.pixels.byteLength !== capture.width * capture.height * 4
  )
    return null;
  return new Uint8Array(capture.pixels);
}
function validContext(capture: AgentCaptureEnvelope) {
  const context = capture?.context;
  return (
    !!context &&
    typeof context.worldId === "string" &&
    /^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,95}$/.test(context.worldId) &&
    /^[a-f0-9]{64}$/.test(context.sourceIdentity) &&
    /^[a-f0-9]{64}$/.test(context.shaderIdentity) &&
    [context.revision, context.generation, context.tick].every(
      (value) => Number.isSafeInteger(value) && value >= 0,
    )
  );
}
function matchesColor(
  r: number,
  g: number,
  b: number,
  color: OracleLandmark["color"],
) {
  if (Math.max(r, g, b) < 18) return false;
  if (color === "green")
    return g > r * 1.12 && g > b * 1.08 && g - Math.min(r, b) > 12;
  if (color === "gold") return r > g * 1.12 && g > b * 1.2 && r - b > 25;
  if (color === "brown") return r > g * 1.07 && g > b * 1.05 && r - b > 12;
  return true;
}
function area(rect: OracleRect) {
  return (rect.maxX - rect.minX) * (rect.maxY - rect.minY);
}
function eachPixel(
  rect: OracleRect,
  width: number,
  visit: (offset: number) => void,
) {
  for (let y = rect.minY; y < rect.maxY; y++)
    for (let x = rect.minX; x < rect.maxX; x++) visit((y * width + x) * 4);
}

/** Check declared identity and independent image evidence; no reference image is blessed. */
export async function verifyAgentCapture(
  capture: AgentCaptureEnvelope,
  expected: OracleCaptureExpectation,
): Promise<OracleReport> {
  const result = report(),
    bytes = rgba(capture);
  check(
    result,
    "schema",
    capture?.schema === "aethyrforge.agent.capture.v1",
    "Use the versioned capture envelope.",
  );
  check(
    result,
    "dimensions",
    dimensions(expected.width, expected.height) &&
      capture?.width === expected.width &&
      capture?.height === expected.height,
    "Capture dimensions match the requested view.",
  );
  check(
    result,
    "rgba-length",
    !!bytes,
    "Capture contains tightly packed RGBA8 pixels.",
  );
  check(
    result,
    "context",
    validContext(capture),
    "Capture has a complete versioned world context.",
  );
  if (!bytes || !validContext(capture)) return result;
  // Take owned copies before awaiting crypto; mutable caller buffers cannot change a verdict.
  const pixels = bytes.slice();
  const context = { ...capture.context };
  const camera = structuredClone(capture.camera);
  for (const key of [
    "sourceIdentity",
    "revision",
    "generation",
    "shaderIdentity",
  ] as const)
    check(
      result,
      `identity.${key}`,
      context[key] === expected[key],
      `Capture matches expected ${key}.`,
      context[key],
      expected[key],
    );
  for (const key of ["worldId", "sessionId", "requestId"] as const)
    if (expected[key] !== undefined) {
      const actual = key === "worldId" ? context.worldId : capture[key];
      check(
        result,
        `identity.${key}`,
        actual === expected[key],
        `Capture matches expected ${key}.`,
        actual,
        expected[key],
      );
    }
  check(
    result,
    "sequence",
    Number.isSafeInteger(capture.sequence) &&
      capture.sequence > (expected.previousSequence ?? 0),
    "Capture sequence advances beyond the previously consumed result.",
  );
  check(
    result,
    "fresh-frame",
    Number.isSafeInteger(capture.frameBefore) &&
      capture.frameBefore >= 0 &&
      Number.isSafeInteger(capture.frameAfter) &&
      capture.frameAfter > capture.frameBefore,
    "Capture rendered a new frame.",
  );
  check(
    result,
    "camera",
    sameCamera(camera, expected.camera),
    "Actual camera matches the expected eye, orientation, and field of view.",
  );
  check(
    result,
    "actor-reference",
    expected.actorReference
      ? sameCaptureActorReference(capture.actors, expected.actorReference) &&
          capture.actors?.sourceIdentity === context.sourceIdentity
      : capture.actors === undefined,
    "Capture uses the requested bounded actor poses, or the live actors when omitted.",
  );
  check(
    result,
    "time",
    near(capture.time, expected.time),
    "Capture uses the expected scene time.",
  );
  check(
    result,
    "exposure",
    near(capture.exposure, expected.exposure),
    "Capture uses the expected exposure.",
  );
  const declaredHash = capture.pixelSha256;
  const actualHash = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", pixels.buffer)),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("");
  check(
    result,
    "pixel-sha256",
    /^[a-f0-9]{64}$/.test(declaredHash) && actualHash === declaredHash,
    "Pixel bytes match their recorded SHA-256.",
    actualHash,
    declaredHash,
  );
  let min = 255,
    max = 0,
    sum = 0,
    squares = 0,
    opaque = 0;
  for (let i = 0; i < pixels.length; i += 4) {
    const luma =
      0.2126 * pixels[i] + 0.7152 * pixels[i + 1] + 0.0722 * pixels[i + 2];
    min = Math.min(min, luma);
    max = Math.max(max, luma);
    sum += luma;
    squares += luma * luma;
    if (pixels[i + 3] === 255) opaque++;
  }
  const count = pixels.length / 4,
    mean = sum / count;
  const std = Math.sqrt(Math.max(0, squares / count - mean * mean));
  Object.assign(result.metrics, {
    lumaMin: min,
    lumaMax: max,
    lumaStdDev: std,
    opaquePixels: opaque,
  });
  check(
    result,
    "luma.range",
    max - min >= (expected.minLumaRange ?? 12),
    "Image has a useful brightness range.",
    max - min,
    expected.minLumaRange ?? 12,
  );
  check(
    result,
    "luma.variation",
    std >= (expected.minLumaStdDev ?? 3),
    "Image contains more than a flat field.",
    std,
    expected.minLumaStdDev ?? 3,
  );
  check(
    result,
    "alpha",
    opaque === count,
    "Every captured pixel is opaque.",
    opaque,
    count,
  );
  for (const landmark of expected.landmarks ?? []) {
    const rect = projectOracleBox(
      expected.camera,
      expected.width,
      expected.height,
      landmark.box,
    );
    let found = 0;
    if (rect)
      eachPixel(rect, expected.width, (i) => {
        if (
          matchesColor(pixels[i], pixels[i + 1], pixels[i + 2], landmark.color)
        )
          found++;
      });
    const coverage = rect ? found / area(rect) : 0;
    result.metrics[`landmark.${landmark.id}.pixels`] = found;
    result.metrics[`landmark.${landmark.id}.coverage`] = coverage;
    check(
      result,
      `landmark.${landmark.id}`,
      !!rect &&
        found >= (landmark.minPixels ?? 4) &&
        coverage >= (landmark.minCoverage ?? 0.01),
      `Expected ${landmark.color} geometry is visible in the projected ${landmark.id} region.`,
      found,
      landmark.minPixels ?? 4,
    );
  }
  return result;
}

function sameContext(
  capture: AgentCaptureEnvelope,
  inspection: AgentInspection,
) {
  return (
    validContext(capture) &&
    [
      "worldId",
      "sourceIdentity",
      "revision",
      "generation",
      "shaderIdentity",
    ].every(
      (key) =>
        capture.context[key as keyof AgentExpectation | "worldId"] ===
        inspection.context[key as keyof AgentExpectation | "worldId"],
    )
  );
}
function validPose(pose: OraclePose) {
  return (
    finiteVector(pose.position) &&
    Number.isFinite(pose.yaw) &&
    Number.isFinite(pose.scale) &&
    pose.scale > 0
  );
}
function inspectPose(
  inspection: AgentInspection,
  id: string,
  expected: OraclePose | null,
) {
  const objects = inspection.objects.filter((object) => object.id === id);
  const uniqueIds =
    new Set(inspection.objects.map((object) => object.id)).size ===
    inspection.objects.length;
  if (!uniqueIds) return false;
  // Read-only source samples can be truncated even when authored enumeration is complete.
  if (!expected)
    return (
      objects.length === 0 &&
      inspection.objects.length === inspection.totalObjects &&
      inspection.nextCursor === undefined
    );
  if (!validPose(expected) || objects.length !== 1) return false;
  const actual = objects[0],
    angle = Math.atan2(
      Math.sin(actual.yaw - expected.yaw),
      Math.cos(actual.yaw - expected.yaw),
    );
  return (
    vectorNear(actual.position, expected.position, 1e-4) &&
    near(angle, 0, 1e-4) &&
    near(actual.scale, expected.scale, 1e-4)
  );
}
function difference(
  a: Uint8Array,
  b: Uint8Array,
  width: number,
  rect: OracleRect,
) {
  let changed = 0;
  eachPixel(rect, width, (i) => {
    // Ignore small rounding and temporal noise while retaining visible material changes.
    if (
      Math.max(
        Math.abs(a[i] - b[i]),
        Math.abs(a[i + 1] - b[i + 1]),
        Math.abs(a[i + 2] - b[i + 2]),
      ) >= 12
    )
      changed++;
  });
  return changed;
}

/** A causal edit oracle: authored pose, submitted geometry, and pixels must agree. */
export function verifyCausalEdit(
  before: AgentCaptureEnvelope,
  after: AgentCaptureEnvelope,
  expected: OracleCausalExpectation,
): OracleReport {
  const result = report(),
    a = rgba(before),
    b = rgba(after);
  const compatible =
    !!a &&
    !!b &&
    before.width === after.width &&
    before.height === after.height &&
    sameCamera(before.camera, after.camera) &&
    ((before.actors === undefined && after.actors === undefined) ||
      sameCaptureActorReference(before.actors, after.actors)) &&
    near(before.time, after.time) &&
    near(before.exposure, after.exposure) &&
    before.context.sourceIdentity === after.context.sourceIdentity &&
    before.context.shaderIdentity === after.context.shaderIdentity;
  check(
    result,
    "causal.fixed-view",
    compatible,
    "Compare one source, shader, camera, time, exposure, and image size.",
  );
  check(
    result,
    "causal.capture-context",
    sameContext(before, expected.beforeInspection) &&
      sameContext(after, expected.afterInspection),
    "Each image belongs to the independently inspected authored state.",
  );
  check(
    result,
    "causal.before-pose",
    inspectPose(
      expected.beforeInspection,
      expected.entityId,
      expected.beforePose,
    ),
    "Entity starts at its expected pose or is absent.",
  );
  check(
    result,
    "causal.after-pose",
    inspectPose(
      expected.afterInspection,
      expected.entityId,
      expected.afterPose,
    ),
    "Entity ends at its expected pose or is absent.",
  );
  const shape =
    !!(expected.beforePose || expected.afterPose) &&
    !!expected.beforePose === !!expected.beforeBox &&
    !!expected.afterPose === !!expected.afterBox;
  check(
    result,
    "causal.expected-shape",
    shape,
    "Each expected present entity has an independently supplied world-space box.",
  );
  const first = expected.beforeInspection.render,
    last = expected.afterInspection.render;
  const countDirection =
    expected.beforePose && expected.afterPose
      ? true
      : expected.afterPose
        ? last.instances > first.instances && last.triangles > first.triangles
        : last.instances < first.instances && last.triangles < first.triangles;
  check(
    result,
    "causal.rendered-geometry",
    [first, last].every(
      (render, index) =>
        Number.isSafeInteger(render.frame) &&
        render.frame > 0 &&
        Number.isSafeInteger(render.instances) &&
        render.instances >= 0 &&
        Number.isSafeInteger(render.triangles) &&
        render.triangles >= 0 &&
        (!(index === 0 ? expected.beforePose : expected.afterPose) ||
          (render.instances > 0 && render.triangles > 0)),
    ) && !!countDirection,
    "Real submitted geometry is present and add/remove changes its counts.",
  );
  if (!compatible || !a || !b || !shape) return result;
  const threshold = expected.minChangedPixels ?? 8,
    coverage = expected.minRegionCoverage ?? 0.015;
  const whole = { minX: 0, minY: 0, maxX: before.width, maxY: before.height };
  result.metrics.changedPixels = difference(a, b, before.width, whole);
  check(
    result,
    "causal.pixels",
    result.metrics.changedPixels >= threshold,
    "An accepted edit changes actual pixels.",
    result.metrics.changedPixels,
    threshold,
  );
  for (const [name, box] of [
    ["before", expected.beforeBox],
    ["after", expected.afterBox],
  ] as const) {
    if (!box) continue;
    const rect = projectOracleBox(
      before.camera,
      before.width,
      before.height,
      box,
    );
    const changed = rect ? difference(a, b, before.width, rect) : 0;
    result.metrics[`${name}RegionChangedPixels`] = changed;
    check(
      result,
      `causal.${name}-region`,
      !!rect && changed >= threshold && changed / area(rect) >= coverage,
      `Pixels change where the independently projected ${name} box is expected.`,
      changed,
      threshold,
    );
  }
  if (expected.emptyCapture) {
    const empty = expected.emptyCapture,
      pixels = rgba(empty);
    const fixed =
      !!pixels &&
      empty.width === before.width &&
      empty.height === before.height &&
      sameCamera(empty.camera, before.camera) &&
      ((empty.actors === undefined && before.actors === undefined) ||
        sameCaptureActorReference(empty.actors, before.actors)) &&
      near(empty.time, before.time) &&
      near(empty.exposure, before.exposure) &&
      empty.context.sourceIdentity === before.context.sourceIdentity &&
      empty.context.shaderIdentity === before.context.shaderIdentity;
    check(
      result,
      "causal.empty-control-view",
      fixed,
      "Empty control uses the same fixed view and rendering inputs.",
    );
    if (fixed && pixels)
      for (const [name, box, image] of [
        ["before", expected.beforeBox, a],
        ["after", expected.afterBox, b],
      ] as const) {
        if (!box) continue;
        const rect = projectOracleBox(
          before.camera,
          before.width,
          before.height,
          box,
        );
        const changed = rect
          ? difference(pixels, image, before.width, rect)
          : 0;
        result.metrics[`${name}OccupiedPixels`] = changed;
        check(
          result,
          `causal.${name}-occupied`,
          !!rect && changed >= threshold && changed / area(rect) >= coverage,
          `Expected ${name} geometry changes its projected region relative to the empty control.`,
          changed,
          threshold,
        );
      }
  }
  return result;
}

export type OracleQuestPhase = "find-key" | "find-exit" | "complete";
export interface OracleQuestTransition {
  action:
    | "collect-key"
    | "unlock-exit"
    | "reset"
    | "locked-exit"
    | "out-of-range"
    | "finished-interact";
  beforePhase: OracleQuestPhase;
  afterPhase: OracleQuestPhase;
  beforeRevision: number;
  afterRevision: number;
  accepted: boolean;
}
/** Product rule expectations, intentionally independent of the game implementation. */
export function verifyQuestTransition(
  transition: OracleQuestTransition,
): OracleReport {
  const result = report();
  const expected: Record<
    OracleQuestTransition["action"],
    { from?: OracleQuestPhase; to?: OracleQuestPhase; accepted: boolean }
  > = {
    "collect-key": { from: "find-key", to: "find-exit", accepted: true },
    "unlock-exit": { from: "find-exit", to: "complete", accepted: true },
    reset: { to: "find-key", accepted: true },
    "locked-exit": { from: "find-key", accepted: false },
    "out-of-range": { accepted: false },
    "finished-interact": { from: "complete", accepted: false },
  };
  const rule = expected[transition.action];
  check(
    result,
    "quest.action",
    !!rule,
    "Use a known independently expected quest action.",
  );
  if (!rule) return result;
  check(
    result,
    "quest.known-phases",
    [transition.beforePhase, transition.afterPhase].every((phase) =>
      ["find-key", "find-exit", "complete"].includes(phase),
    ),
    "Both quest phases are supported.",
  );
  check(
    result,
    "quest.phase",
    (!rule.from || rule.from === transition.beforePhase) &&
      (rule.to ?? transition.beforePhase) === transition.afterPhase,
    "Quest phase follows the fixed key, exit, and reset rules.",
  );
  check(
    result,
    "quest.accepted",
    transition.accepted === rule.accepted,
    "Acceptance matches the expected action.",
    transition.accepted,
    rule.accepted,
  );
  check(
    result,
    "quest.authored-revision",
    Number.isSafeInteger(transition.beforeRevision) &&
      transition.beforeRevision >= 0 &&
      transition.beforeRevision === transition.afterRevision,
    "Live quest progress does not edit the authored world.",
    transition.afterRevision,
    transition.beforeRevision,
  );
  return result;
}
