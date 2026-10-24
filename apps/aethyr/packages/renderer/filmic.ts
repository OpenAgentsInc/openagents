import type { SceneLook } from "../assets/src/scene.ts";

/**
 * Owned SDR profile inspired by the public controls and logistic curve explorer:
 * https://dev.epicgames.com/documentation/en-us/unreal-engine/color-grading-and-the-filmic-tonemapper-in-unreal-engine
 * https://www.desmos.com/calculator/h8rbdpawxj
 *
 * This is not an ACES transform or an exact Unreal implementation. The public
 * graph has overlapping toe/shoulder intervals at its defaults. We merge crossed
 * knee levels, retain logistic tails with a linear log10 segment, and shift the
 * joins analytically to anchor scene-linear 0.18 at display-linear 0.18. The
 * joins match value and first derivative. Zero slope uses an owned 1e-4 floor.
 * Fixed highlight whitening and the shadow-grade transition are owned choices;
 * no working-space transform, blue correction, HDR output, or camera grade is
 * implied. Exposure belongs before this function and sRGB encoding after it.
 */
export const FILMIC_PROFILE = "anchored-log-filmic-v1" as const;
export const FILMIC_FLOAT_COUNT = 20;
/** Float offsets; each field is one aligned vec4 in a uniform struct. */
export const FILMIC_OFFSETS = {
  knee: 0,
  toe: 4,
  shoulder: 8,
  grade: 12,
  color: 16,
} as const;
export const FILMIC_GRAY = 0.18;
export const FILMIC_LUMINANCE = [0.212639, 0.715169, 0.072192] as const;
const MIN_POSITIVE = 1e-30;
const MIN_SLOPE = 1e-4;
const MIN_KNEE = 1e-4;

export interface FilmicControls {
  slope: number;
  toe: number;
  shoulder: number;
  blackClip: number;
  whiteClip: number;
  /** Owned pre-curve shadow contrast exponents in [0,4], with a 1e-4 floor. */
  shadowContrast: readonly [number, number, number];
}
export const DEFAULT_FILMIC_CONTROLS: Readonly<FilmicControls> = Object.freeze({
  slope: 0.88,
  toe: 0.55,
  shoulder: 0.26,
  blackClip: 0,
  whiteClip: 0.04,
  shadowContrast: Object.freeze([1, 1, 1] as const),
});
export interface PreparedFilmic {
  profile: typeof FILMIC_PROFILE;
  controls: Readonly<FilmicControls>;
  /** Five vec4s. Treat as immutable; CPU evaluation reads the same f32 values. */
  packed: Float32Array;
}

const clamp = (v: number, lo: number, hi: number) =>
  Math.max(lo, Math.min(hi, v));
const sigmoid = (v: number) => 1 / (1 + Math.exp(-clamp(v, -80, 80)));
const scalarKeys = [
  "slope",
  "toe",
  "shoulder",
  "blackClip",
  "whiteClip",
] as const;

export function prepareFilmic(
  input: FilmicControls = DEFAULT_FILMIC_CONTROLS,
): PreparedFilmic {
  for (const key of scalarKeys)
    if (!Number.isFinite(input[key]) || input[key] < 0 || input[key] > 1)
      throw new Error(`Film ${key} must be a finite number from zero to one.`);
  if (
    input.shadowContrast.length !== 3 ||
    !input.shadowContrast.every((v) => Number.isFinite(v) && v >= 0 && v <= 4)
  )
    throw new Error(
      "Shadow contrast needs three finite values from zero to four.",
    );
  const controls = Object.freeze({
    ...input,
    shadowContrast: Object.freeze([...input.shadowContrast]) as readonly [
      number,
      number,
      number,
    ],
  });
  const slope = Math.max(MIN_SLOPE, controls.slope);
  let toeY = clamp(1 - controls.toe, MIN_KNEE, 1 - MIN_KNEE);
  let shoulderY = clamp(controls.shoulder, MIN_KNEE, 1 - MIN_KNEE);
  // The public defaults cross. Merging produces one unambiguous smooth join.
  if (toeY > shoulderY) toeY = shoulderY = (toeY + shoulderY) * 0.5;
  const toeAmplitude = toeY + controls.blackClip;
  const upper = 1 + controls.whiteClip;
  const shoulderAmplitude = upper - shoulderY;
  let relativeGray: number;
  if (FILMIC_GRAY < toeY)
    relativeGray =
      (toeAmplitude / (2 * slope)) *
      Math.log(
        (FILMIC_GRAY + controls.blackClip) /
          (2 * toeAmplitude - FILMIC_GRAY - controls.blackClip),
      );
  else if (FILMIC_GRAY <= shoulderY)
    relativeGray = (FILMIC_GRAY - toeY) / slope;
  else
    relativeGray =
      (shoulderY - toeY) / slope +
      (shoulderAmplitude / (2 * slope)) *
        Math.log((2 * shoulderAmplitude) / (upper - FILMIC_GRAY) - 1);
  const toeX = Math.log10(FILMIC_GRAY) - relativeGray;
  const shoulderX = toeX + (shoulderY - toeY) / slope;
  const packed = new Float32Array([
    toeX,
    shoulderX,
    toeY,
    slope,
    toeAmplitude,
    (2 * slope) / toeAmplitude,
    controls.blackClip,
    shoulderY,
    shoulderAmplitude,
    (2 * slope) / shoulderAmplitude,
    upper,
    0,
    ...controls.shadowContrast.map((v) => Math.max(MIN_SLOPE, v)),
    FILMIC_GRAY,
    0.6,
    1,
    MIN_POSITIVE,
    1,
  ]);
  return { profile: FILMIC_PROFILE, controls, packed };
}

