import assert from "node:assert/strict";

export const ENEMY_WORKLOADS = Object.freeze([
  "enemies",
  "enemy-lightning",
  "enemy-meteor",
  "enemy-thunder",
  "enemy-miss-cover",
]);
export const AIM_WORKLOADS = Object.freeze([
  "aim-third-locked",
  "aim-third-cursor",
  "aim-first-locked",
  "aim-first-cursor",
  "aim-sky",
]);
export const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));

/** Accumulated path length also detects motion when a patrol returns to its start. */
export function enemyMotion(samples) {
  assert(samples.length >= 2, "Record at least two native patrol snapshots.");
  const first = samples[0].enemies;
  assert(first?.actors?.length > 0 && first.total === first.actors.length);
  const paths = new Map(first.actors.map((actor) => [actor.id, 0]));
  for (let i = 1; i < samples.length; i++) {
    const previous = new Map(
      samples[i - 1].enemies.actors.map((a) => [a.id, a]),
    );
    const current = samples[i].enemies;
    assert.equal(current.total, first.total);
    assert.equal(current.actors.length, first.total);
    assert.equal(new Set(current.actors.map((a) => a.id)).size, first.total);
    for (const actor of current.actors) {
      assert(
        previous.has(actor.id) && paths.has(actor.id),
        "Patrol topology changed.",
      );
      assert(actor.alive, "A patrol-only window defeated an actor.");
      paths.set(
        actor.id,
        paths.get(actor.id) +
          distance(actor.position, previous.get(actor.id).position),
      );
    }
  }
  return [...paths].map(([id, pathMeters]) => ({ id, pathMeters }));
}

export function assertHeldCorpses(before, after, elapsedSeconds = 1) {
  const dead = before.actors.filter((actor) => !actor.alive);
  assert(dead.length, "The real spell did not defeat a knight.");
  for (const old of dead) {
    const now = after.actors.find((actor) => actor.id === old.id);
    assert(now && !now.alive, "A defeated knight revived without restore.");
    assert(
      distance(now.position, old.position) < 0.001,
      "A defeated knight kept moving.",
    );
    assert(
      now.deathSeconds > old.deathSeconds + elapsedSeconds,
      "The native death clock did not advance.",
    );
  }
  return dead.map((actor) => actor.id);
}

