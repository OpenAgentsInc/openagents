import {
  classifyGpu,
  defaultProfile,
  PROFILE_SCHEMA,
  type DeviceProfile,
  type Pointer,
} from "./detail.ts";

/**
 * What the probe reads. The browser's globals satisfy it; tests pass a fake.
 * It has no user-agent field on purpose: ADR 0007 forbids guessing a device
 * from the user agent, and a test checks the probe never reads one.
 */
export interface ProbeEnvironment {
  navigator: {
    gpu?: {
      requestAdapter(): Promise<ProbeAdapter | null>;
    };
    deviceMemory?: number;
    maxTouchPoints?: number;
    getGamepads?: unknown;
    connection?: {
      effectiveType?: string;
      downlink?: number;
      rtt?: number;
      saveData?: boolean;
    };
    getBattery?(): Promise<{ level: number; charging: boolean }>;
  };
  matchMedia?(query: string): { matches: boolean };
  devicePixelRatio?: number;
  innerWidth?: number;
  innerHeight?: number;
  requestAnimationFrame?(callback: (now: number) => void): unknown;
  /** Root font size in pixels, for the text scale. */
  rootFontSize?(): number | undefined;
}
export interface ProbeAdapter {
  info?: {
    vendor?: string;
    architecture?: string;
    device?: string;
    description?: string;
    isFallbackAdapter?: boolean;
  };
  isFallbackAdapter?: boolean;
  features: Iterable<string>;
  limits: object;
}
export interface ProbeOptions {
  /** Frames to sample for the refresh rate; 0 skips the measurement. */
  refreshFrames?: number;
  /** Upper bound on each asynchronous platform call. */
  timeoutMs?: number;
}

const within = <T>(promise: Promise<T>, ms: number): Promise<T | undefined> =>
  Promise.race([
    promise.catch(() => undefined),
    new Promise<undefined>((done) => setTimeout(done, ms)),
  ]);

const finite = (v: unknown): number | null =>
  typeof v === "number" && Number.isFinite(v) ? v : null;

/** WebGPU limits are getters on the prototype, so walk it by name. */
function adapterLimits(limits: object): Record<string, number> {
  const out: Record<string, number> = {};
  const names = new Set<string>();
  for (
    let o: object | null = limits;
    o && o !== Object.prototype;
    o = Object.getPrototypeOf(o)
  )
    for (const name of Object.getOwnPropertyNames(o)) names.add(name);
  for (const name of [...names].sort()) {
    if (name === "constructor") continue;
    const value = (limits as Record<string, unknown>)[name];
    if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0)
      out[name] = value;
  }
  return out;
}

async function gpu(env: ProbeEnvironment, timeoutMs: number) {
  const base = defaultProfile().gpu;
  const adapter = env.navigator.gpu
    ? await within(env.navigator.gpu.requestAdapter(), timeoutMs)
    : undefined;
  if (!adapter) return base;
  const info = adapter.info ?? {};
  const probed = {
    ...base,
    api: "webgpu" as const,
    vendor: info.vendor ?? "",
    architecture: info.architecture ?? "",
    device: info.device ?? "",
    description: info.description ?? "",
    fallback: !!(info.isFallbackAdapter ?? adapter.isFallbackAdapter),
    features: [...adapter.features].sort(),
    limits: adapterLimits(adapter.limits),
  };
  return { ...probed, class: classifyGpu(probed) };
}

/** Median frame interval over a few animation frames, as a rate. */
async function refreshRate(
  env: ProbeEnvironment,
  frames: number,
  timeoutMs: number,
): Promise<number | null> {
  const raf = env.requestAnimationFrame;
  if (!raf || frames < 2) return null;
  const stamps: number[] = [];
  const measured = await within(
    new Promise<number[]>((done) => {
      const step = (now: number) => {
        stamps.push(now);
        if (stamps.length > frames) done(stamps);
        else raf(step);
      };
      raf(step);
    }),
    timeoutMs,
  );
  if (!measured) return null;
  const gaps = measured
    .slice(1)
    .map((t, i) => t - measured[i])
    .filter((g) => g > 0)
    .sort((a, b) => a - b);
  if (!gaps.length) return null;
  const hz = 1000 / gaps[Math.floor(gaps.length / 2)];
  // Report the nearest common rate; odd medians come from busy frames.
  const common = [30, 48, 50, 60, 72, 75, 90, 100, 120, 144, 165, 240];
  return common.reduce((a, b) => (Math.abs(b - hz) < Math.abs(a - hz) ? b : a));
}

