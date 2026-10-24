/** Focused production-GPU tactical fixtures. No rendering or frame-rate claim. */
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
  schema: "aethyrforge.battle-ai-gpu.v1",
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
const blank = new URL("/battle-ai-probe.html", origin).href;
await page.route(blank, (r) =>
  r.fulfill({
    status: 200,
    contentType: "text/html",
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
    body: "<!doctype html><title>GPU tactical regression</title>",
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
        { battleSquadCount },
      ] = await Promise.all([
        import("/packages/renderer/battle-crowd-shader.ts"),
        import("/packages/renderer/battle-grid.ts"),
        import("/packages/renderer/battle-combat-shader.ts"),
        import("/tools/battle-ai-metrics.mjs"),
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
      window.__battleAiCleanup = cleanup;
      const evidence = { cases: [], shaderSha256: null };
      window.__battleAiEvidence = evidence;
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
        const capacity = 64,
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
        const initialize = (name, actors, r = 100, siege = false) => {
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
          au[9] = 200;
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
          device.queue.writeBuffer(uniform, 0, frame);
          const result = {
            name,
            kind: "synthetic-initial-state-production-compute",
            radius: r,
            initial,
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
            total = stateSize + 256 + layout.BATTLE_COMBAT_BYTES;
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
              0,
              rb,
              stateSize + 256,
              layout.BATTLE_COMBAT_BYTES,
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
            const stats = [...new Uint32Array(raw, stateSize, 64)],
              base = stateSize + 256,
              occ = new Uint32Array(
                raw,
                base +
                  layout.BATTLE_PROJECTILE_OFFSET +
                  layout.BATTLE_PROJECTILE_CAPACITY * 64,
                layout.BATTLE_PROJECTILE_CAPACITY,
              ),
              shots = [];
            for (let id = 0; id < occ.length; id++) {
              if (!occ[id]) continue;
              const f = new Float32Array(
                  raw,
                  base + layout.BATTLE_PROJECTILE_OFFSET + id * 64,
                  16,
                ),
                u = new Uint32Array(f.buffer, f.byteOffset, 16);
              shots.push({
                id,
                position: [...f.slice(0, 3)],
                age: f[3],
                velocity: [...f.slice(4, 7)],
                flight: f[7],
                target: [...f.slice(8, 11)],
                team: u[12],
                damage: u[13],
                kind: u[14],
                targetId: u[15],
              });
            }
            const sieges = Array.from({ length: 8 }, (_, id) => {
              const f = new Float32Array(raw, base + 96 + id * 32, 8);
              return {
                id,
                position: [...f.slice(0, 3)],
                cooldown: f[4],
                team: f[6],
              };
            });
            need(stats[11] === tick, "GPU tick did not advance exactly once");
            need(
              stats[8] === 0 && stats[54] === 0 && stats[38] === 0,
              "AI fixture overflow",
            );
            return { tick, actors, shots, sieges, stats };
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
        // The same first actor is a source-assigned ranged role for count two.
        let result = initialize(
          "ranged-sustained-pressure-retreat",
          [
            actor(0, 0, 0, 0, 0, { target: 1 }),
            actor(1, 4, 0, 1, 2, { target: 0, yaw: -Math.PI / 2 }),
          ],
          12,
        );
        await run(result, 360);
        const band = oracle.rangedBand(0, 1, 12),
          frames = result.frames;
        const retreat = frames.filter((s) => {
          const [a, b] = s.actors,
            d = [b.position[0] - a.position[0], b.position[2] - a.position[2]],
            n = Math.hypot(...d);
          return (
            a.state !== 2 &&
            n < band.inner &&
            a.velocity[0] * d[0] + a.velocity[1] * d[1] < -0.15 * n
          );
        });
        const premature = frames.filter((s) => {
          const a = s.actors[0],
            b = s.actors[1],
            d = Math.hypot(
              b.position[0] - a.position[0],
              b.position[2] - a.position[2],
            );
          const before = frames[s.tick - 2]?.actors[0];
          return (
            a.state === 2 &&
            before?.state !== 2 &&
            d < band.inner - 0.05 &&
            Math.abs(a.position[0]) < 11.7
          );
        });
        result.checks = {
          retreatTicks: retreat.length,
          fired: frames.some((s) =>
            s.shots.some((p) => p.kind === 0 && p.team === 0),
          ),
          noPrematureAttack: premature.length === 0,
        };
        need(
          retreat.length >= 10,
          "Ranged fighter did not actually backpedal under pressure",
        );
        need(
          !premature.length,
          "Ranged fighter entered attack while it should retreat",
        );
        result = initialize("ranged-approach-and-hold", [
          actor(0, -65, 0, 0, 0, { target: 1 }),
          actor(1, 0, 0, 1, 3, {
            target: 0,
            state: 2,
            actionTime: 0,
            cooldown: 100,
            hit: 1,
          }),
        ]);
        await run(result, 180, 3);
        const progress = result.frames.at(-1).actors[0].position[0] + 65;
        result.checks = {
          approachMetres: progress,
          forwardTicks: result.frames.filter(
            (s) => s.actors[0].velocity[0] > 0.5,
          ).length,
        };
        need(
          progress > 4,
          "Ranged fighter did not close a target beyond its preferred band",
        );
        const holdBand = oracle.rangedBand(0, 1, 100);
        result = initialize("ranged-holds-band-during-cooldown", [
          actor(0, 0, 0, 0, 0, { target: 1, cooldown: 2 }),
          actor(1, holdBand.preferred, 0, 1, 3, {
            target: 0,
            state: 2,
            actionTime: 0.1,
            hit: 1,
          }),
        ]);
        await run(result, 60, 2);
        const drift = Math.max(
          ...result.frames.map((s) =>
            Math.hypot(s.actors[0].position[0], s.actors[0].position[2]),
          ),
        );
        result.checks = {
          holdBand,
          maximumDrift: drift,
          noAttack: result.frames.every((s) => s.actors[0].state !== 2),
        };
        need(
          drift < 0.01 && result.checks.noAttack,
          "Ranged fighter did not hold its preferred range during cooldown",
        );
        result = initialize("ranged-authored-windup-moving-target-lead", [
          actor(0, -20, 0, 0, 0, {
            target: 1,
            state: 2,
            actionTime: 0.35 - 1 / 120,
            cooldown: 1,
          }),
          actor(1, 0, 0, 1, 3, {
            target: 2,
            state: 1,
            velocity: [0, 1.666667],
            yaw: Math.PI,
          }),
          actor(2, 0, 10, 0, 0, {
            target: 1,
            state: 1,
            velocity: [0, -3.4],
            yaw: 0,
          }),
        ]);
        await run(result, 1);
        let shot = result.frames[0].shots.find(
          (p) => p.kind === 0 && p.team === 0,
        );
        need(shot, "Real ranged windup did not launch");
        const expected = oracle.ledPoint(
            initial[1],
            initial[0].position,
            42,
            1.8,
          ),
          leadError = Math.hypot(
            ...shot.target.map((v, i) => v - expected.point[i]),
          );
        need(
          shot.targetId === 1 && leadError < 0.003,
          "NPC projectile does not lead the actual moving target",
        );
        await run(result, 45, 1);
        result.checks = {
          targetId: shot.targetId,
          target: shot.target,
          expected: expected.point,
          leadError,
          healthAfter: result.frames.at(-1).actors[1].health,
        };
        need(
          result.checks.healthAfter < 1000,
          "Led projectile did not damage its moving target",
        );
        need(
          result.frames.every((s) => s.actors[2].health === 200),
          "Projectile damaged a friendly actor",
        );
        result = initialize("ranged-moving-target-arena-edge-clamp", [
          actor(0, 80, 0, 0, 0, {
            target: 1,
            state: 2,
            actionTime: 0.35 - 1 / 120,
            cooldown: 1,
          }),
          actor(1, 99, 0, 1, 0, {
            target: 0,
            state: 1,
            velocity: [3.4, 0],
            yaw: Math.PI / 2,
          }),
        ]);
        await run(result, 1);
        const edgeShot = result.frames[0].shots.find(
          (p) => p.kind === 0 && p.team === 0,
        );
        need(edgeShot, "Arena-edge ranged windup did not launch");
        const rawEdgeLead = oracle.ledPoint(
          initial[1],
          initial[0].position,
          42,
          1.8,
        );
        const expectedEdge = rawEdgeLead.point.map((v, i) =>
          i === 1 ? v : Math.max(-100, Math.min(100, v)),
        );
        const edgeError = Math.hypot(
          ...edgeShot.target.map((v, i) => v - expectedEdge[i]),
        );
        result.checks = {
          targetId: edgeShot.targetId,
          target: edgeShot.target,
          unclamped: rawEdgeLead.point,
          expected: expectedEdge,
          edgeError,
        };
        need(
          rawEdgeLead.point[0] > 100.5,
          "Edge fixture did not predict beyond the actual arena",
        );
        need(
          edgeShot.targetId === 1 &&
            edgeError < 0.003 &&
            edgeShot.target[0] <= 100,
          "Ranged projectile aims beyond the arena instead of at the clamped moving target",
        );
        result = initialize(
          "cornered-ranged-last-stand",
          [
            actor(0, -12, 0, 0, 0, { target: 1 }),
            actor(1, -8, 0, 1, 3, { target: 0 }),
          ],
          12,
        );
        await run(result, 60);
        result.checks = {
          attackEntered: result.frames.some((s) => s.actors[0].state === 2),
          fired: result.frames.some((s) =>
            s.shots.some((p) => p.kind === 0 && p.team === 0),
          ),
        };
        need(
          result.checks.attackEntered && result.checks.fired,
          "Cornered ranged fighter remained trapped without firing",
        );
        result = initialize("blocked-melee-sidestep-and-forward-recovery", [
          actor(0, 10, 0, 1, 0, {
            target: 1,
            state: 2,
            actionTime: 0.1,
            cooldown: 4,
            hit: 1,
          }),
          actor(1, -1.4, 0, 0, 0, { target: 0, yaw: Math.PI / 2 }),
          actor(2, 0, 0, 0, 0, {
            target: 0,
            state: 2,
            actionTime: 0.1,
            cooldown: 4,
            hit: 1,
          }),
        ]);
        await run(result, 120);
        const lateral = Math.max(
          ...result.frames
            .slice(0, 60)
            .map((s) => Math.abs(s.actors[1].position[2])),
        );
        let minimumCoreGap = Infinity;
        for (const frame of result.frames)
          for (let a = 0; a < frame.actors.length; a++)
            for (let b = a + 1; b < frame.actors.length; b++) {
              const left = frame.actors[a],
                right = frame.actors[b];
              if (left.health <= 0 || right.health <= 0) continue;
              minimumCoreGap = Math.min(
                minimumCoreGap,
                Math.hypot(
                  left.position[0] - right.position[0],
                  left.position[2] - right.position[2],
                ) -
                  BATTLE_BODY_RADII[left.kind] -
                  BATTLE_BODY_RADII[right.kind],
              );
            }
        const forward = result.frames.at(-1).actors[1].position[0] + 1.4;
        result.checks = {
          lateralEscapeMetres: lateral,
          forwardRecoveryMetres: forward,
          minimumCoreGap,
        };
        need(
          lateral > 0.3,
          "Stationary friendly blocker did not produce actual lateral escape",
        );
        need(
          forward > 0.5,
          "Blocked melee fighter did not recover forward progress",
        );
        need(
          minimumCoreGap >= 0.08,
          "Melee escape penetrated a living body core",
        );
        const fixture = Array.from({ length: 48 }, (_, id) =>
          id < 24
            ? actor(id, -60 + (id % 6) * 3, 20 + Math.floor(id / 6) * 3, 0, 0)
            : id < 42
              ? actor(
                  id,
                  5 + ((id - 24) % 6) * 2.5,
                  -6 + Math.floor((id - 24) / 6) * 2.5,
                  1,
                  2,
                  { state: id % 3 === 0 ? 2 : 0, actionTime: 0.1, hit: 1 },
                )
              : actor(id, 45 + (id - 42) * 5, -35, 1, 2),
        );
        Object.assign(fixture[32], {
          kind: 0,
          health: 200,
          state: 1,
          velocity: [3.4, 0],
        });
        fixture[31].health = 0;
        fixture[31].state = 3;
        fixture[39].velocity = [3, 4];
        fixture[39].state = 1;
        result = initialize(
          "siege-live-cluster-and-capped-lead",
          fixture,
          100,
          true,
        );
        await run(result, 1);
        const first = result.frames[0],
          decisions = [];
        for (let id = 0; id < 8; id++) {
          const siege = first.sieges[id],
            choice = oracle.siegeChoice(initial, siege, id, 1, 100),
            p = first.shots.find(
              (p) =>
                p.kind === 1 &&
                p.team === siege.team &&
                p.targetId === choice.id &&
                Math.abs(p.position[2] - siege.position[2]) < 2,
            );
          need(
            choice.id !== null && p,
            `Siege ${id} did not fire at independently selected living cluster`,
          );
          const error = Math.hypot(
            ...p.target.map((v, i) => v - choice.point[i]),
          );
          need(
            error < 0.005,
            `Siege ${id} lead differs from selected live actor`,
          );
          decisions.push({
            siege: id,
            chosen: choice.id,
            score: choice.score,
            target: p.target,
            expected: choice.point,
            error,
            candidates: choice.candidates,
          });
        }
        result.checks = { decisions };
        need(
          decisions.some(
            (d) => Math.abs(d.target[0] - initial[d.chosen].position[0]) > 5.99,
          ),
          "No real siege decision exercised the six-metre lead cap",
        );
        need(
          decisions.filter((d) => d.siege % 2 === 0).every((d) => d.score > 1),
          "Siege did not prefer the populated opposing cluster",
        );
        result = initialize(
          "siege-no-living-target-skips-shot",
          fixture.map((a) => ({ ...a, health: 0, state: 3 })),
          100,
          true,
        );
        await run(result, 1);
        const skipped = result.frames[0];
        result.checks = {
          shots: skipped.shots.length,
          cooldowns: skipped.sieges.map((s) => s.cooldown),
        };
        need(skipped.shots.length === 0, "Siege fired at a dead-only army");
        need(
          skipped.sieges.every((s) => Math.abs(s.cooldown - 0.5) < 1e-5),
          "Siege did not schedule a bounded retry for missing targets",
        );
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
        () => reject(Error("GPU tactical check exceeded 120 seconds")),
        120000,
      );
    }),
  ]);
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(
    `PASS ${report.result.cases.length} production GPU tactical fixtures`,
  );
} catch (error) {
  report.partialResult = await page
    .evaluate(() => window.__battleAiEvidence ?? null)
    .catch(() => null);
  report.errors.push({ error: String(error), stack: error.stack });
  console.error(error);
  process.exitCode = 1;
} finally {
  clearTimeout(timer);
  closing = true;
  await page.evaluate(() => window.__battleAiCleanup?.()).catch(() => {});
  await context.close();
  await browser.close();
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-ai-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
