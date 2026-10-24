import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  budgetForProfile,
  cssVariables,
  defaultPrefs,
  defaultProfile,
  DetailController,
  knobValues,
  validateKnobs,
  type DeviceProfile,
  type KnobDecl,
  type Sample,
} from "../packages/runtime/detail.ts";
import { DetailDriver, WEB_KNOBS } from "../packages/runtime/detail-driver.ts";
import {
  probeDeviceProfile,
  type ProbeEnvironment,
} from "../packages/runtime/device-profile.ts";
import {
  FULL_RENDERER_DETAIL,
  rendererDetailFromKnobs,
} from "../packages/renderer/detail-settings.ts";
import { GrassPatchPlanner } from "../packages/renderer/grass.ts";

const crate = new URL(
  "../../../crates/aethyr-profile/fixtures/",
  import.meta.url,
);
const read = (name: string) =>
  JSON.parse(readFileSync(new URL(name, crate), "utf8"));
const vectors = read("vectors.json");

function close(a: unknown, b: unknown, path = "$"): void {
  if (typeof a === "number" && typeof b === "number") {
    assert.ok(Math.abs(a - b) < 1e-9, `${path}: ${a} != ${b}`);
  } else if (Array.isArray(a) && Array.isArray(b)) {
    assert.equal(a.length, b.length, `${path}: length`);
    a.forEach((v, i) => close(v, b[i], `${path}[${i}]`));
  } else if (a && b && typeof a === "object" && typeof b === "object") {
    assert.deepEqual(
      Object.keys(a).sort(),
      Object.keys(b).sort(),
      `${path}: keys`,
    );
    for (const k of Object.keys(a))
      close((a as any)[k], (b as any)[k], `${path}.${k}`);
  } else assert.equal(a, b, path);
}

function runController(c: any) {
  const ctl = new DetailController(c.config);
  let frame = 0;
  const trace: [number, number][] = [];
  const after: unknown[] = [];
  for (const seg of c.segments) {
    if (typeof seg.hold === "number") ctl.hold(seg.hold);
    else if (seg.release === true) ctl.release();
    else
      for (let i = 0; i < seg.frames; i++) {
        let sample: Sample;
        if (seg.plant) {
          const cost =
            seg.plant.base_ms + seg.plant.per_detail_ms * ctl.detail();
          const budget = ctl.config.budget_ms;
          sample = {
            interval_ms: cost <= budget ? budget : cost,
            gpu_ms: seg.plant.gpu ? cost : null,
          };
        } else
          sample = { interval_ms: seg.interval_ms, gpu_ms: seg.gpu_ms ?? null };
        const changed = ctl.observe(sample);
        if (changed !== undefined) trace.push([frame, changed]);
        frame++;
      }
    const s = ctl.state();
    after.push({
      detail: s.detail,
      changes: s.changes,
      windows: s.windows,
      raise_after: s.raise_after,
      pressure: s.pressure,
      held: s.held,
    });
  }
  return { after, trace };
}

test("the TypeScript controller matches the crate's shared vectors", () => {
  vectors.controllers.forEach((c: any, i: number) =>
    close(runController(c), vectors.expected.controllers[i], c.name),
  );
});

test("knob values and CSS variables match the crate's shared vectors", () => {
  const decls = read("web-knobs.json") as KnobDecl[];
  vectors.knobs.forEach((c: any, i: number) => {
    const prefs = { ...defaultPrefs(), ...c.prefs };
    const values = knobValues(decls, c.detail, prefs);
    close(
      { values, css: cssVariables(c.detail, values) },
      vectors.expected.knobs[i],
      c.name,
    );
  });
});

test("budgets match the crate's shared vectors", () => {
  vectors.budgets.forEach((c: any, i: number) => {
    const p = defaultProfile();
    p.display.refresh_hz = c.refresh_hz ?? null;
    p.power.battery_level = c.battery_level ?? null;
    p.power.charging = c.charging ?? null;
    p.memory.device_memory_gb = c.device_memory_gb ?? null;
    p.network.save_data = c.save_data ?? null;
    p.network.effective_type = c.effective_type ?? null;
    close(budgetForProfile(p), vectors.expected.budgets[i], c.name);
  });
});

test("the bundled knob table is the crate's", () => {
  assert.deepEqual(WEB_KNOBS, read("web-knobs.json"));
});

