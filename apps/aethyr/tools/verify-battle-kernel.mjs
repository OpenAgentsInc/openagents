/** Real GPU tail, projectile, siege and participant checks. No frame-rate claim. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { battleAssetPin, battleSourcePin } from "./battle-evidence.mjs";
import { observeApplicationBrowserMode } from "./application-pacing.mjs";

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
assert(
  ["http:", "https:"].includes(origin.protocol) &&
    ["127.0.0.1", "localhost"].includes(origin.hostname) &&
    !origin.username &&
    !origin.password,
);
const report = {
  schema: "aethyrforge.battle-kernel-browser.v4",
  startedAt: new Date().toISOString(),
  ok: false,
  origin: origin.origin,
  configuration: {
    count: 129,
    capacity: 129,
    renderSize: [128, 128],
    fixedStep: 1 / 60,
    sampleCount: 4,
    cases: [
      "front-12-steps",
      "fully-culled-3-steps",
      "reset-front-12-steps",
      "natural-ranged-and-siege",
      "participant-melee-dedupe",
      "participant-ranged",
      "participant-natural-death",
      "participant-epoch-respawn",
      "participant-meteor-six-falls-and-impacts",
      "participant-meteor-friendly-damage-attribution",
      "participant-meteor-invalid-held-cooldown",
      "participant-meteor-pending-reset-and-leave",
      "mixed-256-formation-consecutive-motion",
      "mixed-256-natural-contact-consecutive-motion",
      "rear-10000-convoy-five-seconds",
      "wasm-participant-live-body-approach-slide",
      "wasm-participant-stale-life-contacts",
      "wasm-participant-colliding-combat-death-respawn",
    ],
    projectileCapacity: 16_384,
    siegeCount: 8,
    maxParticipantAttackAttempts: 3,
    maxDeathWaitSteps: 720,
    motionCount: 256,
    motionWindows: ["first-60-ticks", "after-120-more-ticks-60-ticks"],
    motionReadbacksPerWindow: 61,
    convoyCount: 10_000,
    convoyReadbacks: 6,
    convoyDurationSeconds: 5,
    participantContactLimit: 64,
    maxContactAgeSeconds: 0.12,
    maxContactLagTicks: 8,
    maxCollidingDeathWaitSteps: 360,
    meteorCount: 6,
    meteorCastTicks: 150,
    meteorFlightTicks: 60,
    meteorCooldownTicks: 252,
    maxMeteorAttempts: 3,
  },
  errors: [],
  failed: [],
  limitations: [
    "This is an offscreen correctness check, not an application, frame-rate, or display-presentation benchmark.",
    "The test copies existing internal state and indirect argument buffers to verify all 129 actors and class/LOD accounting. It does not alter those buffers. Participant setup uses the public setParticipant/resetParticipant API, not actor teleports or health writes.",
    "Motion checks read every actor of a separate 256-member naturally seeded mixed crowd, once per controlled 60Hz tick. Core radii bound living horizontal bodies; this does not directly read vertex shader output, establish silhouette intersection, or qualify frame rate.",
    "The convoy case reads all 10,000 actors at one-second checkpoints. Its far-rear melee cohort starts beyond 94m from the front, outside the legitimate 72m stationary ranged line; no test changes actor poses or forces frontline fighters to keep moving.",
    "The local body case runs the actual fetched Rust/Wasm participant and the GPU's asynchronously published nearby living bodies. NPC poses and health are read-only; player entry/respawn and held movement/look/attack are public controller operations.",
  ],
};
report.sourcePin = await battleSourcePin(root);
report.assetPin = await battleAssetPin(
  origin.href,
  process.env.AF_BATTLE_ASSET_ROOT || process.env.AETHYRFORGE_PRIVATE_ASSETS,
);
const browser = await chromium.connectOverCDP(endpoint);
report.browserMode = await observeApplicationBrowserMode(browser);
const context = await browser.newContext({ serviceWorkers: "block" });
const page = await context.newPage();
let closing = false,
  timer;
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("requestfailed", (request) => {
  if (!closing)
    report.failed.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
});
page.on("response", (response) => {
  if (!closing && response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
});
const blank = new URL("/battle-probe.html", origin).href;
await page.route(blank, (route) =>
  route.fulfill({
    status: 200,
    contentType: "text/html",
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
    body: "<!doctype html><title>Battle kernel regression</title>",
  }),
);
try {
  await page.goto(blank);
  report.result = await Promise.race([
    page.evaluate(async () => {
      const need = (value, message) => {
        if (!value) throw Error(message);
      };
      const [
        { BattleCrowd },
        { loadBattleCrowdAssets, BATTLE_CLASSES },
        math,
        combatLayout,
        { BATTLE_BODY_RADII },
        { evaluateBattleMotion, evaluateBattleConvoy },
        { RuntimeKernel },
        { BattleParticipant },
        { evaluateBattleMeteors },
      ] = await Promise.all([
        import("/packages/renderer/battle-crowd.ts"),
        import("/packages/renderer/battle-assets.ts"),
        import("/packages/renderer/math.ts"),
        import("/packages/renderer/battle-combat-shader.ts"),
        import("/packages/renderer/battle-grid.ts"),
        import("/tools/battle-motion-metrics.mjs"),
        import("/packages/runtime/kernel.ts"),
        import("/packages/runtime/battle-participant.ts"),
        import("/tools/battle-meteor-metrics.mjs"),
      ]);
      need(navigator.gpu, "WebGPU is unavailable in this browser.");
      const adapter = await navigator.gpu.requestAdapter({
        powerPreference: "high-performance",
      });
      need(adapter, "No WebGPU adapter is available.");
      need(
        adapter.limits.maxStorageBuffersPerShaderStage >= 8,
        "The GPU needs eight storage bindings.",
      );
      const device = await adapter.requestDevice({
        requiredLimits: { maxStorageBuffersPerShaderStage: 8 },
      });
      const gpuErrors = [],
        attachments = [];
      let crowd,
        closed = false;
      device.addEventListener("uncapturederror", (event) =>
        gpuErrors.push(String(event.error)),
      );
      void device.lost.then((info) => {
        if (!closed) gpuErrors.push(`Device lost: ${info.message}`);
      });
      const cleanup = () => {
        if (closed) return;
        closed = true;
        crowd?.dispose();
        attachments.forEach((texture) => texture.destroy());
        device.destroy();
      };
      window.__battleKernelCleanup = cleanup;
      try {
        const response = await fetch("/private-assets/scene-ground.json");
        need(response.ok, "The private ground scene could not open.");
        const scene = await response.json();
        const loaded = await loadBattleCrowdAssets(scene.battleCrowdURL);
        device.pushErrorScope("validation");
        const createCrowd = (count) =>
          BattleCrowd.create(
            device,
            loaded.classes,
            {
              heights: new Float32Array(4),
              resolution: 2,
              size: 512,
              center: [0, 0],
            },
            {
              count,
              capacity: count,
              seed: 1,
              center: [0, 0, 0],
              nearDistance: 135,
              format: "rgba16float",
              depthFormat: "depth24plus",
              sampleCount: 4,
            },
          );
        crowd = await createCrowd(129);
        const color = device.createTexture({
          size: [128, 128],
          format: "rgba16float",
          sampleCount: 4,
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
        const resolved = device.createTexture({
          size: [128, 128],
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
        const depth = device.createTexture({
          size: [128, 128],
          format: "depth24plus",
          sampleCount: 4,
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
        attachments.push(color, resolved, depth);
        const creationError = await device.popErrorScope();
        need(
          !creationError,
          `GPU resource validation: ${creationError?.message}`,
        );
        const eye = [0, 60, 120];
        const front = math.multiply(
          math.perspective(Math.PI / 3, 1, 0.1, 2000),
          math.lookAt(eye, [0, 0, 0]),
        );
        const away = math.multiply(
          math.perspective(Math.PI / 3, 1, 0.1, 2000),
          math.lookAt(eye, [0, 60, 240]),
        );
        const lods = BATTLE_CLASSES.map(
          (kind) => loaded.classes.find((model) => model.kind === kind).lods,
        );
        let time = 0;
        const draw = async (matrix, steps) => {
          device.pushErrorScope("validation");
          for (let index = 0; index < steps; index++) {
            time += 1 / 60;
            const encoder = device.createCommandEncoder();
            crowd.prepare(
              encoder,
              { viewProjection: matrix, eye, time },
              1 / 60,
            );
            const pass = encoder.beginRenderPass({
              colorAttachments: [
                {
                  view: color.createView(),
                  resolveTarget: resolved.createView(),
                  clearValue: { r: 0, g: 0, b: 0, a: 1 },
                  loadOp: "clear",
                  storeOp: "discard",
                },
              ],
              depthStencilAttachment: {
                view: depth.createView(),
                depthClearValue: 1,
                depthLoadOp: "clear",
                depthStoreOp: "discard",
              },
            });
            crowd.render(pass);
            pass.end();
            device.queue.submit([encoder.finish()]);
            crowd.afterSubmit();
            await device.queue.onSubmittedWorkDone();
          }
          const validation = await device.popErrorScope();
          need(!validation, `GPU pass validation: ${validation?.message}`);
        };
        const readAll = async (count = 129) => {
          // Read-only test access: the public probe deliberately samples only 32 actors.
          const stateBytes = count * 64,
            argumentBytes = 8 * 20;
          const read = device.createBuffer({
            size: stateBytes + argumentBytes,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          });
          try {
            const encoder = device.createCommandEncoder();
            encoder.copyBufferToBuffer(
              crowd.states[crowd.front],
              0,
              read,
              0,
              stateBytes,
            );
            encoder.copyBufferToBuffer(
              crowd.indirect,
              0,
              read,
              stateBytes,
              argumentBytes,
            );
            device.queue.submit([encoder.finish()]);
            await read.mapAsync(GPUMapMode.READ);
            const bytes = read.getMappedRange().slice(0);
            const actors = Array.from({ length: count }, (_, id) => {
              const floats = new Float32Array(bytes, id * 64, 16),
                words = new Uint32Array(bytes, id * 64, 16);
              need(
                [...floats.slice(0, 12)].every(Number.isFinite),
                `Actor ${id} contains a nonfinite state.`,
              );
              need(
                words[12] < 4 && words[13] < 2 && words[15] < 4,
                `Actor ${id} has an invalid class/team/state.`,
              );
              need(
                floats[6] >= 0 && floats[6] <= [200, 100, 50, 1000][words[12]],
                `Actor ${id} has invalid health.`,
              );
              return {
                id,
                position: [...floats.slice(0, 3)],
                health: floats[6],
                kind: BATTLE_CLASSES[words[12]],
                team: words[13],
                state: words[15],
                velocity: [floats[4], 0, floats[5]],
                yaw: floats[3],
                actionTime: floats[7],
                animationTime: floats[10],
                target: words[14],
              };
            });
            const arguments_ = Array.from({ length: 8 }, (_, batch) => [
              ...new Uint32Array(bytes, stateBytes + batch * 20, 5),
            ]);
            const digest = await crypto.subtle.digest(
              "SHA-256",
              bytes.slice(0, stateBytes),
            );
            return {
              actors,
              arguments: arguments_,
              stateSha256: [...new Uint8Array(digest)]
                .map((n) => n.toString(16).padStart(2, "0"))
                .join(""),
            };
          } finally {
            if (read.mapState === "mapped") read.unmap();
            read.destroy();
          }
        };
        const readOrdnance = async (visibleCount = 0) => {
          const {
            BATTLE_COMBAT_BYTES,
            BATTLE_PROJECTILE_CAPACITY,
            BATTLE_PROJECTILE_OFFSET,
          } = combatLayout;
          need(
            BATTLE_PROJECTILE_CAPACITY === 16384,
            "The projectile pool contract changed.",
          );
          const read = device.createBuffer({
            size: BATTLE_COMBAT_BYTES,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          });
          try {
            const encoder = device.createCommandEncoder();
            encoder.copyBufferToBuffer(
              crowd.ordnance.arena,
              0,
              read,
              0,
              BATTLE_COMBAT_BYTES,
            );
            device.queue.submit([encoder.finish()]);
            await read.mapAsync(GPUMapMode.READ);
            const bytes = read.getMappedRange().slice(0),
              occupied = new Uint32Array(
                bytes,
                BATTLE_PROJECTILE_OFFSET + BATTLE_PROJECTILE_CAPACITY * 64,
                BATTLE_PROJECTILE_CAPACITY,
              );
            let active = 0;
            const byKind = [0, 0, 0, 0, 0],
              samples = [];
            let finite = true;
            for (let id = 0; id < BATTLE_PROJECTILE_CAPACITY; id++) {
              need(
                occupied[id] === 0 || occupied[id] === 1,
                `Projectile ${id} has invalid occupancy.`,
              );
              if (!occupied[id]) continue;
              active++;
              const f = new Float32Array(
                  bytes,
                  BATTLE_PROJECTILE_OFFSET + id * 64,
                  16,
                ),
                u = new Uint32Array(f.buffer, f.byteOffset, 16);
              const valid =
                [...f.slice(0, 12)].every(Number.isFinite) &&
                f[3] >= (u[14] === 3 ? -4 : 0) &&
                f[7] >= 0.29 &&
                f[7] <= 3.01 &&
                f[11] > 0 &&
                f[11] <= 8 &&
                u[12] < 2 &&
                u[14] < 5;
              finite = finite && valid;
              if (u[14] < 5) byKind[u[14]]++;
              if (
                samples.length < 16 ||
                (u[14] >= 2 &&
                  samples.filter((p) => p.kind === u[14]).length < 16)
              )
                samples.push({
                  id,
                  position: [...f.slice(0, 3)],
                  velocity: [...f.slice(4, 7)],
                  age: f[3],
                  flight: f[7],
                  radius: f[11],
                  team: u[12],
                  damage: u[13],
                  kind: u[14],
                  target: [...f.slice(8, 11)],
                  ownerEpoch: u[15],
                });
            }
            need(finite, "A live GPU projectile has invalid ballistic state.");
            need(
              Number.isInteger(visibleCount) &&
                visibleCount >= 0 &&
                visibleCount <= active,
              "Projectile visible count exceeds live pool.",
            );
            const visibleIds = [
              ...new Uint32Array(
                bytes,
                BATTLE_PROJECTILE_OFFSET +
                  BATTLE_PROJECTILE_CAPACITY * (64 + 4),
                visibleCount,
              ),
            ];
            need(
              new Set(visibleIds).size === visibleIds.length &&
                visibleIds.every(
                  (id) => id < BATTLE_PROJECTILE_CAPACITY && occupied[id] === 1,
                ),
              "Compacted projectile IDs are duplicated, invalid or inactive.",
            );
            return {
              visibleIds,
              active,
              byKind,
              finite,
              scannedSlots: BATTLE_PROJECTILE_CAPACITY,
              samples,
            };
          } finally {
            if (read.mapState === "mapped") read.unmap();
            read.destroy();
          }
        };
        const readPendingDamage = async () => {
          const buffer = crowd.ownedBuffers.find(
            (b) => b.label === `Battle crowd: delayed damage ${crowd.front}`,
          );
          need(buffer, "The actual delayed damage buffer is unavailable.");
          const read = device.createBuffer({
            size: 129 * 8,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          });
          try {
            const encoder = device.createCommandEncoder();
            encoder.copyBufferToBuffer(buffer, 0, read, 0, 129 * 8);
            device.queue.submit([encoder.finish()]);
            await read.mapAsync(GPUMapMode.READ);
            const words = new Uint32Array(read.getMappedRange().slice(0));
            return Array.from({ length: 129 }, (_, id) => ({
              id,
              amount: words[id * 2],
              local: words[id * 2 + 1],
            })).filter((d) => d.amount > 0 || d.local > 0);
          } finally {
            if (read.mapState === "mapped") read.unmap();
            read.destroy();
          }
        };
        const cases = [];
        window.__battleKernelEvidence = { cases, combat: [], participant: [] };
        const check = async (name, culled = false) => {
          const probe = await crowd.probe(),
            raw = await readAll(),
            ordnance = await readOrdnance(probe.gpu.projectilesVisible),
            gpu = probe.gpu;
          need(
            probe.finite && probe.count === 129 && raw.actors.length === 129,
            `${name}: incomplete finite state.`,
          );
          need(
            gpu.alive + gpu.dead === 129,
            `${name}: population accounting differs.`,
          );
          need(
            gpu.visible + gpu.culled === 129,
            `${name}: culling accounting differs.`,
          );
          need(
            gpu.near + gpu.far === gpu.visible,
            `${name}: LOD accounting differs.`,
          );
          need(
            gpu.visibleClasses.reduce((a, b) => a + b, 0) === gpu.visible,
            `${name}: class accounting differs.`,
          );
          let expectedTriangles = 0,
            near = 0,
            far = 0;
          raw.arguments.forEach((args, batch) => {
            const kind = Math.floor(batch / 2),
              lod = batch % 2,
              recipe = lods[kind][lod];
            need(
              args[0] === recipe.indexCount &&
                args[2] === recipe.firstIndex &&
                args[3] === 0 &&
                args[4] === 0,
              `${name}: indirect mesh range differs.`,
            );
            need(args[1] <= 129, `${name}: indirect actor count overflow.`);
            expectedTriangles += (args[0] / 3) * args[1];
            if (lod) far += args[1];
            else near += args[1];
          });
          need(
            near === gpu.near &&
              far === gpu.far &&
              expectedTriangles === gpu.triangles,
            `${name}: actual indirect draws disagree with counters.`,
          );
          for (let kind = 0; kind < 4; kind++)
            need(
              raw.arguments[kind * 2][1] + raw.arguments[kind * 2 + 1][1] ===
                gpu.visibleClasses[kind],
              `${name}: per-class indirect draws differ.`,
            );
          need(
            ordnance.active === gpu.projectilesActive,
            `${name}: projectile pool and GPU counters disagree.`,
          );
          need(
            crowd.inspect().frameDraws === 11,
            `${name}: expected eight actor plus three ordnance draws, including meteor effects.`,
          );
          need(
            probe.sieges.length === 8 &&
              probe.sieges.filter((s) => s.team === 0).length === 4 &&
              probe.sieges.filter((s) => s.team === 1).length === 4,
            `${name}: both siege teams must exist.`,
          );
          need(
            gpu.invalid === 0 &&
              gpu.gridOverflows === 0 &&
              gpu.gridOverflowsTotal === 0 &&
              gpu.projectileOverflows === 0,
            `${name}: invalid state or spatial overflow.`,
          );
          need(
            crowd.inspect().droppedSeconds === 0,
            `${name}: fixed-step time was dropped.`,
          );
          if (culled)
            need(
              gpu.projectilesVisible === 0,
              `${name}: away view retained projectile instances.`,
            );
          if (culled)
            need(
              gpu.visible === 0 &&
                gpu.culled === 129 &&
                expectedTriangles === 0,
              `${name}: away view retained draws.`,
            );
          else
            need(
              gpu.visible === 129 && gpu.culled === 0,
              `${name}: front view lost actors.`,
            );
          need(gpuErrors.length === 0, gpuErrors.join("\n"));
          cases.push({
            name,
            probe,
            raw,
            ordnance,
            expectedTriangles,
            inspect: crowd.inspect(),
          });
        };
        await draw(front, 12);
        await check("front-12-steps");
        await draw(away, 3);
        await check("fully-culled-3-steps", true);
        crowd.setCount(129);
        time = 0;
        const resetState = crowd.inspect();
        need(
          resetState.simulationSteps === 0 &&
            resetState.droppedSeconds === 0 &&
            resetState.gpu === null,
          "Reset did not clear the previous counters.",
        );
        await draw(front, 12);
        await check("reset-front-12-steps");
        need(
          cases[2].probe.tick === 12,
          "Reset did not restart the simulation tick.",
        );
        // Six simulated seconds let real rear ranks and both siege lines launch and land.
        const natural = [];
        for (let chunk = 0; chunk < 6; chunk++) {
          await draw(front, 60);
          const probe = await crowd.probe(),
            ordnance = await readOrdnance(probe.gpu.projectilesVisible);
          need(
            ordnance.active === probe.gpu.projectilesActive,
            "Natural projectile accounting differs.",
          );
          need(
            probe.gpu.invalid === 0 &&
              probe.gpu.gridOverflows === 0 &&
              probe.gpu.gridOverflowsTotal === 0 &&
              probe.gpu.projectileOverflows === 0,
            "Natural combat overflowed a bounded resource.",
          );
          natural.push({ probe, ordnance });
          window.__battleKernelEvidence.combat = natural;
        }
        const naturalLast = natural.at(-1).probe;
        need(
          natural.some(
            (p) => p.probe.gpu.rangedActors > 0 && p.probe.gpu.meleeActors > 0,
          ),
          "Both real actor roles must be present.",
        );
        need(
          naturalLast.gpu.rangedShots > 0 &&
            naturalLast.gpu.projectileImpacts > 0 &&
            naturalLast.gpu.projectileHits > 0,
          "Rear ranks did not produce real projectile impacts and damage.",
        );
        need(
          naturalLast.gpu.siegeShots >= 8 && naturalLast.gpu.siegeHits > 0,
          "Both trebuchet lines did not complete real attacks.",
        );
        need(
          natural.some((p) => p.ordnance.byKind[0] > 0) &&
            natural.some((p) => p.ordnance.byKind[1] > 0),
          "Readback did not observe both ranged and siege ballistic bodies.",
        );
        need(
          natural.every((p) =>
            p.probe.sieges.every(
              (s) => s.position.every(Number.isFinite) && s.health > 0,
            ),
          ),
          "A siege actor has invalid state.",
        );
        // Isolate participant setup with a real public battle reset, never actor-state edits.
        crowd.setCount(129);
        await draw(front, 12);
        let sequence = 0;
        let input = {
          active: true,
          firstPerson: true,
          position: [0, 0, 0],
          yaw: Math.PI / 2,
          team: 0,
          attackSequence: sequence,
          attack: "melee",
          aim: [2, 1, 0],
        };
        const lifeFields = [
          "participantAttacks",
          "participantHits",
          "participantKills",
          "participantDamage",
          "participantMeleeHits",
        ];
        const participantCases = [];
        const participantSample = async (name) => {
          const probe = await crowd.probe(),
            raw = await readAll(),
            ordnance = await readOrdnance(probe.gpu.projectilesVisible);
          need(
            probe.finite && probe.participant.position.every(Number.isFinite),
            `${name}: participant pose is invalid.`,
          );
          need(
            probe.gpu.alive + probe.gpu.dead === 129,
            `${name}: participant altered pool population accounting.`,
          );
          need(
            probe.participant.health >= 0 &&
              probe.participant.health <= 200 &&
              probe.participant.dead === (probe.participant.health === 0),
            `${name}: participant health/death disagrees.`,
          );
          need(
            ordnance.active === probe.gpu.projectilesActive,
            `${name}: projectile accounting differs.`,
          );
          need(
            probe.gpu.invalid === 0 &&
              probe.gpu.gridOverflows === 0 &&
              probe.gpu.gridOverflowsTotal === 0 &&
              probe.gpu.projectileOverflows === 0,
            `${name}: GPU resource/state overflow.`,
          );
          need(
            crowd.inspect().frameDraws === 12,
            `${name}: first-person focus was not submitted.`,
          );
          const entry = {
            name,
            input: {
              ...input,
              position: [...input.position],
              aim: [...input.aim],
            },
            probe,
            raw,
            ordnance,
          };
          participantCases.push(entry);
          window.__battleKernelEvidence.participant = participantCases;
          return entry;
        };
        const resetLife = async (name, team, position, yaw, aim) => {
          input = {
            ...input,
            active: true,
            team,
            position: [...position],
            yaw,
            aim: [...aim],
            attackSequence: sequence,
          };
          crowd.setParticipant(input);
          const before = await crowd.probe(),
            epoch = crowd.resetParticipant(),
            pending = await crowd.probe();
          need(
            pending.gpu.participantEpoch < epoch,
            `${name}: CPU reset incorrectly claims a GPU acknowledgement.`,
          );
          await draw(front, 1);
          const ready = await participantSample(name);
          need(
            ready.probe.gpu.participantEpoch === epoch &&
              ready.probe.participant.resetEpoch === epoch,
            `${name}: GPU did not acknowledge this life.`,
          );
          need(
            ready.probe.participant.health === 200 &&
              ready.probe.participant.protectedTicks > 0,
            `${name}: protected life did not start.`,
          );
          need(
            ready.probe.participant.acceptedSequence === sequence,
            `${name}: a held attack could replay after reset.`,
          );
          for (const key of lifeFields)
            need(
              ready.probe.gpu[key] >= before.gpu[key],
              `${name}: ${key} rolled back on respawn.`,
            );
          ready.reset = { epoch, pendingProbe: pending, beforeProbe: before };
          return ready;
        };
        let actors = (await readAll()).actors;
        const meleeVictim = actors
          .filter((a) => a.team === 1 && a.kind === "warrok" && a.health > 600)
          .sort(
            (a, b) =>
              Math.abs(a.position[0]) +
              Math.abs(a.position[2]) -
              Math.abs(b.position[0]) -
              Math.abs(b.position[2]),
          )[0];
        need(
          meleeVictim,
          "No durable living Warrok melee fixture exists in the natural pool.",
        );
        const join = await resetLife(
          "participant-protected-entry",
          0,
          [meleeVictim.position[0] - 2.1, 0, meleeVictim.position[2]],
          Math.PI / 2,
          [...meleeVictim.position],
        );
        join.meleeFixture = {
          victim: meleeVictim,
          participantCoreRadius: 0.4,
          victimCoreRadius: 1.33,
          requiredClearance: 1.33 + 0.4 + 0.1,
          reason:
            "The natural Warrok starts2.1m from the participant, beyond both physical cores and their.1m margin. Source1000HP withstands allied strikes during the.35s windup.",
        };
        const enteredVictim = join.raw.actors.find(
          (a) => a.id === meleeVictim.id,
        );
        need(
          enteredVictim,
          "The natural melee target disappeared during entry.",
        );
        join.meleeFixture.measuredEntryDistance = Math.hypot(
          enteredVictim.position[0] - input.position[0],
          enteredVictim.position[2] - input.position[2],
        );
        need(
          join.meleeFixture.measuredEntryDistance >=
            join.meleeFixture.requiredClearance,
          "The participant melee fixture starts inside the target's physical clearance.",
        );
        input = { ...input, attackSequence: ++sequence, attack: "melee" };
        crowd.setParticipant(input);
        await draw(front, 23);
        const melee = await participantSample("participant-melee-impact");
        need(
          melee.probe.gpu.participantAttacks ===
            join.probe.gpu.participantAttacks + 1,
          "Melee action was not accepted exactly once.",
        );
        need(
          melee.probe.gpu.participantMeleeHits >
            join.probe.gpu.participantMeleeHits &&
            melee.probe.gpu.participantDamage >
              join.probe.gpu.participantDamage,
          "Animation-timed melee did not damage real opponents.",
        );
        need(
          melee.probe.participant.health === 200 &&
            melee.probe.participant.protectedTicks > 0,
          "Protected entry lost health early.",
        );
        const damagedVictim = melee.raw.actors.find(
          (a) => a.id === meleeVictim.id,
        );
        need(
          damagedVictim && damagedVictim.health < meleeVictim.health,
          "The durable melee fixture did not lose real health.",
        );
        melee.meleeFixture = { before: meleeVictim, after: damagedVictim };
        const attacks = melee.probe.gpu.participantAttacks,
          acceptedSequence = sequence;
        input = { ...input, attackSequence: ++sequence, attack: "ranged" };
        crowd.setParticipant(input);
        await draw(front, 1);
        const rejected = await participantSample(
          "participant-cooldown-edge-consumed",
        );
        need(
          rejected.probe.gpu.participantAttacks === attacks &&
            rejected.probe.participant.acceptedSequence === acceptedSequence &&
            rejected.probe.participant.receivedSequence === sequence,
          "An inhibited sequence was not consumed without starting a new action.",
        );
        for (let n = 0; n < 4; n++) {
          crowd.setParticipant({ ...input });
          await draw(front, 25);
        }
        const dedupe = await participantSample(
          "participant-held-sequence-deduped",
        );
        need(
          dedupe.probe.gpu.participantAttacks === attacks &&
            dedupe.probe.participant.acceptedSequence === acceptedSequence &&
            dedupe.probe.participant.receivedSequence === sequence,
          "An inhibited/held attackSequence fired after its original cooldown.",
        );
        let rangedSuccess = false;
        for (let attempt = 0; attempt < 3 && !rangedSuccess; attempt++) {
          let victims = [];
          for (let wait = 0; wait < 12 && !victims.length; wait++) {
            actors = (await readAll()).actors;
            victims = actors.filter(
              (a) =>
                a.team === 1 &&
                a.kind === "warrok" &&
                a.health > 300 &&
                a.state === 2 &&
                a.actionTime < 1.4,
            );
            if (!victims.length) await draw(front, 60);
          }
          need(
            victims.length,
            "No durable naturally attacking Warrok replenished for the swept-firebolt fixture.",
          );
          victims.sort(
            (a, b) => b.health - a.health || a.position[0] - b.position[0],
          );
          const target = victims[0];
          const origin = [target.position[0] - 8, 0, target.position[2]],
            aim = [origin[0] + 90, 1, origin[2]];
          await resetLife(
            `participant-before-swept-ranged-${attempt}`,
            0,
            origin,
            Math.PI / 2,
            aim,
          );
          input = {
            ...input,
            attackSequence: ++sequence,
            attack: "ranged",
            aim,
          };
          crowd.setParticipant(input);
          const before = await crowd.probe();
          await draw(front, 45);
          const launch = await participantSample(
            `participant-ballistic-launch-${attempt}`,
          );
          need(
            launch.probe.gpu.participantAttacks ===
              before.gpu.participantAttacks + 1,
            "Ranged sequence did not start exactly one action.",
          );
          need(
            launch.ordnance.byKind[2] > 0,
            "The local ranged action did not create a real in-flight projectile before the nearer victim.",
          );
          let impact = launch;
          for (let step = 0; step < 20; step++) {
            if (
              impact.probe.participant.lastImpact &&
              impact.probe.participant.lastImpact.tick >= before.tick
            )
              break;
            await draw(front, 6);
            impact = await participantSample(
              `participant-swept-impact-${attempt}-${step}`,
            );
          }
          const hit = impact.probe.participant.lastImpact;
          need(
            hit && hit.tick >= before.tick,
            "The live local firebolt never reported an actual impact.",
          );
          const launchOrigin = [origin[0], 1, origin[2]],
            impactDistance = Math.hypot(
              ...hit.position.map((n, i) => n - launchOrigin[i]),
            );
          const endpoint = [origin[0] + 80, 1, origin[2]],
            endpointDistance = Math.hypot(
              ...hit.position.map((n, i) => n - endpoint[i]),
            );
          impact.sweptFixture = {
            target,
            origin: launchOrigin,
            aim,
            endpoint,
            actualImpact: hit,
            impactDistance,
            endpointDistance,
          };
          rangedSuccess =
            impact.probe.gpu.participantDamage > before.gpu.participantDamage &&
            impactDistance < 20 &&
            endpointDistance > 50;
          if (rangedSuccess) {
            await draw(front, 1);
            const applied = await participantSample(
              `participant-swept-damage-applied-${attempt}`,
            );
            impact.sweptFixture.appliedActors = applied.raw.actors.filter(
              (a) =>
                a.team === 1 &&
                Math.hypot(
                  a.position[0] - hit.position[0],
                  a.position[2] - hit.position[2],
                ) < 4,
            );
          }
        }
        need(
          rangedSuccess,
          "Firebolts aimed90m beyond a near moving opponent did not cause a real early body hit and damage before their80m endpoint.",
        );
        let death = await crowd.probe();
        if (!death.participant.dead) {
          // Join the monster side near a naturally living source Paladin. Its 300 damage
          // can kill the participant after protection; no test mutates crowd health/AI.
          let paladins = [];
          for (let wait = 0; wait < 12 && !paladins.length; wait++) {
            actors = (await readAll()).actors;
            paladins = actors.filter(
              (a) => a.kind === "paladin" && a.health > 0,
            );
            if (!paladins.length) await draw(front, 60);
          }
          need(
            paladins.length,
            "No living Paladin exists for natural incoming damage.",
          );
          paladins.sort((a, b) => a.position[0] - b.position[0]);
          const opponent = paladins[0];
          await resetLife(
            "participant-incoming-damage-entry",
            1,
            [opponent.position[0] - 1.05, 0, opponent.position[2]],
            Math.PI / 2,
            opponent.position,
          );
          for (let chunk = 0; chunk < 12; chunk++) {
            await draw(front, 60);
            const entry = await participantSample(
              `participant-incoming-${chunk}`,
            );
            death = entry.probe;
            if (death.participant.dead) break;
          }
        }
        need(
          death.participant.dead && death.gpu.participantHits > 0,
          "Natural opposing actors did not damage and kill the participant.",
        );
        const deadMeteorBefore = await crowd.probe();
        crowd.setParticipant({
          ...input,
          attack: "meteor_shower",
          meteorTargetValid: true,
          attackSequence: ++sequence,
          aim: [input.position[0] + 5, 0, input.position[2]],
        });
        await draw(front, 2);
        const deadMeteorAfter = await crowd.probe();
        need(
          deadMeteorAfter.gpu.meteorCasts ===
            deadMeteorBefore.gpu.meteorCasts &&
            deadMeteorAfter.gpu.participantAttacks ===
              deadMeteorBefore.gpu.participantAttacks,
          "A dead participant accepted a meteor cast.",
        );
        const deathBefore = death;
        crowd.setParticipant({ ...input });
        await draw(front, 1);
        const stillDead = await participantSample(
          "participant-pose-update-does-not-heal",
        );
        need(
          stillDead.probe.participant.health === 0,
          "A pose update healed the participant.",
        );
        const respawn = await resetLife(
          "participant-epoch-respawn",
          input.team,
          input.position,
          input.yaw,
          input.aim,
        );
        for (const key of lifeFields)
          need(
            respawn.probe.gpu[key] >= deathBefore.gpu[key],
            `${key} did not survive natural death and respawn.`,
          );
        const attacksAfter = respawn.probe.gpu.participantAttacks;
        await draw(front, 30);
        const afterRespawn = await participantSample(
          "participant-respawn-no-replayed-action",
        );
        need(
          afterRespawn.probe.gpu.participantAttacks === attacksAfter,
          "Respawn replayed the previous life’s attack.",
        );
        const meteorEvidence = {
          attempts: [],
          lifecycle: [],
          damageApplications: [],
          checks: {},
        };
        window.__battleKernelEvidence.meteors = meteorEvidence;
        let meteorSuccess = false;
        for (let attempt = 0; attempt < 3 && !meteorSuccess; attempt++) {
          crowd.setCount(129);
          time = 0;
          await draw(front, 12);
          const target = [0, 0, (attempt - 1) * 3];
          const meteorEntry = await resetLife(
            `meteor-life-${attempt}`,
            1,
            [77, 0, target[2]],
            -Math.PI / 2,
            target,
          );
          const nearestOpponent = Math.min(
            ...meteorEntry.raw.actors
              .filter((a) => a.team === 0 && a.health > 0)
              .map((a) =>
                Math.hypot(a.position[0] - 77, a.position[2] - target[2]),
              ),
          );
          need(
            nearestOpponent > 75,
            "The meteor fixture must begin on its own army's side beyond opposing ranged reach.",
          );
          const before = await crowd.probe();
          input = {
            ...input,
            attack: "meteor_shower",
            meteorTargetValid: true,
            attackSequence: ++sequence,
            aim: [...target],
          };
          crowd.setParticipant(input);
          await draw(front, 1);
          const sampleMeteor = async () => {
            const probe = await crowd.probe(),
              raw = await readAll(),
              ordnance = await readOrdnance(probe.gpu.projectilesVisible),
              pending = await readPendingDamage();
            need(
              probe.finite &&
                probe.gpu.invalid === 0 &&
                probe.gpu.gridOverflowsTotal === 0 &&
                probe.gpu.projectileOverflows === 0 &&
                crowd.inspect().droppedSeconds === 0,
              "Meteor case overflowed a bounded resource.",
            );
            need(
              !probe.participant.dead,
              `The natural meteor caster died at tick${probe.tick}; this cannot prove an uninterrupted six-meteor cast.`,
            );
            for (const hit of pending.filter((d) => d.local > 0))
              need(
                raw.actors[hit.id].team !== input.team &&
                  hit.local <= hit.amount,
                "Meteor damage was attributed to a friendly actor or exceeds total queued damage.",
              );
            return { tick: probe.tick, probe, raw, ordnance, pending };
          };
          const first = await sampleMeteor(),
            frames = [first],
            accepted = sequence;
          const attemptEvidence = {
            attempt,
            aim: target,
            entry: input.position,
            nearestOpponent,
            frames,
            metrics: null,
          };
          meteorEvidence.attempts.push(attemptEvidence);
          const rolledDamage = first.ordnance.samples.find(
            (p) => p.kind === 3,
          )?.damage;
          need(
            first.probe.gpu.meteorCasts === before.gpu.meteorCasts + 1 &&
              first.probe.gpu.meteorCooldownTicks === 252 &&
              first.probe.participant.acceptedSequence === accepted &&
              first.probe.participant.attackKind === "meteor_shower",
            "A fresh meteor input did not commit exactly one cast.",
          );
          input = {
            ...input,
            attackSequence: ++sequence,
            aim: [target[0] + 8, 0, target[2]],
          };
          crowd.setParticipant(input);
          let previous = first;
          for (let frame = 0; frame < 260; frame++) {
            await draw(front, 1);
            const current = await sampleMeteor();
            if (
              current.pending.some((d) => d.local > 0) ||
              current.probe.gpu.participantDamage >
                previous.probe.gpu.participantDamage
            )
              need(
                current.probe.gpu.meteorImpacts ===
                  previous.probe.gpu.meteorImpacts + 1,
                "A meteor pulse or scheduled rock dealt damage without a new impact.",
              );
            for (const hit of current.pending.filter((d) => d.local > 0))
              need(
                hit.local === rolledDamage,
                "A meteor actor hit did not use the cast's full shared 40d6 roll.",
              );
            let expectedKills = 0;
            for (const queued of previous.pending.filter((d) => d.local > 0)) {
              const old = previous.raw.actors[queued.id],
                now = current.raw.actors[queued.id],
                expected = Math.max(0, old.health - queued.amount);
              need(
                now.health === expected,
                `Meteor queued damage did not become real actor health: ${queued.id} ${old.health}−${queued.amount} != ${now.health}`,
              );
              if (old.health > 0 && old.health <= queued.amount)
                expectedKills++;
              meteorEvidence.damageApplications.push({
                tick: current.tick,
                id: queued.id,
                team: old.team,
                before: old.health,
                queued: queued.amount,
                local: queued.local,
                after: now.health,
              });
            }
            need(
              current.probe.gpu.participantKills -
                previous.probe.gpu.participantKills ===
                expectedKills,
              "Actual lethal meteor damage did not receive exactly one local kill credit.",
            );
            need(
              current.probe.participant.acceptedSequence === accepted &&
                current.probe.participant.receivedSequence === sequence,
              "An inhibited held meteor input replayed when cooldown ended.",
            );
            frames.push(current);
            previous = current;
          }
          const metrics = evaluateBattleMeteors(frames, {
            epoch: first.probe.participant.resetEpoch,
            castTick: first.tick,
            aim: target,
            castsBefore: before.gpu.meteorCasts,
            impactsBefore: before.gpu.meteorImpacts,
            damageBefore: before.gpu.participantDamage,
            hitsBefore: before.gpu.meteorHits,
            killsBefore: before.gpu.participantKills,
            requireKill: false,
          });
          need(
            frames.at(-1).probe.gpu.meteorCooldownTicks === 0,
            "The held-input check did not run beyond the real meteor cooldown.",
          );
          attemptEvidence.metrics = metrics;
          need(
            metrics.ok,
            `Real meteor timeline failed: ${JSON.stringify(metrics.failures)}`,
          );
          meteorSuccess = metrics.summary.kills > 0;
        }
        need(
          meteorSuccess,
          "Three natural meteor showers caused damage without any actual local kill credit.",
        );
        need(
          meteorEvidence.damageApplications.some(
            (hit) => hit.before > hit.after && hit.local > 0,
          ),
          "No real per-actor meteor damage application was observed.",
        );
        // A fresh public reset cancels scheduled meteors, not already falling rocks.
        crowd.setCount(129);
        time = 0;
        await draw(front, 12);
        await resetLife(
          "meteor-invalid-entry",
          1,
          [77, 0, 0],
          -Math.PI / 2,
          [0, 0, 0],
        );
        const invalidBefore = await crowd.probe();
        input = {
          ...input,
          attack: "meteor_shower",
          meteorTargetValid: false,
          attackSequence: ++sequence,
          aim: [0, 0, 0],
        };
        crowd.setParticipant(input);
        await draw(front, 1);
        let invalidAfter = await crowd.probe();
        need(
          invalidAfter.gpu.meteorCasts === invalidBefore.gpu.meteorCasts &&
            invalidAfter.participant.receivedSequence === sequence,
          "Invalid surface targeting committed a meteor cast or failed to consume its edge.",
        );
        input = { ...input, meteorTargetValid: true };
        crowd.setParticipant(input);
        await draw(front, 2);
        invalidAfter = await crowd.probe();
        need(
          invalidAfter.gpu.meteorCasts === invalidBefore.gpu.meteorCasts,
          "Making a held invalid target valid replayed the rejected edge.",
        );
        input = {
          ...input,
          attackSequence: ++sequence,
          aim: [input.position[0] + 81, 0, input.position[2]],
        };
        crowd.setParticipant(input);
        await draw(front, 1);
        invalidAfter = await crowd.probe();
        need(
          invalidAfter.gpu.meteorCasts === invalidBefore.gpu.meteorCasts &&
            invalidAfter.participant.receivedSequence === sequence,
          "An out-of-range meteor target committed a cast or retained its edge.",
        );
        input = { ...input, aim: [0, 0, 0] };
        crowd.setParticipant(input);
        await draw(front, 1);
        need(
          (await crowd.probe()).gpu.meteorCasts ===
            invalidBefore.gpu.meteorCasts,
          "Moving a rejected held target into range replayed the edge.",
        );
        input = { ...input, attackSequence: ++sequence };
        crowd.setParticipant(input);
        await draw(front, 1);
        const pendingMeteor = await crowd.probe(),
          pendingOrdnance = await readOrdnance(
            pendingMeteor.gpu.projectilesVisible,
          );
        need(
          pendingOrdnance.byKind[3] === 6 &&
            pendingOrdnance.samples
              .filter((p) => p.kind === 3)
              .every((p) => p.age < 0),
          "Pending-reset fixture did not reserve six scheduled meteors.",
        );
        const epoch = crowd.resetParticipant();
        await draw(front, 1);
        let cancelled = await crowd.probe(),
          cancelledOrdnance = await readOrdnance(
            cancelled.gpu.projectilesVisible,
          );
        need(
          cancelled.participant.resetEpoch === epoch &&
            cancelledOrdnance.byKind[3] === 0 &&
            cancelled.gpu.meteorImpacts === 0,
          "A new life retained the old life's scheduled meteors.",
        );
        meteorEvidence.lifecycle.push({
          name: "pending-new-life-cancel",
          before: pendingMeteor,
          after: cancelled,
          ordnance: cancelledOrdnance,
        });
        input = {
          ...input,
          attackSequence: ++sequence,
          aim: [input.position[0] + 15, 0, input.position[2]],
        };
        crowd.setParticipant(input);
        await draw(front, 166);
        const fallingBefore = await crowd.probe(),
          fallingOrdnance = await readOrdnance(
            fallingBefore.gpu.projectilesVisible,
          ),
          falling = fallingOrdnance.samples.filter(
            (p) => p.kind === 3 && p.age >= 0,
          );
        need(
          falling.length > 0 &&
            fallingOrdnance.samples.some((p) => p.kind === 3 && p.age < 0),
          "Lifecycle fixture did not observe both falling and still-scheduled meteors.",
        );
        crowd.resetParticipant();
        await draw(front, 100);
        cancelled = await crowd.probe();
        cancelledOrdnance = await readOrdnance(
          cancelled.gpu.projectilesVisible,
        );
        need(
          cancelled.gpu.meteorImpacts - fallingBefore.gpu.meteorImpacts ===
            falling.length &&
            cancelledOrdnance.byKind[3] === 0 &&
            cancelled.gpu.participantDamage ===
              fallingBefore.gpu.participantDamage,
          "Life reset failed to cancel pending rocks or changed already falling meteor completion.",
        );
        meteorEvidence.lifecycle.push({
          name: "falling-finish-pending-cancel",
          before: fallingBefore,
          fallingIds: falling.map((p) => p.id),
          after: cancelled,
          ordnance: cancelledOrdnance,
        });
        input = { ...input, attackSequence: ++sequence };
        crowd.setParticipant(input);
        await draw(front, 1);
        const leaveBefore = await crowd.probe();
        need(
          (await readOrdnance(leaveBefore.gpu.projectilesVisible)).byKind[3] ===
            6,
          "Leave fixture did not start a pending meteor cast.",
        );
        input = { ...input, active: false };
        crowd.setParticipant(input);
        await draw(front, 255);
        const leaveAfter = await crowd.probe(),
          leaveOrdnance = await readOrdnance(leaveAfter.gpu.projectilesVisible);
        need(
          leaveAfter.gpu.meteorImpacts === leaveBefore.gpu.meteorImpacts &&
            leaveOrdnance.byKind[3] === 0,
          "Leaving the battle allowed scheduled meteors to launch.",
        );
        meteorEvidence.lifecycle.push({
          name: "pending-leave-cancel",
          before: leaveBefore,
          after: leaveAfter,
          ordnance: leaveOrdnance,
        });
        meteorEvidence.checks = {
          sixRealFalls: true,
          sixRealImpacts: true,
          sharedDamageRoll: true,
          noEarlyDamage: true,
          frozenAim: true,
          heldSequenceDedupe: true,
          cooldown: true,
          invalidSurfaceRejected: true,
          friendlyDamageAttribution: true,
          realHealthDamage: true,
          actualKillCredit: true,
          deadCannotCast: true,
          pendingLifeAndLeaveCancel: true,
          airborneFinish: true,
          emptyGroundMiss: true,
        };
        // Actual Rust movement consumes actual asynchronous GPU bodies, not a synthetic obstacle list.
        crowd.setCount(129);
        time = 0;
        await draw(front, 12);
        const wasmResponse = await fetch("/kernel.wasm");
        need(wasmResponse.ok, "The movement kernel could not open.");
        const wasmBytes = await wasmResponse.arrayBuffer();
        const localKernel = await RuntimeKernel.fromBytes(wasmBytes);
        const controller = new BattleParticipant(localKernel);
        const controllerTerrain = {
          layout: "mmorpg-patch",
          resolution: 257,
          size: 3328,
          center: [0, 0],
          heights: new Float32Array(257 ** 2),
        };
        const collision = {
          frames: [],
          lives: [],
          staleLifeChecks: [],
          kernelSha256: [
            ...new Uint8Array(await crypto.subtle.digest("SHA-256", wasmBytes)),
          ]
            .map((n) => n.toString(16).padStart(2, "0"))
            .join(""),
          physicalPlayerRadius: 0.4,
          controllerMargin: 0.12,
        };
        window.__battleKernelEvidence.participantCollision = collision;
        const neutral = {
          forward: 0,
          turn: 0,
          sprint: false,
          attack: false,
          dance: false,
        };
        const radiusOf = (actor) =>
          BATTLE_BODY_RADII[BATTLE_CLASSES.indexOf(actor.kind)];
        const headingTo = (from, target) =>
          Math.atan2(target[0] - from[0], -(target[2] - from[2]));
        const shortest = (angle) =>
          Math.atan2(Math.sin(angle), Math.cos(angle));
        const findPhysicalEntry = (raw, kind, distance) => {
          let best = null;
          const candidates = raw.actors.filter(
            (a) =>
              a.kind === kind &&
              a.health > (kind === "warrok" ? 900 : 0) &&
              (kind !== "warrok" || (a.state === 2 && a.actionTime < 1)),
          );
          for (const target of candidates)
            for (let angle = 0; angle < 32; angle++) {
              const radians = (angle * Math.PI * 2) / 32;
              const position = [
                target.position[0] + Math.cos(radians) * distance,
                0,
                target.position[2] + Math.sin(radians) * distance,
              ];
              let minimumGap = Infinity;
              for (const actor of raw.actors)
                if (actor.health > 0)
                  minimumGap = Math.min(
                    minimumGap,
                    Math.hypot(
                      position[0] - actor.position[0],
                      position[2] - actor.position[2],
                    ) -
                      0.4 -
                      radiusOf(actor),
                  );
              if (minimumGap >= 0.14 && (!best || minimumGap > best.minimumGap))
                best = {
                  target,
                  position,
                  yaw: headingTo(position, target.position),
                  minimumGap,
                  distance,
                };
            }
          return best;
        };
        let expectedEpoch = 0;
        const controllerContactSample = (sample) =>
          sample
            ? {
                epoch: sample.epoch,
                tick: sample.tick,
                position: [...sample.position],
                contacts: sample.contacts.map((body) => ({
                  id: body.id,
                  position: [...body.position],
                  radius: body.radius,
                  velocity: [...body.velocity],
                })),
                overflow: sample.overflow,
              }
            : null;
        const history = new Map();
        let activeTeam = 0;
        const publishController = (aim) => {
          const pose = controller.snapshot;
          input = {
            active: true,
            firstPerson: true,
            position: [...pose.position],
            yaw: pose.yaw,
            team: activeTeam,
            attackSequence: pose.attackSequence,
            attack: "melee",
            aim: [...aim],
          };
          crowd.setParticipant(input);
        };
        const contactFrame = async (label) => {
          const raw = await readAll(),
            probe = await crowd.probe(),
            tick = crowd.inspect().simulationSteps;
          history.set(tick, { raw, participantPosition: [...input.position] });
          while (history.size > 16) history.delete(history.keys().next().value);
          let sample;
          for (let attempt = 0; attempt < 100; attempt++) {
            sample = crowd.latestParticipantContacts();
            if (
              sample &&
              sample.epoch === expectedEpoch &&
              sample.tick <= tick &&
              tick - sample.tick <= 8
            )
              break;
            await new Promise((resolve) => setTimeout(resolve, 2));
          }
          need(
            sample &&
              sample.epoch === expectedEpoch &&
              sample.tick <= tick &&
              tick - sample.tick <= 8,
            `${label}: no fresh epoch-matching GPU body sample.`,
          );
          const now = performance.now();
          need(
            Number.isFinite(sample.sampledAt) &&
              sample.sampledAt >= 0 &&
              sample.sampledAt <= now,
            `${label}: body query timestamp is invalid.`,
          );
          const ageSeconds = (now - sample.sampledAt) / 1000;
          need(
            ageSeconds <= 0.12 &&
              sample.contacts.length <= 64 &&
              sample.overflow === 0,
            `${label}: stale or overflowing body sample.`,
          );
          const source = history.get(sample.tick);
          need(
            source,
            `${label}: the GPU body sample has no matching source readback.`,
          );
          need(
            Math.hypot(
              ...sample.position.map(
                (n, i) => n - source.participantPosition[i],
              ),
            ) < 1e-4,
            `${label}: body-query origin differs from the actual submitted participant.`,
          );
          for (const body of sample.contacts) {
            const actor = source.raw.actors[body.id];
            need(
              actor?.health > 0 &&
                Math.abs(radiusOf(actor) - body.radius) < 1e-5 &&
                Math.hypot(
                  ...body.position.map((n, i) => n - actor.position[i]),
                ) < 1e-4 &&
                Math.hypot(
                  body.velocity[0] - actor.velocity[0],
                  body.velocity[1] - actor.velocity[2],
                ) < 1e-4,
              `${label}: contact ${body.id} is not the exact living GPU body at sample.tick.`,
            );
          }
          const pose = controller.snapshot;
          let minimumGap = Infinity,
            closest;
          for (const actor of raw.actors)
            if (actor.health > 0) {
              const gap =
                Math.hypot(
                  pose.position[0] - actor.position[0],
                  pose.position[2] - actor.position[2],
                ) -
                0.4 -
                radiusOf(actor);
              if (gap < minimumGap) {
                minimumGap = gap;
                closest = actor;
              }
            }
          const frame = {
            label,
            tick,
            pose,
            minimumGap,
            closest,
            sample,
            ageSeconds,
            probe: {
              tick: probe.tick,
              participant: probe.participant,
              gpu: probe.gpu,
            },
            rawActors: raw.actors,
          };
          collision.frames.push(frame);
          need(
            minimumGap >= -0.001,
            `${label}: actual participant overlaps living ${closest?.kind} ${closest?.id} by ${-minimumGap}m.`,
          );
          need(
            probe.gpu.alive + probe.gpu.dead === 129 &&
              probe.gpu.invalid === 0 &&
              probe.gpu.gridOverflowsTotal === 0 &&
              probe.gpu.projectileOverflows === 0,
            `${label}: body collision altered population or bounded combat state.`,
          );
          return frame;
        };
        const beginControlledLife = async (name, fixture, team) => {
          const oldSample = crowd.latestParticipantContacts();
          activeTeam = team;
          if (!controller.snapshot)
            controller.enter(controllerTerrain, {
              class: "paladin",
              position: fixture.position,
              yaw: fixture.yaw,
            });
          else
            controller.respawn({
              position: fixture.position,
              yaw: fixture.yaw,
            });
          publishController(fixture.target.position);
          expectedEpoch = crowd.resetParticipant();
          need(
            crowd.latestParticipantContacts() === null,
            `${name}: reset retained prior-life body samples.`,
          );
          history.clear();
          const before = controller.snapshot;
          const blocked = controller.advance(
            1 / 60,
            { ...neutral, forward: 1 },
            true,
            0.1,
            0,
            {
              expectedEpoch,
              currentTick: crowd.inspect().simulationSteps,
              ageSeconds: oldSample
                ? (performance.now() - oldSample.sampledAt) / 1000
                : 0,
              sample: controllerContactSample(oldSample),
            },
          );
          need(
            blocked.crowdReady === false &&
              blocked.crowdBlocked === true &&
              Math.hypot(
                ...blocked.position.map((n, i) => n - before.position[i]),
              ) < 1e-6 &&
              Math.abs(shortest(blocked.yaw - before.yaw)) > 0.05,
            `${name}: missing/prior-life contacts allowed translation or blocked looking.`,
          );
          collision.staleLifeChecks.push({
            name,
            expectedEpoch,
            oldSampleEpoch: oldSample?.epoch ?? null,
            before,
            after: blocked,
          });
          publishController(fixture.target.position);
          await draw(front, 1);
          const entry = await contactFrame(name);
          need(
            entry.probe.participant.resetEpoch === expectedEpoch &&
              entry.probe.participant.health === 200 &&
              entry.probe.participant.protectedTicks > 0,
            `${name}: no GPU life acknowledgement/protection.`,
          );
          collision.lives.push({
            name,
            fixture,
            expectedEpoch,
            entryTick: entry.tick,
          });
          return entry;
        };
        const controlledStep = async (
          label,
          previous,
          controls,
          aim,
          yawOffset = 0,
        ) => {
          const before = controller.snapshot;
          const yaw = headingTo(before.position, aim) + yawOffset;
          const next = controller.advance(
            1 / 60,
            controls,
            !previous.probe.participant.dead,
            Math.max(-0.3, Math.min(0.3, shortest(yaw - before.yaw))),
            0,
            {
              expectedEpoch,
              currentTick: previous.tick,
              ageSeconds:
                (performance.now() - previous.sample.sampledAt) / 1000,
              sample: controllerContactSample(previous.sample),
            },
          );
          need(
            next.crowdReady === true,
            `${label}: fresh GPU bodies did not enable actual Wasm collision.`,
          );
          publishController(aim);
          await draw(front, 1);
          return contactFrame(label);
        };
        let collisionFixture;
        for (let attempt = 0; attempt < 12 && !collisionFixture; attempt++) {
          collisionFixture = findPhysicalEntry(await readAll(), "warrok", 2.1);
          if (!collisionFixture) await draw(front, 5);
        }
        need(
          collisionFixture,
          "No naturally attacking durable Warrok has a physically clear public entry point.",
        );
        let controlled = await beginControlledLife(
          "wasm-warrok-entry",
          collisionFixture,
          0,
        );
        const entryPose = controlled.pose,
          targetId = collisionFixture.target.id;
        let blockedFrames = 0,
          approached = 0;
        for (let step = 0; step < 60; step++) {
          const target = controlled.rawActors[targetId];
          need(
            target.health > 0,
            "The durable natural Warrok died before the body-approach proof.",
          );
          const before = controlled.pose;
          controlled = await controlledStep(
            `wasm-warrok-forward-${step}`,
            controlled,
            { ...neutral, forward: 1 },
            target.position,
          );
          const moved = Math.hypot(
            controlled.pose.position[0] - before.position[0],
            controlled.pose.position[2] - before.position[2],
          );
          approached += moved;
          if (controlled.pose.crowdBlocked && moved < 0.008) blockedFrames++;
        }
        need(
          approached >= 0.05 && blockedFrames >= 5,
          `Real forward input did not both approach and stop against the Warrok: travel=${approached}, blocked=${blockedFrames}.`,
        );
        const slideStart = controlled.pose.position;
        for (let step = 0; step < 30; step++) {
          const target = controlled.rawActors[targetId];
          need(
            target.health > 0,
            "The durable Warrok died before tangential sliding.",
          );
          controlled = await controlledStep(
            `wasm-warrok-slide-${step}`,
            controlled,
            { ...neutral, forward: 1 },
            target.position,
            Math.PI / 3,
          );
        }
        const slideTravel = Math.hypot(
          controlled.pose.position[0] - slideStart[0],
          controlled.pose.position[2] - slideStart[2],
        );
        need(
          slideTravel >= 0.1,
          "Real forward/look input could not slide along the living Warrok.",
        );
        for (let step = 0; step < 6; step++)
          controlled = await controlledStep(
            `wasm-warrok-face-before-attack-${step}`,
            controlled,
            neutral,
            controlled.rawActors[targetId].position,
          );
        const beforeAttack = controlled;
        for (let step = 0; step < 45; step++) {
          const target = controlled.rawActors[targetId];
          controlled = await controlledStep(
            `wasm-warrok-attack-${step}`,
            controlled,
            { ...neutral, attack: true },
            target.position,
          );
        }
        need(
          controlled.probe.gpu.participantAttacks >
            beforeAttack.probe.gpu.participantAttacks &&
            controlled.probe.gpu.participantDamage >
              beforeAttack.probe.gpu.participantDamage,
          "Collision-safe actual Wasm attack did not reach the real GPU opponent.",
        );
        collision.approach = {
          targetId,
          entryPose,
          travel: approached,
          blockedFrames,
          slideTravel,
          attacks:
            controlled.probe.gpu.participantAttacks -
            beforeAttack.probe.gpu.participantAttacks,
          damage:
            controlled.probe.gpu.participantDamage -
            beforeAttack.probe.gpu.participantDamage,
        };
        let incomingFixture = findPhysicalEntry(
          await readAll(),
          "paladin",
          1.4,
        );
        need(
          incomingFixture,
          "No naturally living opposing Paladin has a clear physical entry for incoming damage.",
        );
        controlled = await beginControlledLife(
          "wasm-opposing-protected-entry",
          incomingFixture,
          1,
        );
        for (
          let step = 0;
          step < 360 && !controlled.probe.participant.dead;
          step++
        )
          controlled = await controlledStep(
            `wasm-incoming-${step}`,
            controlled,
            neutral,
            incomingFixture.target.position,
          );
        need(
          controlled.probe.participant.dead &&
            controlled.probe.gpu.participantHits > 0,
          "Physical body clearance prevented actual incoming damage/death.",
        );
        const deadPosition = controlled.pose.position;
        controlled = await controlledStep(
          "wasm-dead-input-is-still",
          controlled,
          { ...neutral, forward: 1, attack: true },
          incomingFixture.target.position,
        );
        need(
          controlled.pose.state === "death" &&
            !controlled.pose.alive &&
            Math.hypot(
              ...controlled.pose.position.map((n, i) => n - deadPosition[i]),
            ) < 1e-6,
          "Actual GPU death did not stop real Wasm movement and actions.",
        );
        const respawnFixture = findPhysicalEntry(
          await readAll(),
          "paladin",
          1.4,
        );
        need(
          respawnFixture,
          "No live physical entry remains for the final body-safe respawn.",
        );
        controlled = await beginControlledLife(
          "wasm-contact-epoch-respawn",
          respawnFixture,
          1,
        );
        need(
          controlled.probe.participant.health === 200 &&
            controlled.sample.epoch === expectedEpoch,
          "Respawn did not restore GPU health and a distinct matching body epoch.",
        );
        collision.checks = {
          livingBodyApproach: true,
          tangentialSlide: true,
          actualWasmAttack: true,
          liveGpuContacts: true,
          boundedFreshContacts: true,
          staleLifeTranslationBlocked: true,
          actualIncomingDeath: true,
          deathStopsMovement: true,
          freshRespawn: true,
        };
        collision.minimumLivingCoreGap = Math.min(
          ...collision.frames.map((frame) => frame.minimumGap),
        );
        controller.clear();
        // New public seed, without test writes to actor health, positions, target IDs or clocks.
        crowd.dispose();
        crowd = await createCrowd(256);
        time = 0;
        const motion = [];
        window.__battleKernelEvidence.motion = motion;
        const motionWindow = async (name) => {
          const frames = [];
          const evidence = { name, frames };
          motion.push(evidence);
          for (let index = 0; index <= 60; index++) {
            if (index) await draw(front, 1);
            const state = await readAll(256);
            frames.push({
              tick: crowd.inspect().simulationSteps,
              actors: state.actors,
              stateSha256: state.stateSha256,
            });
          }
          evidence.metrics = evaluateBattleMotion(frames, {
            radii: Object.fromEntries(
              BATTLE_CLASSES.map((kind, i) => [kind, BATTLE_BODY_RADII[i]]),
            ),
            speeds: {
              paladin: 3.4,
              sorceror: 3.4,
              zombie: 1.666667,
              warrok: 1.666667,
            },
            penetrationTolerance: 0.05,
            turnRate: 4,
            pushSpeed: 0.6,
            targetRatio: 0.75,
            targetDistanceTolerance: 0.15,
          });
          evidence.probe = await crowd.probe();
          need(
            evidence.metrics.ok,
            `${name}: actual GPU motion check failed: ${JSON.stringify(evidence.metrics.failures)}`,
          );
          need(
            evidence.probe.finite &&
              evidence.probe.gpu.alive + evidence.probe.gpu.dead === 256 &&
              evidence.probe.gpu.invalid === 0 &&
              evidence.probe.gpu.gridOverflowsTotal === 0 &&
              evidence.probe.gpu.projectileOverflows === 0 &&
              crowd.inspect().droppedSeconds === 0,
            `${name}: motion case lost bounded state, population, or simulation time.`,
          );
          return evidence;
        };
        await draw(front, 1);
        const formationMotion = await motionWindow(
          "mixed-256-formation-consecutive-motion",
        );
        await draw(front, 120);
        const contactMotion = await motionWindow(
          "mixed-256-natural-contact-consecutive-motion",
        );
        const coverage = {
          movingIntervals: motion.reduce(
            (n, sample) => n + sample.metrics.summary.movingIntervals,
            0,
          ),
          clockIntervals: motion.reduce(
            (n, sample) => n + sample.metrics.summary.clockIntervals,
            0,
          ),
          gaitIntervals: motion.reduce(
            (n, sample) => n + sample.metrics.summary.gaitIntervals,
            0,
          ),
          attackIntervals: motion.reduce(
            (n, sample) => n + sample.metrics.summary.attackIntervals,
            0,
          ),
          retainedTargets: motion.reduce(
            (n, sample) => n + sample.metrics.summary.targetRetainedIntervals,
            0,
          ),
          contactPairSamples: motion.reduce(
            (n, sample) => n + sample.metrics.summary.contactPairs,
            0,
          ),
          naturalTightContactPairSamples:
            contactMotion.metrics.summary.tightContactPairs,
          classes: [
            ...new Set(
              formationMotion.frames[0].actors.map((actor) => actor.kind),
            ),
          ],
          teams: [
            ...new Set(
              formationMotion.frames[0].actors.map((actor) => actor.team),
            ),
          ],
        };
        window.__battleKernelEvidence.motionCoverage = coverage;
        need(
          coverage.classes.length === 4 &&
            coverage.teams.length === 2 &&
            coverage.movingIntervals >= 100 &&
            coverage.clockIntervals >= 100 &&
            coverage.gaitIntervals >= 100 &&
            coverage.attackIntervals >= 10 &&
            coverage.retainedTargets >= 100 &&
            coverage.contactPairSamples >= 100 &&
            coverage.naturalTightContactPairSamples >= 10,
          `Motion checks lacked actual movement/contact/clock/target coverage: ${JSON.stringify(coverage)}`,
        );
        // At radius 100, the deepest melee ranks stay behind the legitimate 72m ranged stop line.
        crowd.dispose();
        crowd = await createCrowd(10_000);
        time = 0;
        const convoy = {
          name: "rear-10000-convoy-five-seconds",
          checkpoints: [],
        };
        window.__battleKernelEvidence.convoy = convoy;
        await draw(front, 1);
        for (let second = 0; second <= 5; second++) {
          if (second) await draw(front, 60);
          const raw = await readAll(10_000);
          convoy.checkpoints.push({
            tick: crowd.inspect().simulationSteps,
            actors: raw.actors,
            stateSha256: raw.stateSha256,
          });
        }
        convoy.metrics = evaluateBattleConvoy(convoy.checkpoints, {
          radius: crowd.inspect().radius,
          minimumEligible: 16,
          minimumPassingFraction: 0.5,
          minimumNetForward: 4,
          minimumLateForward: 0.5,
        });
        convoy.probe = await crowd.probe();
        need(
          convoy.metrics.ok,
          `Rear convoy stalled: ${JSON.stringify({ summary: convoy.metrics.summary, failures: convoy.metrics.failures })}`,
        );
        need(
          convoy.probe.finite &&
            convoy.probe.gpu.alive + convoy.probe.gpu.dead === 10_000 &&
            convoy.probe.gpu.invalid === 0 &&
            convoy.probe.gpu.gridOverflowsTotal === 0 &&
            convoy.probe.gpu.projectileOverflows === 0 &&
            crowd.inspect().droppedSeconds === 0,
          "Rear convoy lost bounded state, population, or simulation time.",
        );
        need(gpuErrors.length === 0, gpuErrors.join("\n"));
        return {
          natural,
          participantCases,
          participantCollision: collision,
          meteors: meteorEvidence,
          motion,
          motionCoverage: coverage,
          convoy,
          checks: {
            tail: true,
            allCulled: true,
            reset: true,
            ballistics: true,
            bothSiegeTeams: true,
            meleeDamage: true,
            rangedDamage: true,
            sequenceDedupe: true,
            naturalPlayerDeath: true,
            epochRespawn: true,
            lifetimeCounters: true,
            boundedConsecutiveMotion: true,
            livingCoreClearance: true,
            clockAndTargetContinuity: true,
            rearConvoyProgress: true,
            actualWasmParticipantBodyCollision: true,
            actualMeteorShower: true,
          },
          adapter: {
            vendor: adapter.info.vendor,
            architecture: adapter.info.architecture,
            description: adapter.info.description,
          },
          loadedBytes: loaded.bytes,
          lods,
          resetState,
          cases,
          gpuErrors,
        };
      } finally {
        cleanup();
      }
    }),
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(Error("The bounded GPU regression exceeded 180 seconds.")),
        180000,
      );
    }),
  ]);
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(
    "PASS actual GPU 129-actor combat and 256-actor consecutive motion, contact, clock, and target checks",
  );
} catch (error) {
  report.partialResult = await page
    .evaluate(() => window.__battleKernelEvidence ?? null)
    .catch(() => null);
  report.errors.push({ error: String(error), stack: error.stack });
  process.exitCode = 1;
  console.error(error);
} finally {
  clearTimeout(timer);
  await page.evaluate(() => window.__battleKernelCleanup?.()).catch(() => {});
  closing = true;
  await context.close().catch(() => {});
  await browser.close();
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-kernel-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
