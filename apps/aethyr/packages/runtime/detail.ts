/**
 * TypeScript mirror of `crates/aethyr-profile` (ADR 0007): Detail, knobs,
 * curves, floors, the budget, and the controller. Wire objects keep the
 * crate's snake_case JSON. `crates/aethyr-profile/fixtures/vectors.json` is
 * run by both test suites, so the two implementations can't drift; see
 * docs/aethyr/device-profile.md.
 */

export const PROFILE_SCHEMA = "aethyr.profile.v1";
export const REPORT_SCHEMA = "aethyr.detail-report.v1";

export type Knob =
  | "render_scale"
  | "aa"
  | "occlusion"
  | "cluster_lod_bias"
  | "crowd_tier_distance"
  | "shadow_quality"
  | "foliage_density"
  | "backdrop_blur"
  | "ui_shadows"
  | "motion"
  | "ui_effects";
export const KNOBS: readonly Knob[] = [
  "render_scale",
  "aa",
  "occlusion",
  "cluster_lod_bias",
  "crowd_tier_distance",
  "shadow_quality",
  "foliage_density",
  "backdrop_blur",
  "ui_shadows",
  "motion",
  "ui_effects",
];
export type Curve =
  | { kind: "linear" }
  | { kind: "ease_in" }
  | { kind: "ease_out" }
  | { kind: "steps"; count: number };
export interface KnobDecl {
  knob: Knob;
  curve: Curve;
  floor: number;
  full: number;
}
export interface KnobValue {
  knob: Knob;
  value: number;
  limited: boolean;
}

export type Pointer = "none" | "coarse" | "fine";
export type Thermal = "unknown" | "nominal" | "fair" | "serious" | "critical";
export interface UserPrefs {
  reduced_motion: boolean;
  reduced_transparency: boolean;
  more_contrast: boolean;
  text_scale: number;
}
export interface DeviceProfile {
  schema: typeof PROFILE_SCHEMA;
  gpu: {
    api: "none" | "webgpu" | "metal" | "vulkan" | "d3d12";
    vendor: string;
    architecture: string;
    device: string;
    description: string;
    fallback: boolean;
    class: "none" | "software" | "baseline" | "extended";
    features: string[];
    limits: Record<string, number>;
  };
  memory: { device_memory_gb: number | null };
  display: {
    width: number;
    height: number;
    scale: number;
    refresh_hz: number | null;
    hdr: boolean | null;
  };
  input: {
    pointer: Pointer;
    hover: boolean;
    any_fine: boolean;
    any_coarse: boolean;
    touch_points: number;
    gamepad: boolean;
  };
  power: {
    battery_level: number | null;
    charging: boolean | null;
    low_power: boolean | null;
    thermal: Thermal;
  };
  network: {
    effective_type: string | null;
    downlink_mbps: number | null;
    rtt_ms: number | null;
    save_data: boolean | null;
  };
  prefs: UserPrefs;
}
export interface Budget {
  frame_ms: number;
  gpu_ms: number;
  memory_mb: number | null;
  download_mb: number | null;
}

/** Six decimals, as the crate rounds, so repeated steps agree exactly. */
export const round6 = (value: number) => Math.round(value * 1e6) / 1e6;
const clamp = (v: number, lo: number, hi: number) =>
  Math.min(hi, Math.max(lo, v));

export function detail(value: number): number {
  if (!Number.isFinite(value)) throw new Error("Use a finite detail value.");
  return clamp(value, 0, 1);
}

export function defaultPrefs(): UserPrefs {
  return {
    reduced_motion: false,
    reduced_transparency: false,
    more_contrast: false,
    text_scale: 1,
  };
}

export function defaultProfile(): DeviceProfile {
  return {
    schema: PROFILE_SCHEMA,
    gpu: {
      api: "none",
      vendor: "",
      architecture: "",
      device: "",
      description: "",
      fallback: false,
      class: "none",
      features: [],
      limits: {},
    },
    memory: { device_memory_gb: null },
    display: { width: 0, height: 0, scale: 1, refresh_hz: null, hdr: null },
    input: {
      pointer: "none",
      hover: false,
      any_fine: false,
      any_coarse: false,
      touch_points: 0,
      gamepad: false,
    },
    power: {
      battery_level: null,
      charging: null,
      low_power: null,
      thermal: "unknown",
    },
    network: {
      effective_type: null,
      downlink_mbps: null,
      rtt_ms: null,
      save_data: null,
    },
    prefs: defaultPrefs(),
  };
}

