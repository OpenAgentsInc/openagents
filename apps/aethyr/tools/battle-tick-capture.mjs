/**
 * Battle Lab fixed-tick capture helpers (issue 12599). Pure functions shared by
 * `capture-battle-tick.mjs` and its host-side tests; no browser or GPU here.
 */
import { createHash } from "node:crypto";

/** Matches `BATTLE_HOLD_LIMITS.maxTick` in packages/renderer/battle-crowd.ts. */
export const BATTLE_TICK_MAX = 36_000;
export const BATTLE_TICK_VIEWS = Object.freeze(["overview", "orbit", "dolly"]);
/**
 * GPU counters that depend on the camera (culling, LOD and draw sizes), not on
 * the simulation. The state digest leaves them out so it compares only the
 * battle itself; the image comparison covers what the camera sees.
 */
export const BATTLE_VIEW_COUNTERS = Object.freeze([
  "near",
  "far",
  "visible",
  "culled",
  "visibleClasses",
  "triangles",
  "projectilesVisible",
]);

const sha = (value) => createHash("sha256").update(value).digest("hex");

function integer(env, name, fallback, low, high) {
  const raw = env[name] ?? fallback;
  const n = Number(raw);
  if (!Number.isSafeInteger(n) || n < low || n > high)
    throw new Error(`${name} must be an integer from ${low} to ${high}.`);
  return n;
}
function seconds(env, name, fallback) {
  if (env[name] === undefined || env[name] === "") return fallback;
  const n = Number(env[name]);
  if (!Number.isFinite(n) || n < 0 || n > 86400)
    throw new Error(`${name} must be seconds from 0 to 86400.`);
  return n;
}

/** Read and bound the capture plan from the environment. */
export function battleTickPlan(env = process.env) {
  const tick = integer(env, "AF_BATTLE_TICK", 180, 0, BATTLE_TICK_MAX);
  const views = (env.AF_BATTLE_VIEWS || BATTLE_TICK_VIEWS.join(",")).split(",");
  if (
    !views.length ||
    new Set(views).size !== views.length ||
    !views.every((view) => BATTLE_TICK_VIEWS.includes(view))
  )
    throw new Error(
      `AF_BATTLE_VIEWS must list spectator views: ${BATTLE_TICK_VIEWS.join(", ")}.`,
    );
  const label = env.AF_BATTLE_LABEL ?? "";
  if (!/^[A-Za-z0-9._-]{0,64}$/.test(label))
    throw new Error("AF_BATTLE_LABEL may use letters, digits, '.', '_' and '-'.");
  return {
    count: integer(env, "AF_BATTLE_COUNT", 1_000_000, 100, 1_000_000),
    seed: integer(env, "AF_BATTLE_SEED", 1, 0, 0xffffffff),
    tick,
    views,
    cameraSeconds: seconds(env, "AF_BATTLE_CAMERA_SECONDS", tick / 60),
    time: seconds(env, "AF_BATTLE_TIME", tick / 60),
    viewport: {
      width: integer(env, "AF_BATTLE_WIDTH", 1280, 640, 3840),
      height: integer(env, "AF_BATTLE_HEIGHT", 720, 360, 2160),
    },
    label,
  };
}

/** Canonical JSON: object keys sorted, so the digest ignores key order. */
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object")
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  if (typeof value === "number" && !Number.isFinite(value))
    throw new Error("A battle probe holds a non-finite number.");
  return JSON.stringify(value);
}

/**
 * Camera-independent digest of a `readBattle` probe: sampled fighters,
 * participant slot, projectiles, siege engines and simulation counters. The
 * probe's own `sampleSha256` also covers the camera-dependent visible list,
 * so it is recorded but not compared across views.
 */
export function battleStateDigest(probe) {
  if (!probe || !Number.isSafeInteger(probe.tick) || !probe.gpu)
    throw new Error("Read a complete battle probe first.");
  const gpu = Object.fromEntries(
    Object.entries(probe.gpu).filter(
      ([key]) => !BATTLE_VIEW_COUNTERS.includes(key),
    ),
  );
  const state = {
    count: probe.count,
    tick: probe.tick,
    finite: probe.finite,
    samples: probe.samples,
    participant: probe.participant,
    projectiles: probe.projectiles,
    sieges: probe.sieges,
    gpu,
  };
  return { tick: probe.tick, sha256: sha(canonical(state)) };
}

/** Width and height from a PNG's IHDR chunk. */
export function pngSize(png) {
  const bytes = new Uint8Array(png);
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (
    bytes.length < 24 ||
    signature.some((b, i) => bytes[i] !== b) ||
    String.fromCharCode(...bytes.slice(12, 16)) !== "IHDR"
  )
    throw new Error("The capture is not a PNG.");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return { width: view.getUint32(16), height: view.getUint32(20) };
}

/** Exact pixel comparison of two tightly packed RGBA8 frames. */
export function compareRgba(a, b, width, height) {
  const left = new Uint8Array(a),
    right = new Uint8Array(b),
    pixels = width * height;
  if (left.length !== pixels * 4 || right.length !== pixels * 4)
    throw new Error("Both captures must be the same RGBA size.");
  let differing = 0,
    maxDelta = 0;
  for (let p = 0; p < pixels; p++) {
    let delta = 0;
    for (let c = 0; c < 4; c++)
      delta = Math.max(delta, Math.abs(left[p * 4 + c] - right[p * 4 + c]));
    if (delta) differing++;
    maxDelta = Math.max(maxDelta, delta);
  }
  return { pixels, differing, maxDelta, identical: differing === 0 };
}

/**
 * Compare two capture reports. Views must use the same plan; a view matches
 * only when both its state digest and its RGBA bytes are identical.
 * `loadRgba(report, view)` returns the raw frame for a pixel count.
 */
export async function compareBattleTickReports(a, b, loadRgba) {
  const keys = ["count", "seed", "tick", "cameraSeconds", "time"];
  const plan = Object.fromEntries(keys.map((key) => [key, a.plan?.[key]]));
  const mismatchedPlan = keys.filter((key) => a.plan?.[key] !== b.plan?.[key]);
  const views = [];
  for (const left of a.views ?? []) {
    const right = (b.views ?? []).find((view) => view.view === left.view);
    if (!right) {
      views.push({ view: left.view, ok: false, reason: "missing" });
      continue;
    }
    const sameSize =
      left.width === right.width && left.height === right.height;
    const state = left.stateSha256 === right.stateSha256;
    const image = sameSize && left.rgbaSha256 === right.rgbaSha256;
    const pixels =
      image || !sameSize
        ? null
        : compareRgba(
            await loadRgba(a, left),
            await loadRgba(b, right),
            left.width,
            left.height,
          );
    views.push({
      view: left.view,
      ok: state && image,
      state,
      image,
      sameSize,
      ...(pixels ? { pixels } : {}),
    });
  }
  return {
    ok: mismatchedPlan.length === 0 && views.length > 0 && views.every((v) => v.ok),
    plan,
    mismatchedPlan,
    revisions: [a.revision ?? null, b.revision ?? null],
    views,
  };
}
