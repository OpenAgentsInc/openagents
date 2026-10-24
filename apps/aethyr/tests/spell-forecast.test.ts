import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  prepareGameplayCast,
  type SpellForecastRequest,
} from "../packages/runtime/spell-forecast.ts";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";

const request: SpellForecastRequest = {
  castId: "cast",
  ability: "meteor_shower",
  origin: [0, 1, 0],
  direction: [0, 0, -1],
};
function fake() {
  let clock = 0,
    ticks = 0,
    cancelled = 0,
    starts = 0;
  const calls: any[] = [];
  const reply = () => ({
    ok: true,
    forecastId: 7,
    version: 3,
    status: ticks >= 20 ? "ready" : "pending",
    phase: ticks ? "events" : "cast",
    ticks,
    restarts: 0,
    ...(ticks >= 20
      ? {
          prospectiveDestroyed: [2],
          sourceIds: [1],
          cast: { castId: "cast", ability: "meteor_shower" },
          aim: {},
        }
      : {}),
  });
  return {
    calls,
    now: () => clock,
    get cancelled() {
      return cancelled;
    },
    request(message: any) {
      calls.push(message);
      if (message.type === "gameplay_forecast_start") {
        starts++;
        clock += 0.5;
        return reply();
      }
      if (message.type === "gameplay_forecast_continue") {
        ticks++;
        clock += 0.5;
        return reply();
      }
      cancelled++;
      return { ok: true };
    },
  };
}

test("native one-tick continuations batch to a small budget without a timer per tick", async () => {
  const kernel = fake();
  let yields = 0;
  const result = await prepareGameplayCast(kernel, request, {
    now: kernel.now,
    yieldControl: async () => {
      yields++;
    },
    budgetMs: 2,
  });
  assert.equal(result.plan.version, 3);
  assert.deepEqual(result.plan.prospectiveDestroyed, [2]);
  assert.equal(result.metrics.continueCalls, 20);
  assert.equal(result.metrics.ticks, 20);
  assert.equal(result.metrics.beginMs, 0.5);
  assert.equal(result.metrics.longestTickMs, 0.5);
  assert.equal(result.metrics.totalCpuMs, 10.5);
  assert(yields > 0 && yields < 10);
  assert.equal(kernel.cancelled, 0);
});

test("abort, rejected preparation and yield failure cancel only this invisible native candidate", async () => {
  for (const mode of ["signal", "callback", "visuals", "yield"] as const) {
    const kernel = fake(),
      signal = new AbortController();
    let cancelled = false;
    const pending = prepareGameplayCast(kernel, request, {
      now: kernel.now,
      signal: signal.signal,
      cancelled: () => cancelled,
      budgetMs: 2,
      prepareVisuals: mode === "visuals" ? async () => false : undefined,
      yieldControl: async () => {
        if (mode === "signal") signal.abort();
        if (mode === "callback") cancelled = true;
        if (mode === "yield") throw Error("Yield failed");
      },
    });
    await assert.rejects(
      pending,
      mode === "yield"
        ? /Yield failed/
        : mode === "visuals"
          ? /fragments/
          : /canceled/,
    );
    assert.equal(kernel.cancelled, 1);
    assert.equal(kernel.calls.at(-1).forecastId, 7);
  }
  const kernel = fake(),
    signal = new AbortController();
  signal.abort();
  await assert.rejects(
    prepareGameplayCast(kernel, request, { signal: signal.signal }),
    /canceled/,
  );
  assert.deepEqual(kernel.calls, []);
});

test("malformed progress or a foreign native job cannot reach visual preparation or cast", async () => {
  for (const change of [
    { forecastId: 8 },
    { ticks: 3 },
    { version: 4 },
    { restarts: 3 },
  ]) {
    const kernel = fake(),
      raw = kernel.request.bind(kernel);
    kernel.request = (message) => {
      const result = raw(message);
      return message.type === "gameplay_forecast_continue"
        ? { ...result, ...change }
        : result;
    };
    await assert.rejects(
      prepareGameplayCast(kernel, request, { now: kernel.now }),
      /incomplete/,
    );
    assert.equal(kernel.cancelled, 1);
    assert(!kernel.calls.some((call) => call.type === "gameplay_cast"));
  }
});