export function classifyGpu(
  gpu: Omit<DeviceProfile["gpu"], "class">,
): DeviceProfile["gpu"]["class"] {
  if (gpu.api === "none") return "none";
  if (gpu.fallback) return "software";
  const limit = (name: string) => gpu.limits[name] ?? 0;
  return limit("maxTextureDimension2D") >= 16_384 &&
    limit("maxBufferSize") >= 2 ** 30
    ? "extended"
    : "baseline";
}

export const FULL_MS = 1000 / 60;
export const CONSTRAINED_MS = 1000 / 30;
const SLOW_DOWNLOAD_MB = 50;
const THERMAL_ORDER: Thermal[] = [
  "unknown",
  "nominal",
  "fair",
  "serious",
  "critical",
];

export function powerConstrained(power: DeviceProfile["power"]) {
  const lowBattery =
    power.charging === false &&
    power.battery_level !== null &&
    power.battery_level < 0.2;
  return (
    lowBattery ||
    power.low_power === true ||
    THERMAL_ORDER.indexOf(power.thermal) >= THERMAL_ORDER.indexOf("serious")
  );
}

/** Provisional until ADR 0007 qualification; see `Budget::for_profile`. */
export function budgetForProfile(profile: DeviceProfile): Budget {
  const hz = profile.display.refresh_hz;
  const refresh = clamp(
    hz !== null && Number.isFinite(hz) && hz > 0 ? hz : 60,
    24,
    60,
  );
  const frame = powerConstrained(profile.power)
    ? CONSTRAINED_MS
    : Math.max(1000 / refresh, FULL_MS);
  const gb = profile.memory.device_memory_gb;
  const net = profile.network;
  const slow =
    net.save_data === true ||
    ["slow-2g", "2g", "3g"].includes(net.effective_type ?? "");
  return {
    frame_ms: round6(frame),
    gpu_ms: round6(frame * 0.8),
    memory_mb:
      gb !== null && Number.isFinite(gb) && gb > 0
        ? Math.round((gb * 1024) / 4)
        : null,
    download_mb: slow ? SLOW_DOWNLOAD_MB : null,
  };
}

export function curveAt(curve: Curve, d: number): number {
  switch (curve.kind) {
    case "linear":
      return d;
    case "ease_in":
      return d * d;
    case "ease_out":
      return 1 - (1 - d) * (1 - d);
    case "steps": {
      const n = Math.max(1, curve.count);
      return Math.floor(d * n) / n;
    }
  }
}

const PREFERENCE_GOVERNED: readonly Knob[] = [
  "motion",
  "ui_effects",
  "backdrop_blur",
];
export function limitedBy(knob: Knob, prefs: UserPrefs) {
  if (knob === "motion" || knob === "ui_effects") return prefs.reduced_motion;
  if (knob === "backdrop_blur") return prefs.reduced_transparency;
  return false;
}

/** The checks `KnobSet::new` makes; throws with a plain reason. */
export function validateKnobs(decls: readonly KnobDecl[]): readonly KnobDecl[] {
  const seen = new Set<Knob>();
  for (const d of decls) {
    if (!KNOBS.includes(d.knob)) throw new Error(`Unknown knob ${d.knob}.`);
    if (seen.has(d.knob)) throw new Error(`Knob ${d.knob} is declared twice.`);
    seen.add(d.knob);
    if (!Number.isFinite(d.floor) || !Number.isFinite(d.full))
      throw new Error(`Knob ${d.knob} needs finite floor and full values.`);
    if (d.floor > d.full)
      throw new Error(`Knob ${d.knob} has a floor above its full value.`);
    if (
      !["linear", "ease_in", "ease_out", "steps"].includes(d.curve?.kind) ||
      (d.curve.kind === "steps" &&
        (!Number.isInteger(d.curve.count) || d.curve.count < 1))
    )
      throw new Error(`Knob ${d.knob} needs a supported curve.`);
    if (PREFERENCE_GOVERNED.includes(d.knob) && d.floor !== 0)
      throw new Error(
        `Knob ${d.knob} follows a preference, so its floor must be 0.`,
      );
  }
  return decls;
}