test("knob validation refuses what the crate refuses", () => {
  const lin = { kind: "linear" } as const;
  for (const bad of [
    [
      { knob: "aa", curve: lin, floor: 0, full: 1 },
      { knob: "aa", curve: lin, floor: 0, full: 1 },
    ],
    [{ knob: "aa", curve: lin, floor: 1, full: 0.5 }],
    [{ knob: "aa", curve: lin, floor: NaN, full: 1 }],
    [{ knob: "aa", curve: { kind: "steps", count: 0 }, floor: 0, full: 1 }],
    [{ knob: "motion", curve: lin, floor: 0.2, full: 1 }],
    [{ knob: "sparkle", curve: lin, floor: 0, full: 1 }],
  ])
    assert.throws(() => validateKnobs(bad as KnobDecl[]));
});

test("full detail leaves the renderer exactly as it was", () => {
  const values = knobValues(WEB_KNOBS, 1, defaultPrefs());
  assert.deepEqual(rendererDetailFromKnobs(values), FULL_RENDERER_DETAIL);
  const floor = rendererDetailFromKnobs(
    knobValues(WEB_KNOBS, 0, defaultPrefs()),
  );
  assert.equal(floor.renderScale, 0.5);
  assert.equal(floor.shadowRefreshMeters, 8);
  assert.equal(floor.foliageReach, 0.4);
  assert.equal(floor.crowdReach, 0.4);
  assert.equal(floor.crowdSilhouettePixels, 16);
  const half = rendererDetailFromKnobs([
    { knob: "crowd_tier_distance", value: 0.7 },
  ]);
  assert(Math.abs(half.crowdSilhouettePixels - 12) < 1e-9);
  // Below the knob's floor the threshold stays at the readability floor.
  const under = rendererDetailFromKnobs([
    { knob: "crowd_tier_distance", value: 0.2 },
  ]);
  assert.equal(under.crowdSilhouettePixels, 16);
});

test("sustained overload lowers renderer settings without passing the floors", () => {
  const driver = new DetailDriver(defaultProfile());
  for (let i = 0; i < 5000; i++) {
    driver.observe({ interval_ms: 40 });
    const r = driver.renderer();
    assert.ok(
      r.renderScale >= 0.5 &&
        r.foliageReach >= 0.4 &&
        r.crowdReach >= 0.4 &&
        r.crowdSilhouettePixels <= 16,
    );
  }
  assert.equal(driver.detail, 0);
  assert.equal(driver.report().controller.pressure, "at_floor");
});

test("interface knobs write CSS variables; reduced motion turns motion off", () => {
  const props = new Map<string, string>();
  const target = {
    style: { setProperty: (k: string, v: string) => void props.set(k, v) },
    dataset: {} as Record<string, string | undefined>,
  };
  const profile = defaultProfile();
  const driver = new DetailDriver(profile);
  driver.applyInterface(target);
  assert.deepEqual(Object.fromEntries(props), {
    "--detail": "1.000",
    "--detail-blur": "1.000",
    "--detail-shadow": "1.000",
    "--detail-motion": "1.000",
    "--detail-effects": "1.000",
  });
  assert.equal(target.dataset.detailMotion, "on");
  driver.updateProfile({
    ...profile,
    prefs: { ...profile.prefs, reduced_motion: true },
  });
  driver.applyInterface(target);
  assert.equal(props.get("--detail-motion"), "0.000");
  assert.equal(props.get("--detail-effects"), "0.000");
  assert.equal(props.get("--detail-blur"), "1.000");
  assert.equal(target.dataset.detailMotion, "off");
  assert.equal(driver.detail, 1, "a preference never moves Detail");
});

test("holding detail pins it through load", () => {
  const driver = new DetailDriver(defaultProfile());
  driver.hold(1);
  for (let i = 0; i < 2000; i++) driver.observe({ interval_ms: 60 });
  assert.equal(driver.detail, 1);
  assert.equal(driver.report().controller.held, true);
  driver.release();
  assert.equal(driver.report().controller.held, false);
});

function sameShape(a: unknown, b: unknown, path = "$") {
  if (
    a &&
    b &&
    typeof a === "object" &&
    typeof b === "object" &&
    !Array.isArray(a)
  ) {
    if (path === "$.gpu.limits") return;
    assert.deepEqual(Object.keys(a).sort(), Object.keys(b).sort(), path);
    for (const k of Object.keys(a))
      sameShape((a as any)[k], (b as any)[k], `${path}.${k}`);
  }
}

