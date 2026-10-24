/** Focused production-GPU squad fixtures. No rendering or frame-rate claim. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { battleSourcePin } from "./battle-evidence.mjs";
const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(output !== root && !output.startsWith(root + sep));
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const origin = new URL(process.env.AF_PREVIEW_URL || "http://127.0.0.1:55327/");
assert(["localhost", "127.0.0.1"].includes(origin.hostname));
const report = {
  schema: "aethyrforge.battle-squads-gpu.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  sourcePin: await battleSourcePin(root),
  errors: [],
  failed: [],
  limitations: [
    "Diagnostic initial actor/health/clock and siege cooldown fixtures are written before the first tick only. All later motion, targeting, firing, impacts, and damage come from the unmodified production compute shader.",
    "Flat terrain isolates tactical decisions; this checks neither rendering nor performance. The separate existing full kernel check covers naturally seeded mixed crowds, live materials, motion, contacts, player combat, and Meteor Shower.",
  ],
};
const browser = await chromium.connectOverCDP(endpoint),
  context = await browser.newContext({ serviceWorkers: "block" }),
  page = await context.newPage();
let closing = false,
  timer;
page.on("pageerror", (e) => report.errors.push(String(e)));
page.on("requestfailed", (r) => {
  if (!closing)
    report.failed.push({ url: r.url(), error: r.failure()?.errorText });
});
page.on("response", (r) => {
  if (!closing && r.status() >= 400)
    report.failed.push({ url: r.url(), status: r.status() });
});
const blank = new URL("/battle-squad-probe.html", origin).href;
await page.route(blank, (r) =>
  r.fulfill({
    status: 200,
    contentType: "text/html",
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
    body: "<!doctype html><title>GPU squad regression</title>",
  }),
);
try {
  await page.goto(blank);
  report.result = await Promise.race([
    page.evaluate(async () => {
      const need = (v, m) => {
        if (!v) throw Error(m);
      };
      const [
        { BATTLE_SIMULATION_WGSL },
        { BATTLE_GRID, BATTLE_BODY_RADII },
        layout,
        oracle,
        { battleSquadCount, battleSquadId },
      ] = await Promise.all([
        import("/packages/renderer/battle-crowd-shader.ts"),
        import("/packages/renderer/battle-grid.ts"),
        import("/packages/renderer/battle-combat-shader.ts"),
        import("/tools/battle-squad-metrics.mjs"),
        import("/packages/renderer/battle-squad-shader.ts"),
      ]);
      const adapter = await navigator.gpu?.requestAdapter({
        powerPreference: "high-performance",
      });
      need(adapter, "WebGPU adapter unavailable");
      const device = await adapter.requestDevice({
          requiredLimits: { maxStorageBuffersPerShaderStage: 8 },
        }),
        errors = [],
        buffers = [];
      let closed = false;
      device.addEventListener("uncapturederror", (e) =>
        errors.push(String(e.error)),
      );
      void device.lost.then((info) => {
        if (!closed) errors.push(info.message);
      });
      const cleanup = () => {
        if (closed) return;
        closed = true;
        buffers.forEach((b) => b.destroy());
        device.destroy();
      };
      window.__battleSquadCleanup = cleanup;
      const evidence = { cases: [], shaderSha256: null };
      window.__battleSquadEvidence = evidence;
      try {
        evidence.shaderSha256 = [
          ...new Uint8Array(
            await crypto.subtle.digest(
              "SHA-256",
              new TextEncoder().encode(BATTLE_SIMULATION_WGSL),
            ),
          ),
        ]
          .map((x) => x.toString(16).padStart(2, "0"))
          .join("");
        const capacity = 128,
          storage =
            GPUBufferUsage.STORAGE |
            GPUBufferUsage.COPY_SRC |
            GPUBufferUsage.COPY_DST;
        const buffer = (size, usage = storage) => {
          const b = device.createBuffer({ size, usage });
          buffers.push(b);
          return b;
        };
        const states = [buffer(capacity * 64), buffer(capacity * 64)],
          damage = [buffer(capacity * 8), buffer(capacity * 8)],
          grid = buffer(BATTLE_GRID.dimension ** 2 * 268),
          arena = buffer(layout.BATTLE_COMBAT_BYTES),
          counters = buffer(256),
          heights = buffer(16),
          uniform = buffer(
            336,
            GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
          );
        const groupLayout = device.createBindGroupLayout({
          entries: Array.from({ length: 9 }, (_, binding) => ({
            binding,
            visibility: GPUShaderStage.COMPUTE,
            buffer: {
              type:
                binding === 8
                  ? "uniform"
                  : [0, 4, 7].includes(binding)
                    ? "read-only-storage"
                    : "storage",
            },
          })),
        });
        const module = device.createShaderModule({
            code: BATTLE_SIMULATION_WGSL,
          }),
          pipelineLayout = device.createPipelineLayout({
            bindGroupLayouts: [groupLayout],
          });
        const compile = await module.getCompilationInfo();
        need(
          !compile.messages.some((m) => m.type === "error"),
          JSON.stringify(
            compile.messages.map(({ type, message, lineNum, linePos }) => ({
              type,
              message,
              lineNum,
              linePos,
            })),
          ),
        );
        const pipelines = await Promise.all(
          [
            "clear",
            "scatter",
            "commandSquads",
            "simulate",
            "participants",
            "projectiles",
          ].map((entryPoint) =>
            device.createComputePipelineAsync({
              layout: pipelineLayout,
              compute: { module, entryPoint },
            }),
          ),
        );
        const groups = states.map((s, i) =>
          device.createBindGroup({
            layout: groupLayout,
            entries: [
              s,
              states[1 - i],
              grid,
              arena,
              damage[i],
              damage[1 - i],
              counters,
              heights,
              uniform,
            ].map((b, binding) => ({ binding, resource: { buffer: b } })),
          }),
        );
        let front = 0,
          count = 0,
          radius = 100,
          tick = 0,
          initial;
        const actor = (id, x, z, team, kind = 0, extra = {}) => ({
          id,
          position: [x, 0, z],
          yaw: team ? -Math.PI / 2 : Math.PI / 2,
          velocity: [0, 0],
          health: [200, 100, 20, 1000][kind],
          kind,
          team,
          target: 0xffffffff,
          state: 0,
          actionTime: 0,
          cooldown: 0,
          hit: 0,
          ...extra,
        });
        const initialize = (
          name,
          actors,
          r = 100,
          siege = false,
          participant = null,
        ) => {
          front = 0;
          tick = 0;
          count = actors.length;
          radius = r;
          initial = structuredClone(actors);
          const bytes = new ArrayBuffer(capacity * 64),
            f = new Float32Array(bytes),
            u = new Uint32Array(bytes);
          for (const a of actors) {
            const o = a.id * 16;
            f.set(
              [
                ...a.position,
                a.yaw,
                ...a.velocity,
                a.health,
                a.actionTime,
                a.cooldown,
                0,
                0,
                a.hit,
              ],
              o,
            );
            u.set([a.kind, a.team, a.target, a.state], o + 12);
          }
          for (const b of states) device.queue.writeBuffer(b, 0, bytes);
          for (const b of damage)
            device.queue.writeBuffer(b, 0, new Uint32Array(capacity * 2));
          device.queue.writeBuffer(counters, 0, new Uint32Array(64));
          const raw = new ArrayBuffer(layout.BATTLE_COMBAT_BYTES),
            af = new Float32Array(raw),
            au = new Uint32Array(raw);
          au[9] = participant?.health ?? 200;
          au[14] = 1;
          for (let id = 0; id < 8; id++) {
            const team = id % 2;
            af.set(
              [
                (team ? 1 : -1) * r * 0.88,
                0,
                (Math.floor(id / 2) - 1.5) * r * 0.43,
                0,
                siege ? 0 : 600,
                0,
                team,
                750,
              ],
              24 + id * 8,
            );
          }
          device.queue.writeBuffer(arena, 0, raw);
          const frame = new ArrayBuffer(336),
            ff = new Float32Array(frame),
            fu = new Uint32Array(frame);
          ff.set([0, 0, r, 1 / 60], 20);
          fu.set([count, capacity, 1, 0], 24);
          ff.set([0, 0, 512, 2], 28);
          ff.set([135, 2000, 18, 0], 32);
          if (participant) {
            ff.set([...participant.position, Math.PI], 72);
            ff.set([0, 0, 0, 0], 76);
            fu.set([participant.team, Number(participant.active), 0, 0], 80);
          }
          device.queue.writeBuffer(uniform, 0, frame);
          const result = {
            name,
            kind: "synthetic-initial-state-production-compute",
            radius: r,
            initial,
            participant,
            frames: [],
            checks: {},
          };
          evidence.cases.push(result);
          return result;
        };
        const step = async () => {
          device.pushErrorScope("validation");
          const e = device.createCommandEncoder(),
            p = e.beginComputePass();
          p.setBindGroup(0, groups[front]);
          const dispatch = [
            Math.ceil(Math.max(BATTLE_GRID.dimension ** 2, count) / 128),
            Math.ceil(count / 128),
            Math.ceil(battleSquadCount(count) / 128),
            Math.ceil(count / 128),
            1,
            layout.BATTLE_PROJECTILE_CAPACITY / 128,
          ];
          for (let i = 0; i < pipelines.length; i++) {
            p.setPipeline(pipelines[i]);
            p.dispatchWorkgroups(dispatch[i]);
          }
          p.end();
          device.queue.submit([e.finish()]);
          front = 1 - front;
          tick++;
          const error = await device.popErrorScope();
          need(!error, error?.message);
        };
        const read = async () => {
          const stateSize = count * 64,
            squadCount = battleSquadCount(count),
            squadSize = squadCount * 64,
            total = stateSize + 256 + squadSize;
          const rb = device.createBuffer({
            size: total,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          });
          try {
            const e = device.createCommandEncoder();
            e.copyBufferToBuffer(states[front], 0, rb, 0, stateSize);
            e.copyBufferToBuffer(counters, 0, rb, stateSize, 256);
            e.copyBufferToBuffer(
              arena,
              layout.BATTLE_SQUAD_OFFSET,
              rb,
              stateSize + 256,
              squadSize,
            );
            device.queue.submit([e.finish()]);
            await rb.mapAsync(GPUMapMode.READ);
            const raw = rb.getMappedRange().slice(0);
            const actors = Array.from({ length: count }, (_, id) => {
              const f = new Float32Array(raw, id * 64, 16),
                u = new Uint32Array(raw, id * 64, 16);
              need(
                [...f.slice(0, 12)].every(Number.isFinite),
                `Nonfinite actor ${id}`,
              );
              return {
                id,
                position: [...f.slice(0, 3)],
                yaw: f[3],
                velocity: [f[4], f[5]],
                health: f[6],
                actionTime: f[7],
                kind: u[12],
                team: u[13],
                target: u[14],
                state: u[15],
              };
            });
            const stats = [...new Uint32Array(raw, stateSize, 64)];
            need(stats[11] === tick, "GPU tick did not advance once");
            need(
              stats[8] === 0 && stats[54] === 0 && stats[38] === 0,
              "Squad fixture overflow",
            );
            const squads = Array.from({ length: squadCount }, (_, id) => {
              const f = new Float32Array(raw, stateSize + 256 + id * 64, 16),
                u = new Uint32Array(f.buffer, f.byteOffset, 16);
              need(
                [...f.slice(0, 12)].every(Number.isFinite),
                `Nonfinite squad ${id}`,
              );
              return {
                id,
                center: [...f.slice(0, 2)],
                strength: f[2],
                alive: f[3],
                anchor: [...f.slice(4, 6)],
                ranged: f[6],
                enemies: f[7],
                goal: [...f.slice(8, 10)],
                order: f[10],
                threatDistance: f[11],
                updatedTick: u[12],
                orderSinceTick: u[13],
                targetId: u[14],
                team: u[15],
              };
            });
            return { tick, actors, squads, stats };
          } finally {
            if (rb.mapState === "mapped") rb.unmap();
            rb.destroy();
          }
        };
        const run = async (result, steps, stride = 1) => {
          for (let i = 0; i < steps; i++) {
            await step();
            if (i % stride === 0 || i === steps - 1)
              result.frames.push(await read());
          }
          return result.frames.at(-1);
        };

        const ranged = (id) => {
          const columns = 8;
          return (
            id % 4 === 0 &&
            Math.abs(((id % columns) + 0.5) / columns - 0.5) * 2 > 0.35
          );
        };
        const members = Array.from({ length: 32 }, (_, id) => id),
          guards = members.filter((id) => !ranged(id)),
          shooters = members.filter(ranged);
        const formation = (
          health,
          enemyCenter = [30, 0],
          withoutRanged = false,
        ) =>
          Array.from({ length: 64 }, (_, id) => {
            if (id < 32)
              return actor(
                id,
                ((id % 8) - 3.5) * 2.5,
                (Math.floor(id / 8) - 1.5) * 2.5,
                0,
                0,
                {
                  health: withoutRanged && ranged(id) ? 0 : health,
                  state: withoutRanged && ranged(id) ? 3 : 0,
                  target: 35,
                  cooldown: 3,
                },
              );
            const slot = id - 32;
            return actor(
              id,
              enemyCenter[0] + ((slot % 8) - 3.5) * 3,
              enemyCenter[1] + (Math.floor(slot / 8) - 1.5) * 3,
              1,
              3,
              { target: 15, state: 2, actionTime: 0, hit: 1, cooldown: 10 },
            );
          });
        const ownSquad = battleSquadId(0, 64, 0);
        const orders = (result) =>
          result.frames.map((frame) => frame.squads[ownSquad]);
        const finish = (result) => {
          result.checks.clearance = oracle.squadClearance(
            result.frames,
            BATTLE_BODY_RADII,
          );
          need(
            result.checks.clearance.minimumCoreGap >= 0.075,
            `${result.name} lost living core clearance`,
          );
          const records = orders(result);
          need(
            records.every(
              (r) =>
                r.team === 0 &&
                r.alive >= 4 &&
                r.updatedTick >= 1 &&
                r.updatedTick <= result.frames.at(-1).tick,
            ),
            "Invalid squad order metadata",
          );
          need(
            records.every((r) => (r.updatedTick - 1) % 12 === 0),
            "Squad commander ran outside its bounded cadence",
          );
        };
        let result = initialize(
          "weak-near-squad-shared-withdraw",
          formation(40),
        );
        await run(result, 120);
        result.checks.motion = oracle.squadMotion(
          initial,
          result.frames,
          members,
          [-1, 0],
          1,
          0.75,
        );
        result.checks.orderKinds = [
          ...new Set(orders(result).map((r) => r.order)),
        ];
        result.checks.orderSince = [
          ...new Set(orders(result).map((r) => r.orderSinceTick)),
        ];
        need(result.checks.motion.ok, "Weak squad did not withdraw together");
        need(
          result.checks.orderKinds.length === 1 &&
            result.checks.orderKinds[0] === 2,
          "Weak close squad did not share a withdrawal order",
        );
        need(
          result.checks.orderSince.length === 1 &&
            result.checks.orderSince[0] === 1,
          "Withdrawal did not retain its shared order",
        );
        finish(result);
        const retreat = result;
        result = initialize(
          "weak-far-pressure-negative-control",
          formation(40, [80, 0]),
        );
        await run(result, 120);
        result.checks.motion = oracle.squadMotion(
          initial,
          result.frames,
          members,
          [1, 0],
          1,
          0.75,
        );
        result.checks.orderKinds = [
          ...new Set(orders(result).map((r) => r.order)),
        ];
        need(
          result.checks.motion.ok,
          "Weak unpressured squad did not keep advancing",
        );
        need(
          !result.checks.orderKinds.includes(2),
          "Low health alone caused retreat without nearby pressure",
        );
        finish(result);
        result.checks.pairedRetreatMean = retreat.checks.motion.meanProjection;
        result = initialize(
          "healthy-squad-stable-shared-flank",
          formation(200),
        );
        await run(result, 120);
        const flankSign = Math.sign(result.frames[0].squads[ownSquad].goal[1]);
        result.checks.motion = oracle.squadMotion(
          initial,
          result.frames,
          guards,
          [0, flankSign],
          0.5,
          0.75,
        );
        result.checks.orderKinds = [
          ...new Set(orders(result).map((r) => r.order)),
        ];
        result.checks.orderSince = [
          ...new Set(orders(result).map((r) => r.orderSinceTick)),
        ];
        need(
          flankSign !== 0 && result.checks.motion.ok,
          "Healthy squad members did not move along their common flank",
        );
        need(
          result.checks.orderKinds.every((k) => k === 1) &&
            result.checks.orderSince.every((t) => t === 1),
          "Flank order oscillated or was not shared",
        );
        finish(result);
        result = initialize(
          "ranged-anchor-shared-melee-screen",
          formation(200, [-7.25, 19.5]),
        );
        await run(result, 60);
        const early = result.frames.slice(0, 30);
        result.checks.guards = oracle.squadMotion(
          initial,
          early,
          guards,
          [0, 1],
          0.15,
          0.65,
        );
        result.checks.ranged = oracle.squadMotion(
          initial,
          early,
          shooters,
          [0, -1],
          0.15,
          0.75,
        );
        result.checks.initialOrder = result.frames[0].squads[ownSquad];
        need(
          result.checks.initialOrder.order === 3 &&
            result.checks.initialOrder.ranged === shooters.length,
          "Threatened ranged anchor did not share a screening objective",
        );
        need(
          result.checks.guards.ok && result.checks.ranged.ok,
          "Melee guards did not approach the threat while ranged members withdrew",
        );
        finish(result);
        const screen = result;
        result = initialize(
          "no-ranged-screen-negative-control",
          formation(200, [-7.25, 19.5], true),
        );
        await run(result, 60);
        result.checks.orderKinds = [
          ...new Set(orders(result).map((r) => r.order)),
        ];
        result.checks.initialOrder = result.frames[0].squads[ownSquad];
        need(
          result.checks.initialOrder.ranged === 0 &&
            !result.checks.orderKinds.includes(3),
          "Squad screened a nonexistent ranged anchor",
        );
        finish(result);
        result.checks.pairedScreenOrder = screen.checks.initialOrder.order;

        const human = {
          position: [-8.75, 0, 12],
          team: 1,
          active: true,
          health: 200,
        };
        result = initialize(
          "opposing-human-triggers-shared-ranged-screen",
          formation(200, [80, 0]),
          100,
          false,
          human,
        );
        await run(result, 60);
        const humanEarly = result.frames.slice(0, 30);
        result.checks.guards = oracle.squadMotion(
          initial,
          humanEarly,
          guards,
          [0, 1],
          0.15,
          0.65,
        );
        result.checks.ranged = oracle.squadMotion(
          initial,
          humanEarly,
          shooters,
          [0, -1],
          0.15,
          0.75,
        );
        result.checks.initialOrder = result.frames[0].squads[ownSquad];
        need(
          result.checks.initialOrder.order === 3 &&
            result.checks.initialOrder.targetId === 0xfffffffe,
          "Opposing human did not become the shared ranged-screen threat",
        );
        need(
          result.checks.guards.ok && result.checks.ranged.ok,
          "Shared human-threat order did not move guards and ranged members apart appropriately",
        );
        finish(result);
        for (const [name, change] of [
          ["inactive", { active: false }],
          ["dead", { health: 0 }],
          ["same-team", { team: 0 }],
        ]) {
          result = initialize(
            `human-${name}-negative-control`,
            formation(200, [80, 0]),
            100,
            false,
            { ...human, ...change },
          );
          await run(result, 60);
          result.checks.orderKinds = [
            ...new Set(orders(result).map((r) => r.order)),
          ];
          result.checks.targetIds = [
            ...new Set(orders(result).map((r) => r.targetId)),
          ];
          need(
            !result.checks.orderKinds.includes(3) &&
              !result.checks.targetIds.includes(0xfffffffe),
            `A ${name} human caused an opposing-human screen order`,
          );
          finish(result);
        }
        await device.queue.onSubmittedWorkDone();
        need(!errors.length, errors.join("\n"));
        return {
          ...evidence,
          gpuErrors: errors,
          adapter: {
            vendor: adapter.info.vendor,
            architecture: adapter.info.architecture,
            description: adapter.info.description,
          },
          checks: evidence.cases.map((c) => ({ name: c.name, ok: true })),
        };
      } finally {
        cleanup();
      }
    }),
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(Error("GPU squad check exceeded 120 seconds")),
        120000,
      );
    }),
  ]);
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(
    `PASS ${report.result.cases.length} production GPU squad fixtures`,
  );
} catch (error) {
  report.partialResult = await page
    .evaluate(() => window.__battleSquadEvidence ?? null)
    .catch(() => null);
  report.errors.push({ error: String(error), stack: error.stack });
  console.error(error);
  process.exitCode = 1;
} finally {
  clearTimeout(timer);
  closing = true;
  await page.evaluate(() => window.__battleSquadCleanup?.()).catch(() => {});
  await context.close();
  await browser.close();
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-squads-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