/** Independent pinhole projection, shared with the real-input targeting verifier. */
export function projectWorldTarget({
  eye,
  target,
  point,
  width,
  height,
  fov = 1,
  viewEye = eye,
}) {
  const subtract = (a, b) => a.map((v, i) => v - b[i]);
  const normalize = (v) => {
    const length = Math.hypot(...v);
    assert(length > 0);
    return v.map((x) => x / length);
  };
  const cross = (a, b) => [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
  const dot = (a, b) => a.reduce((sum, v, i) => sum + v * b[i], 0);
  const forward = normalize(subtract(target, viewEye));
  const right = normalize(cross(forward, [0, 1, 0]));
  const up = normalize(cross(right, forward));
  const delta = subtract(point, eye),
    depth = dot(delta, forward);
  assert(depth > 0, "The marker is behind the rendered camera.");
  assert(width > 0 && height > 0 && fov > 0 && fov < Math.PI);
  return [
    dot(delta, right) / ((depth * Math.tan(fov / 2) * width) / height),
    dot(delta, up) / (depth * Math.tan(fov / 2)),
  ];
}

/** Real native actions; the adapter supplies only existing host commands and observation. */
export async function runEnemyWorkload(adapter, feature, durationMs) {
  assert(ENEMY_WORKLOADS.includes(feature));
  const started = performance.now(),
    deadline = started + durationMs;
  const phases = [],
    samples = [],
    attempts = [];
  const state = async () => {
    const value = await adapter.state();
    assert(
      Number.isInteger(value.revision) && value.revision >= 0,
      "Observe the native authored revision.",
    );
    assert.equal(
      value.enemies?.total,
      8,
      "The tested Town requires all eight native knights.",
    );
    assert.equal(value.enemies.actors.length, 8);
    assert.equal(new Set(value.enemies.actors.map((a) => a.id)).size, 8);
    assert.equal(value.enemies.alive + value.enemies.killed, 8);
    return value;
  };
  const mark = async (phase) => {
    const value = await state();
    phases.push({ phase, atMs: performance.now() - started, ...value });
    return value;
  };
  const sampleUntil = async (end) => {
    while (performance.now() < end) {
      const value = await state();
      samples.push({
        atMs: performance.now() - started,
        enemies: value.enemies,
      });
      await adapter.wait(Math.min(250, Math.max(0, end - performance.now())));
    }
  };
  const waitFor = async (test, timeout, label) => {
    const end = performance.now() + timeout;
    let last;
    do {
      const value = last = await state();
      if (await test(value)) return value;
      await adapter.wait(80);
    } while (performance.now() < end);
    throw Error(`${label} Last native phase: ${JSON.stringify({
      gameplay: last?.gameplay,
      killed: last?.enemies.killed,
    })}`);
  };
  await mark("restore-requested");
  await adapter.restore();
  await adapter.point([6, 2, 26.5], [6, 3.37, 20]);
  const initial = await mark("all-eight-alive");
  assert.equal(initial.enemies.alive, 8);
  assert.equal(initial.enemies.killed, 0);
  if (feature === "enemies") {
    samples.push({
      atMs: performance.now() - started,
      enemies: initial.enemies,
    });
    await sampleUntil(deadline);
    const motion = enemyMotion(samples);
    assert(
      motion.filter((value) => value.pathMeters > 0.5).length >= 6,
      "Six native patrols must move during this window.",
    );
    const final = await mark("patrol-window-complete");
    assert.equal(final.revision, initial.revision);
    return {
      feature,
      phases,
      samples,
      motion,
      attempts,
      samplingHz: 4,
      observation:
        "Native inspection polling is included in measured host work.",
    };
  }
  if (feature === "enemy-miss-cover") {
    await adapter.point([100, 150, 100], [100, 50, 100]);
    await adapter.cast("lightning_bolt", false);
    const miss = await mark("sky-miss-rejected");
    assert.equal(miss.enemies.killed, 0);
    assert.equal(miss.gameplay.castCount, initial.gameplay.castCount);
    await adapter.restore();
    await adapter.point([6.5, 1.9, 26.25], [6, 1.87, 35]);
    await adapter.cast("lightning_bolt", true);
    await adapter.wait(1000);
    const covered = await mark("intact-arch-blocks-lightning");
    assert.equal(covered.enemies.killed, 0);
  } else {
    const ability = {
      "enemy-lightning": "lightning_bolt",
      "enemy-meteor": "meteor_shower",
      "enemy-thunder": "thunderbolt",
    }[feature];
    let defeated;
    for (let attempt = 0; attempt < 5 && !defeated; attempt++) {
      const live = await state();
      const actor =
        live.enemies.actors.find((a) => a.id === 6 && a.alive) ??
        live.enemies.actors.find((a) => a.id === 7 && a.alive);
      assert(actor, "The exposed source patrols are missing.");
      const target =
        ability === "meteor_shower"
          ? [6.8, 1, 26.5]
          : [actor.position[0], actor.position[1] + 0.9, actor.position[2]];
      await adapter.point(target, [6, 3.37, 20]);
      await mark(`cast-${ability}-${attempt}-requested`);
      try {
        const receipt = await adapter.cast(ability, true);
        attempts.push({
          attempt,
          actorId: actor.id,
          target,
          ok: true,
          receipt,
        });
      } catch (error) {
        attempts.push({
          attempt,
          actorId: actor.id,
          target,
          ok: false,
          error: String(error),
        });
        continue;
      }
      try {
        defeated = await waitFor(
          (value) => value.enemies.killed > 0,
          ability === "meteor_shower"
            ? 12000
            : ability === "thunderbolt"
              ? 3500
              : 650,
          "The spell did not strike a live knight.",
        );
      } catch (error) {
        attempts.at(-1).outcome = String(error);
      }
    }
    assert(
      defeated?.enemies.killed > 0,
      `${ability} never defeated a live source knight.`,
    );
    await mark("death-animation-started");
    await adapter.wait(2200);
    const held = await mark("corpse-held-after-death-clip");
    const corpseIds = assertHeldCorpses(defeated.enemies, held.enemies);
    assert.equal(held.revision, initial.revision);
    if (adapter.rejectShader) {
      await waitFor(
        (value) =>
          !value.gameplay.pendingImpacts &&
          !value.gameplay.activeCast &&
          value.gameplay.effects === 0 &&
          value.gameplay.debris === 0,
        // Native debris lives for 20 seconds; the last meteor can land several
        // seconds after its cast. Wait for the actual settled state.
        35000,
        "Live spell impacts did not finish before shader preservation was checked.",
      );
      const settled = await mark("settled-corpses-before-failed-shader");
      const receipt = await adapter.rejectShader();
      const retained = await mark("failed-shader-retains-corpses");
      assert.equal(retained.enemies.killed, settled.enemies.killed);
      assert.equal(retained.shaderIdentity, settled.shaderIdentity);
      assert.equal(retained.revision, settled.revision);
      assert(distance(retained.player, settled.player) < 0.1);
      attempts.push({ type: "rejected-shader", receipt, corpseIds });
    }
  }
  await sampleUntil(Math.max(performance.now(), deadline - 800));
  await mark("restore-after-combat-requested");
  await adapter.restore();
  const restored = await mark("all-eight-revived-and-damage-cleared");
  assert.equal(restored.enemies.alive, 8);
  assert.equal(restored.enemies.killed, 0);
  assert.equal(restored.gameplay.destroyedCount, 0);
  assert.equal(restored.revision, initial.revision);
  await adapter.wait(Math.max(0, deadline - performance.now()));
  return {
    feature,
    phases,
    attempts,
    samples,
    samplingHz: 4,
    observation:
      "Actions, asynchronous preparation, death, failed edit, corpse hold, restore and polling occur inside this frame window.",
  };
}

/** Re-evaluate native workload outcomes when indexing a saved matrix report. */
export function gameplayActivityComplete(
  activity,
  feature,
  { editor = false } = {},
) {
  try {
    assert.equal(activity?.feature, feature);
    assert(Array.isArray(activity.phases) && activity.phases.length >= 2);
    assert(
      activity.phases.every(
        (phase, i, phases) =>
          Number.isFinite(phase.atMs) &&
          phase.atMs >= 0 &&
          (!i || phase.atMs >= phases[i - 1].atMs),
      ),
    );
    const phase = (name) => {
      const value = activity.phases.find((value) => value.phase === name);
      assert(value, `Missing actual workload phase: ${name}`);
      return value;
    };
    if (ENEMY_WORKLOADS.includes(feature)) {
      const initial = phase("all-eight-alive");
      assert(Number.isInteger(initial.revision) && initial.revision >= 0);
      assert.equal(initial.renderer?.modelLoaded, true);
      assert.equal(initial.renderer?.submittedActors, 8);
      assert.equal(initial.enemies.total, 8);
      assert.equal(initial.enemies.alive, 8);
      assert.equal(initial.enemies.killed, 0);
      if (feature === "enemies") {
        assert(
          enemyMotion(activity.samples).filter(
            (value) => value.pathMeters > 0.5,
          ).length >= 6,
        );
        assert.equal(
          phase("patrol-window-complete").revision,
          initial.revision,
        );
      } else {
        if (feature === "enemy-miss-cover") {
          const miss = phase("sky-miss-rejected"),
            cover = phase("intact-arch-blocks-lightning");
          assert.equal(miss.enemies.killed, 0);
          assert.equal(miss.gameplay.castCount, initial.gameplay.castCount);
          assert.equal(cover.enemies.killed, 0);
          assert(cover.gameplay.castCount > initial.gameplay.castCount);
        } else {
          const death = phase("death-animation-started"),
            corpse = phase("corpse-held-after-death-clip");
          assert(death.enemies.killed > 0);
          assertHeldCorpses(death.enemies, corpse.enemies);
          assert.equal(corpse.revision, initial.revision);
          assert(activity.attempts.some((value) => value.ok === true));
          if (editor) {
            const settled = phase("settled-corpses-before-failed-shader"),
              rejected = phase("failed-shader-retains-corpses");
            assert(settled.enemies.killed >= corpse.enemies.killed);
            assert.equal(rejected.enemies.killed, settled.enemies.killed);
            assert.equal(rejected.shaderIdentity, settled.shaderIdentity);
            assert.equal(rejected.revision, settled.revision);
            assert(distance(rejected.player, settled.player) < 0.1);
          }
        }
        const restored = phase("all-eight-revived-and-damage-cleared");
        assert.equal(restored.enemies.alive, 8);
        assert.equal(restored.enemies.killed, 0);
        assert.equal(restored.gameplay.destroyedCount, 0);
        assert.equal(restored.revision, initial.revision);
      }
    } else if (AIM_WORKLOADS.includes(feature)) {
      const before = phase("real-input-aim-selected");
      phase("cast-through-real-control-requested");
      if (feature === "aim-sky") {
        const miss = phase("sky-cast-rejected");
        assert(!miss.target && miss.aimTriangles === 0);
        assert.equal(miss.casts, before.casts);
      } else {
        const hit = phase("impact-matches-world-ring");
        assert.equal(hit.casts, before.casts + 1);
        assert(hit.impacts > before.impacts);
        assert.equal(hit.locked, feature.endsWith("locked"));
        const expected =
          feature === "aim-third-locked"
            ? [0, 0.22]
            : feature === "aim-third-cursor"
              ? [0.2, 0.04]
              : [0, 0];
        assert(
          activity.samples.some(
            (value) =>
              value.aimTriangles === 128 &&
              value.actualNDC?.length === 2 &&
              value.actualNDC.every(
                (v, i) =>
                  Number.isFinite(v) && Math.abs(v - expected[i]) < 0.025,
              ),
          ),
        );
      }
      phase("restored-after-real-aiming");
    } else return false;
    return true;
  } catch {
    return false;
  }
}