/** The scalar curve alone, without the separate pre-curve shadow grade. */
export function evaluateFilmic(
  value: number,
  prepared: PreparedFilmic,
): number {
  if (!Number.isFinite(value)) throw new Error("Film input must be finite.");
  if (value <= 0) return 0;
  const p = prepared.packed;
  const x = Math.log10(Math.max(value, p[18]));
  let mapped: number;
  if (x < p[0]) mapped = 2 * p[4] * sigmoid(p[5] * (x - p[0])) - p[6];
  else if (x > p[1]) mapped = p[10] - 2 * p[8] * sigmoid(-p[9] * (x - p[1]));
  else mapped = p[2] + p[3] * (x - p[0]);
  return clamp(mapped, 0, 1);
}

/**
 * Owned shadow-only grade. In log space its derivative stays positive for every
 * supported exponent and becomes one at gray. This avoids a nonmonotone blend
 * between a contrast power and identity. The source engine's shadow window and
 * grading formula are not recovered; this is an explicitly bounded approximation.
 */
export function gradeFilmicShadow(value: number, contrast: number): number {
  if (
    !Number.isFinite(value) ||
    !Number.isFinite(contrast) ||
    contrast < 0 ||
    contrast > 4
  )
    throw new Error(
      "Shadow grading needs finite color and contrast from zero to four.",
    );
  if (value <= 0) return 0;
  if (value >= FILMIC_GRAY) return value;
  const exponent = Math.max(MIN_SLOPE, contrast);
  const t = Math.log(Math.max(value, MIN_POSITIVE) / FILMIC_GRAY);
  return (
    FILMIC_GRAY * Math.exp(exponent * t + (exponent - 1) * (1 - Math.exp(t)))
  );
}

export function evaluateFilmicColor(
  rgb: readonly number[],
  prepared: PreparedFilmic,
): [number, number, number] {
  if (rgb.length !== 3 || !rgb.every(Number.isFinite))
    throw new Error("Film input needs three finite color values.");
  const graded = rgb.map((v, i) =>
    gradeFilmicShadow(v, prepared.packed[12 + i]),
  );
  const mapped = graded.map((v) => evaluateFilmic(v, prepared));
  const neutral = evaluateFilmic(
    graded.reduce((sum, v, i) => sum + v * FILMIC_LUMINANCE[i], 0),
    prepared,
  );
  const t = clamp(
    (neutral - prepared.packed[16]) /
      (prepared.packed[17] - prepared.packed[16]),
    0,
    1,
  );
  const whitening = t * t * (3 - 2 * t);
  return mapped.map((v) => v + (neutral - v) * whitening) as [
    number,
    number,
    number,
  ];
}

export interface SourceFilmicControls {
  controls: FilmicControls;
  appliedOverrides: string[];
  diagnostics: string[];
}
const sourceKeys = {
  FilmSlope: "slope",
  FilmToe: "toe",
  FilmShoulder: "shoulder",
  FilmBlackClip: "blackClip",
  FilmWhiteClip: "whiteClip",
} as const;
function setting(text: string, name: string): string | undefined {
  return new RegExp(`(?:^|[,(])\\s*${name}\\s*=\\s*([^,()]+)`)
    .exec(text)?.[1]
    .trim();
}

/**
 * Admit declared overrides from enabled, unbound volumes only. Priority and
 * blend weight explicitly blend this profile's known parameters; spatial volumes
 * and camera settings are outside this source-admission contract.
 */