export function knobValue(
  decl: KnobDecl,
  d: number,
  prefs: UserPrefs,
): KnobValue {
  const limited = limitedBy(decl.knob, prefs);
  const value = limited
    ? decl.floor
    : round6(
        clamp(
          decl.floor + (decl.full - decl.floor) * curveAt(decl.curve, d),
          decl.floor,
          decl.full,
        ),
      );
  return { knob: decl.knob, value, limited };
}

export const knobValues = (
  decls: readonly KnobDecl[],
  d: number,
  prefs: UserPrefs,
) => decls.map((decl) => knobValue(decl, d, prefs));

export const CSS_VARIABLES: Partial<Record<Knob, string>> = {
  backdrop_blur: "--detail-blur",
  ui_shadows: "--detail-shadow",
  motion: "--detail-motion",
  ui_effects: "--detail-effects",
};

function fixed3(value: number) {
  const thousandths = Math.round(value * 1000);
  const abs = Math.abs(thousandths);
  return `${thousandths < 0 ? "-" : ""}${Math.floor(abs / 1000)}.${String(abs % 1000).padStart(3, "0")}`;
}

/** `--detail` and the interface knobs, formatted exactly as the crate does. */
export function cssVariables(
  d: number,
  values: readonly KnobValue[],
): [string, string][] {
  const out: [string, string][] = [["--detail", fixed3(d)]];
  for (const v of values) {
    const name = CSS_VARIABLES[v.knob];
    if (name) out.push([name, fixed3(v.value)]);
  }
  return out;
}

export interface ControllerConfig {
  budget_ms: number;
  gpu_budget_ms: number;
  floor: number;
  ceiling: number;
  start: number;
  window: number;
  warmup: number;
  gap_ms: number;
  lower_ratio: number;
  raise_ratio_gpu: number;
  raise_ratio_pacing: number;
  step_down: number;
  step_up: number;
  raise_after: number;
  max_raise_after: number;
}
export function defaultControllerConfig(): ControllerConfig {
  return {
    budget_ms: FULL_MS,
    gpu_budget_ms: FULL_MS * 0.8,
    floor: 0,
    ceiling: 1,
    start: 1,
    window: 30,
    warmup: 60,
    gap_ms: 250,
    lower_ratio: 1.1,
    raise_ratio_gpu: 0.75,
    raise_ratio_pacing: 1.05,
    step_down: 0.1,
    step_up: 0.05,
    raise_after: 4,
    max_raise_after: 64,
  };
}
function sanitize(input: Partial<ControllerConfig>): ControllerConfig {
  const base = defaultControllerConfig();
  const c = { ...base, ...input };
  const positive = (v: number, d: number) =>
    Number.isFinite(v) && v > 0 ? v : d;
  const unit = (v: number, d: number) =>
    Number.isFinite(v) ? clamp(v, 0, 1) : d;
  const count = (v: number, d: number) =>
    Number.isInteger(v) && v >= 0 && v <= 0xffffffff ? v : d;
  for (const key of [
    "budget_ms",
    "gpu_budget_ms",
    "gap_ms",
    "lower_ratio",
    "raise_ratio_gpu",
    "raise_ratio_pacing",
    "step_down",
    "step_up",
  ] as const)
    c[key] = positive(c[key], base[key]);
  c.floor = unit(c.floor, 0);
  c.ceiling = Math.max(unit(c.ceiling, 1), c.floor);
  c.start = clamp(unit(c.start, 1), c.floor, c.ceiling);
  c.window = Math.max(1, count(c.window, base.window));
  c.warmup = count(c.warmup, base.warmup);
  c.raise_after = Math.max(1, count(c.raise_after, base.raise_after));
  c.max_raise_after = Math.max(
    c.raise_after,
    count(c.max_raise_after, base.max_raise_after),
  );
  return c;
}

export interface Sample {
  interval_ms: number;
  gpu_ms?: number | null;
}
export type Source = "gpu" | "pacing";
export type Pressure = "holding" | "lowering" | "at_floor";
export interface ControllerState {
  detail: number;
  floor: number;
  ceiling: number;
  source: Source | null;
  window_ms: number | null;
  budget_ms: number;
  pressure: Pressure;
  changes: number;
  windows: number;
  raise_after: number;
  held: boolean;
}