const query = (env: ProbeEnvironment, q: string) => {
  try {
    return env.matchMedia?.(q).matches ?? false;
  } catch {
    return false;
  }
};

/** The parts that change with platform events and cost nothing to read. */
export function probeLive(
  env: ProbeEnvironment,
  previous: DeviceProfile = defaultProfile(),
): DeviceProfile {
  const pointer: Pointer = query(env, "(pointer: fine)")
    ? "fine"
    : query(env, "(pointer: coarse)")
      ? "coarse"
      : "none";
  const connection = env.navigator.connection;
  const rootSize = env.rootFontSize?.();
  return {
    ...previous,
    schema: PROFILE_SCHEMA,
    memory: { device_memory_gb: finite(env.navigator.deviceMemory) },
    display: {
      ...previous.display,
      width: Math.max(0, Math.floor(env.innerWidth ?? 0)),
      height: Math.max(0, Math.floor(env.innerHeight ?? 0)),
      scale: finite(env.devicePixelRatio) ?? 1,
      hdr: env.matchMedia ? query(env, "(dynamic-range: high)") : null,
    },
    input: {
      pointer,
      hover: query(env, "(hover: hover)"),
      any_fine: query(env, "(any-pointer: fine)"),
      any_coarse: query(env, "(any-pointer: coarse)"),
      touch_points: Math.max(0, Math.floor(env.navigator.maxTouchPoints ?? 0)),
      gamepad: typeof env.navigator.getGamepads === "function",
    },
    network: {
      effective_type: connection?.effectiveType ?? null,
      downlink_mbps: finite(connection?.downlink),
      rtt_ms: finite(connection?.rtt),
      save_data:
        typeof connection?.saveData === "boolean" ? connection.saveData : null,
    },
    prefs: {
      reduced_motion: query(env, "(prefers-reduced-motion: reduce)"),
      reduced_transparency: query(
        env,
        "(prefers-reduced-transparency: reduce)",
      ),
      more_contrast: query(env, "(prefers-contrast: more)"),
      text_scale:
        rootSize && Number.isFinite(rootSize) && rootSize > 0
          ? Math.round((rootSize / 16) * 1000) / 1000
          : 1,
    },
  };
}

/** Probe the device once. Every source is optional and bounded in time. */
export async function probeDeviceProfile(
  env: ProbeEnvironment = browserEnvironment(),
  options: ProbeOptions = {},
): Promise<DeviceProfile> {
  const timeoutMs = options.timeoutMs ?? 1500;
  const [gpuInfo, refresh, battery] = await Promise.all([
    gpu(env, timeoutMs),
    refreshRate(env, options.refreshFrames ?? 20, timeoutMs),
    env.navigator.getBattery
      ? within(env.navigator.getBattery(), timeoutMs)
      : Promise.resolve(undefined),
  ]);
  const profile = probeLive(env);
  return {
    ...profile,
    gpu: gpuInfo,
    display: { ...profile.display, refresh_hz: refresh },
    power: {
      battery_level: finite(battery?.level),
      charging:
        typeof battery?.charging === "boolean" ? battery.charging : null,
      // The web has no low-power or thermal signal.
      low_power: null,
      thermal: "unknown",
    },
  };
}

/** The browser's globals as a probe environment. */
export function browserEnvironment(): ProbeEnvironment {
  const g = globalThis as unknown as ProbeEnvironment & {
    navigator: ProbeEnvironment["navigator"];
    document?: { documentElement?: Element };
    getComputedStyle?: (e: Element) => { fontSize: string };
  };
  return {
    navigator: g.navigator,
    matchMedia: g.matchMedia?.bind(g),
    devicePixelRatio: g.devicePixelRatio,
    innerWidth: g.innerWidth,
    innerHeight: g.innerHeight,
    requestAnimationFrame: g.requestAnimationFrame?.bind(g),
    rootFontSize: () => {
      const root = g.document?.documentElement;
      return root && g.getComputedStyle
        ? parseFloat(g.getComputedStyle(root).fontSize)
        : undefined;
    },
  };
}

/** Media queries whose change should refresh the live part of the profile. */
export const LIVE_QUERIES = [
  "(prefers-reduced-motion: reduce)",
  "(prefers-reduced-transparency: reduce)",
  "(prefers-contrast: more)",
  "(pointer: fine)",
  "(pointer: coarse)",
  "(hover: hover)",
] as const;