function fakeEnvironment(overrides: Partial<ProbeEnvironment> = {}) {
  const queries = new Set([
    "(pointer: coarse)",
    "(any-pointer: coarse)",
    "(prefers-reduced-motion: reduce)",
    "(dynamic-range: high)",
  ]);
  class Limits {
    get maxTextureDimension2D() {
      return 8192;
    }
    get maxBufferSize() {
      return 268435456;
    }
  }
  const navigator = new Proxy(
    {
      gpu: {
        requestAdapter: async () => ({
          info: { vendor: "apple", architecture: "metal-3" },
          features: new Set(["timestamp-query", "depth-clip-control"]),
          limits: new Limits(),
        }),
      },
      maxTouchPoints: 5,
      getGamepads: () => [],
      connection: {
        effectiveType: "4g",
        downlink: 10,
        rtt: 50,
        saveData: false,
      },
      getBattery: async () => ({ level: 0.5, charging: true }),
    },
    {
      get(target, key) {
        if (
          [
            "userAgent",
            "userAgentData",
            "platform",
            "appVersion",
            "vendor",
          ].includes(String(key))
        )
          throw new Error(`The probe read navigator.${String(key)}.`);
        return (target as any)[key];
      },
    },
  );
  let now = 0;
  return {
    navigator,
    matchMedia: (q: string) => ({ matches: queries.has(q) }),
    devicePixelRatio: 3,
    innerWidth: 390,
    innerHeight: 844,
    requestAnimationFrame: (cb: (t: number) => void) =>
      setTimeout(() => cb((now += 1000 / 60)), 0),
    rootFontSize: () => 20,
    ...overrides,
  } as ProbeEnvironment;
}

test("the probe reads platform signals and never the user agent", async () => {
  const p = await probeDeviceProfile(fakeEnvironment(), { refreshFrames: 6 });
  sameShape(p, read("profile-web.json"));
  assert.equal(p.gpu.api, "webgpu");
  assert.equal(p.gpu.class, "baseline");
  assert.deepEqual(p.gpu.features, ["depth-clip-control", "timestamp-query"]);
  assert.deepEqual(p.gpu.limits, {
    maxBufferSize: 268435456,
    maxTextureDimension2D: 8192,
  });
  assert.deepEqual(p.display, {
    width: 390,
    height: 844,
    scale: 3,
    refresh_hz: 60,
    hdr: true,
  });
  assert.equal(p.input.pointer, "coarse");
  assert.equal(p.input.hover, false);
  assert.equal(p.input.touch_points, 5);
  assert.equal(p.power.battery_level, 0.5);
  assert.equal(p.network.effective_type, "4g");
  assert.equal(p.prefs.reduced_motion, true);
  assert.equal(p.prefs.reduced_transparency, false);
  assert.equal(p.prefs.text_scale, 1.25);
});

test("the probe degrades to defaults when the platform says nothing", async () => {
  const p: DeviceProfile = await probeDeviceProfile(
    { navigator: {} },
    { refreshFrames: 0 },
  );
  sameShape(p, read("profile-web.json"));
  assert.equal(p.gpu.class, "none");
  assert.equal(p.display.refresh_hz, null);
  assert.equal(p.power.battery_level, null);
  assert.equal(p.network.effective_type, null);
});

test("a hung adapter request is bounded", async () => {
  const p = await probeDeviceProfile(
    fakeEnvironment({
      navigator: { gpu: { requestAdapter: () => new Promise(() => {}) } },
    }),
    { refreshFrames: 0, timeoutMs: 20 },
  );
  assert.equal(p.gpu.api, "none");
});

test("grass reach shortens the draw distance but keeps the near ring", () => {
  const planner = new GrassPatchPlanner({
    schema: "aethyrforge.grass.v1",
    profile: "quick-grass-v1",
    enabled: true,
    groundHeight: 0,
    bladeWidth: 0.1,
    bladeHeight: 0.5,
    lodDistance: 10,
    maxDistance: 60,
    windStrength: 1,
    interactionRadius: 1,
  });
  assert.equal(planner.drawDistance, 60);
  planner.reach = 0.5;
  assert.equal(planner.drawDistance, 30);
  planner.reach = 0.1;
  assert.equal(planner.drawDistance, 15);
});