/** Mirrors `aethyr_profile::Controller`; the crate documents the rules. */
export class DetailController {
  readonly config: ControllerConfig;
  private value: number;
  private held: number | null = null;
  private samples: number[] = [];
  private source: Source | null = null;
  private warm = 0;
  private good = 0;
  private raiseAfter: number;
  private lastRaise: number | null = null;
  private quiet = 0;
  private windows = 0;
  private changes = 0;
  private windowMs: number | null = null;
  private lastSource: Source | null = null;
  private pressure: Pressure = "holding";

  constructor(config: Partial<ControllerConfig> = {}) {
    this.config = Object.freeze(sanitize(config));
    this.value = this.config.start;
    this.raiseAfter = this.config.raise_after;
  }
  detail() {
    return this.held ?? this.value;
  }
  hold(value: number) {
    this.held = detail(value);
    this.samples = [];
    this.source = null;
  }
  release() {
    this.held = null;
    this.reset();
  }
  reset() {
    this.samples = [];
    this.source = null;
    this.warm = 0;
    this.good = 0;
  }
  /** Returns the new Detail when it changed. */
  observe(sample: Sample): number | undefined {
    if (this.held !== null) return;
    const interval = sample.interval_ms;
    if (!Number.isFinite(interval) || interval < 0) return;
    if (interval > this.config.gap_ms) {
      this.reset();
      return;
    }
    if (this.warm < this.config.warmup) {
      this.warm++;
      return;
    }
    const gpu = sample.gpu_ms;
    const [source, ms]: [Source, number] =
      typeof gpu === "number" && Number.isFinite(gpu) && gpu >= 0
        ? ["gpu", gpu]
        : ["pacing", interval];
    if (this.source !== source) {
      this.samples = [];
      this.source = source;
    }
    this.samples.push(ms);
    if (this.samples.length < this.config.window) return;
    return this.decide(source);
  }
  private decide(source: Source): number | undefined {
    const sorted = this.samples.sort((a, b) => a - b);
    const stat = sorted[Math.ceil((9 * sorted.length) / 10) - 1];
    this.samples = [];
    this.windows++;
    this.quiet++;
    this.windowMs = stat;
    this.lastSource = source;
    const c = this.config;
    const [budget, raiseRatio] =
      source === "gpu"
        ? [c.gpu_budget_ms, c.raise_ratio_gpu]
        : [c.budget_ms, c.raise_ratio_pacing];
    const ratio = stat / budget;
    const before = this.value;
    if (ratio > c.lower_ratio) {
      this.good = 0;
      this.quiet = 0;
      if (this.lastRaise !== null && this.windows - this.lastRaise <= 2)
        this.raiseAfter = Math.min(this.raiseAfter * 2, c.max_raise_after);
      this.lastRaise = null;
      const step = ratio > 1.5 ? c.step_down * 2 : c.step_down;
      this.value = round6(Math.max(this.value - step, c.floor));
      this.pressure = before <= c.floor ? "at_floor" : "lowering";
    } else {
      this.pressure = "holding";
      if (ratio <= raiseRatio) {
        this.good++;
        if (this.good >= this.raiseAfter && this.value < c.ceiling) {
          this.value = round6(Math.min(this.value + c.step_up, c.ceiling));
          this.good = 0;
          this.lastRaise = this.windows;
        }
      } else this.good = 0;
      if (this.quiet >= 2 * c.max_raise_after) {
        this.raiseAfter = Math.max(
          Math.floor(this.raiseAfter / 2),
          c.raise_after,
        );
        this.quiet = 0;
      }
    }
    if (this.value !== before) {
      this.changes++;
      return this.detail();
    }
  }
  state(): ControllerState {
    return {
      detail: this.detail(),
      floor: this.config.floor,
      ceiling: this.config.ceiling,
      source: this.lastSource,
      window_ms: this.windowMs === null ? null : round6(this.windowMs),
      budget_ms:
        this.lastSource === "gpu"
          ? this.config.gpu_budget_ms
          : this.config.budget_ms,
      pressure: this.pressure,
      changes: this.changes,
      windows: this.windows,
      raise_after: this.raiseAfter,
      held: this.held !== null,
    };
  }
}

export interface DetailReport {
  schema: typeof REPORT_SCHEMA;
  profile: DeviceProfile;
  budget: Budget;
  controller: ControllerState;
  knobs: KnobValue[];
}