export function sourceFilmicControls(
  look: SceneLook | undefined,
  mode: "day" | "night",
): SourceFilmicControls {
  const result: SourceFilmicControls = {
    controls: {
      ...DEFAULT_FILMIC_CONTROLS,
      shadowContrast: [...DEFAULT_FILMIC_CONTROLS.shadowContrast],
    },
    appliedOverrides: [],
    diagnostics: [],
  };
  const volumes = (look?.scenarios[mode].environment ?? [])
    .map((actor, order) => ({ actor, order }))
    .filter(({ actor }) => {
      const p = actor.properties as Record<string, unknown> | undefined;
      return (
        actor.class === "PostProcessVolume" &&
        p?.enabled === true &&
        p.unbound === true
      );
    })
    .sort((a, b) => {
      const priority = (v: typeof a) => {
        const p = v.actor.properties as Record<string, unknown>;
        return typeof p.priority === "number" && Number.isFinite(p.priority)
          ? p.priority
          : 0;
      };
      return priority(a) - priority(b) || a.order - b.order;
    });
  for (const { actor, order } of volumes) {
    const p = actor.properties as Record<string, unknown>;
    const name = typeof actor.id === "string" ? actor.id : `volume ${order}`;
    if (typeof p.settings !== "string") {
      result.diagnostics.push(`${name}: film settings are unavailable.`);
      continue;
    }
    const weight = p.blend_weight;
    if (
      typeof weight !== "number" ||
      !Number.isFinite(weight) ||
      weight < 0 ||
      weight > 1
    ) {
      result.diagnostics.push(
        `${name}: unsupported blend weight; film overrides were skipped.`,
      );
      continue;
    }
    if (
      p.priority !== undefined &&
      (typeof p.priority !== "number" || !Number.isFinite(p.priority))
    ) {
      result.diagnostics.push(
        `${name}: unsupported priority; film overrides were skipped.`,
      );
      continue;
    }
    if (weight === 0) continue;
    for (const [field, key] of Object.entries(sourceKeys)) {
      if (setting(p.settings, `bOverride_${field}`) !== "True") continue;
      const raw = setting(p.settings, field);
      const value = raw === undefined || raw === "" ? NaN : Number(raw);
      if (!Number.isFinite(value) || value < 0 || value > 1) {
        result.diagnostics.push(
          `${name}: invalid ${field}; inherited value was kept.`,
        );
        continue;
      }
      result.controls[key as (typeof sourceKeys)[keyof typeof sourceKeys]] +=
        (value -
          result.controls[
            key as (typeof sourceKeys)[keyof typeof sourceKeys]
          ]) *
        weight;
      result.appliedOverrides.push(`${name}:${field}`);
    }
    if (setting(p.settings, "bOverride_ColorContrastShadows") !== "True")
      continue;
    const tuple = /(?:^|[,(])\s*ColorContrastShadows\s*=\s*\(([^()]*)\)/.exec(
      p.settings,
    )?.[1];
    const values =
      tuple === undefined
        ? []
        : ["X", "Y", "Z", "W"].map((key) => {
            const raw = setting(tuple, key);
            return raw === undefined || raw === "" ? NaN : Number(raw);
          });
    const effective = values.slice(0, 3).map((v) => v * values[3]);
    if (
      values.length !== 4 ||
      !values.every((v) => Number.isFinite(v) && v >= 0 && v <= 4) ||
      !effective.every((v) => v <= 4)
    ) {
      result.diagnostics.push(
        `${name}: invalid shadow contrast; inherited value was kept.`,
      );
      continue;
    }
    // Applying source W multiplicatively is owned; the recovered source uses W=1.
    result.controls.shadowContrast = result.controls.shadowContrast.map(
      (v, i) => v + (effective[i] - v) * weight,
    ) as [number, number, number];
    result.appliedOverrides.push(`${name}:ColorContrastShadows`);
  }
  return result;
}

/** No bindings: embed FilmicParams in the caller's aligned post-process uniform. */
// FilmicParams lanes: knee = toeX/shoulderX/toeY/slope; toe = amplitude/rate/blackClip/shoulderY.
// Shoulder = amplitude/rate/upperAsymptote/unused; grade = RGB exponents/gray.
// Color = whitening start/end/minimum positive input/profile version.
export const WGSL_FILMIC = `struct FilmicParams{knee:vec4<f32>,toe:vec4<f32>,shoulder:vec4<f32>,grade:vec4<f32>,color:vec4<f32>,};fn filmicSigmoid(x:f32)->f32{return 1.0/(1.0+exp(-clamp(x,-80.0,80.0)));}fn filmicCurve(value:f32,p:FilmicParams)->f32{if value<=0.0{return 0.0;}let x=log2(max(value,p.color.z))*0.3010299956639812;var y=p.knee.z+p.knee.w*(x-p.knee.x);if x<p.knee.x{y=2.0*p.toe.x*filmicSigmoid(p.toe.y*(x-p.knee.x))-p.toe.z;}else if x>p.knee.y{y=p.shoulder.z-2.0*p.shoulder.x*filmicSigmoid(-p.shoulder.y*(x-p.knee.y));}return clamp(y,0.0,1.0);}fn filmicShadow(value:f32,contrast:f32,p:FilmicParams)->f32{if value<=0.0{return 0.0;}if value>=p.grade.w{return value;}let t=log(max(value,p.color.z)/p.grade.w);return p.grade.w*exp(contrast*t+(contrast-1.0)*(1.0-exp(t)));}fn filmicColor(value:vec3<f32>,p:FilmicParams)->vec3<f32>{let c=vec3(filmicShadow(value.r,p.grade.r,p),filmicShadow(value.g,p.grade.g,p),filmicShadow(value.b,p.grade.b,p));let mapped=vec3(filmicCurve(c.r,p),filmicCurve(c.g,p),filmicCurve(c.b,p));let neutral=filmicCurve(dot(c,vec3(0.212639,0.715169,0.072192)),p);return mix(mapped,vec3(neutral),smoothstep(p.color.x,p.color.y,neutral));}`;