const bytes = await readFile(new URL("../public/kernel.wasm", import.meta.url));
async function world() {
  const kernel = await RuntimeKernel.fromBytes(bytes);
  assert.equal(
    kernel.request<any>({
      type: "init",
      spawn: { position: [0, 0, 0], mode: "walk" },
    }).ok,
    true,
  );
  const objects = Array.from({ length: 96 }, (_, id) => ({
    id,
    sourceId: (id / 2) | 0,
    min: [(id % 16) * 3 - 0.5, 0, -4 - ((id / 16) | 0) * 3 - 0.5],
    max: [(id % 16) * 3 + 0.5, 3, -4 - ((id / 16) | 0) * 3 + 0.5],
    hp: 1,
    anchored: true,
  }));
  assert.equal(
    kernel.request<any>({
      type: "gameplay_init",
      objects,
      seed: 42,
      targetGroups: { castle: objects.slice(0, 16).map((o) => o.id) },
    }).ok,
    true,
  );
  return kernel;
}
function commit(
  kernel: RuntimeKernel,
  cast: SpellForecastRequest,
  plan: Awaited<ReturnType<typeof prepareGameplayCast>>["plan"],
) {
  return kernel.request<any>({
    type: "gameplay_cast",
    ...cast,
    expectedVersion: plan.version,
    fragmentMasks: plan.prospectiveDestroyed.map((nodeId) => ({
      nodeId,
      mask: [0xffffffff, 0xffffffff],
    })),
  });
}

test("real Wasm forecasting preserves the original plan and leaves live gameplay unchanged until commit", async () => {
  for (const ability of [
    "lightning_bolt",
    "thunderbolt",
    "meteor_shower",
    "destroy_castle",
  ] as const) {
    const kernel = await world(),
      reference = await world(),
      cast = { ...request, ability };
    const before = kernel.request<any>({ type: "gameplay_snapshot" });
    const result = await prepareGameplayCast(kernel, cast, {
      prepareVisuals: async () => true,
    });
    assert.deepEqual(kernel.request({ type: "gameplay_snapshot" }), before);
    const expected = reference.request<any>({
      type: "gameplay_prepare_cast",
      ...cast,
    });
    for (const field of [
      "version",
      "prospectiveDestroyed",
      "sourceIds",
      "cast",
      "aim",
    ] as const)
      assert.deepEqual(result.plan[field], expected[field], field);
    assert.deepEqual(
      commit(kernel, cast, result.plan),
      commit(reference, cast, result.plan),
    );
    assert(
      result.metrics.beginMs >= 0 && result.metrics.longestContinueMs >= 0,
    );
  }
});

test("real Wasm final revalidation restarts only when in-flight impacts make live timing relevant", async () => {
  const kernel = await world(),
    first = { ...request, castId: "first" };
  const old = await prepareGameplayCast(kernel, first);
  assert.equal(commit(kernel, first, old.plan).ok, true);
  for (let tick = 0; tick < 160; tick++)
    kernel.request({ type: "step", elapsed: 1 / 60 });
  let preparations = 0;
  const result = await prepareGameplayCast(
    kernel,
    { ...request, ability: "thunderbolt" },
    {
      prepareVisuals: async () => {
        if (preparations++ === 0)
          for (let tick = 0; tick < 10; tick++)
            kernel.request({ type: "step", elapsed: 1 / 60 });
      },
    },
  );
  assert.equal(result.metrics.restarts, 1);
  assert.equal(preparations, 2);
  assert.equal(
    commit(kernel, { ...request, ability: "thunderbolt" }, result.plan).ok,
    true,
  );
});
