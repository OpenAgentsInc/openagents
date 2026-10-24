import assert from "node:assert/strict";
import {
  AIM_WORKLOADS,
  distance,
  projectWorldTarget,
  runEnemyWorkload,
} from "./gameplay-workloads.mjs";

export function editorEnemyAdapter({
  page,
  invoke,
  receipt,
  inspect,
  snapshot,
}) {
  return {
    wait: (ms) => page.waitForTimeout(ms),
    state: async () => {
      const [native, current] = await Promise.all([inspect(), snapshot()]);
      const g = current.status.gameplay;
      return {
        enemies: native.enemies,
        revision: native.context.revision,
        shaderIdentity: native.context.shaderIdentity,
        player: native.player.position,
        gameplay: {
          castCount: g.casts,
          destroyedCount: g.destroyed,
          effects: g.effects,
          debris: g.debris,
          pendingImpacts: g.pendingImpacts,
          activeCast: Boolean(g.casting),
        },
        renderer: {
          modelLoaded: g.enemies?.modelLoaded,
          submittedActors: g.enemies?.rendered,
        },
      };
    },
    point: async (target, eye) => {
      const frame = await (
        await page.locator("#preview-frame").elementHandle()
      ).contentFrame();
      assert(frame);
      await frame.evaluate(() => document.exitPointerLock());
      await receipt(() => invoke("selectAbility", null));
      await receipt(() => invoke("cameraPose", eye, target, 1, "fly"));
      const canvas = frame.locator("#world");
      await canvas.focus();
      const box = await canvas.boundingBox();
      assert(box);
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    },
    restore: async () => {
      await receipt(() => invoke("selectAbility", null));
      await receipt(() => invoke("resetGameplay"));
    },
    cast: async (ability, expected = true) => {
      await receipt(() => invoke("selectAbility", ability));
      return receipt(() => page.keyboard.press("Enter"), expected);
    },
    rejectShader: () =>
      receipt(
        () => invoke("shader", "this is deliberately invalid WGSL;"),
        false,
      ),
  };
}

/** Uses actual focus, number keys, V, pointer lock, cursor movement and casting. */
export async function runEditorAimWorkload({
  page,
  invoke,
  receipt,
  snapshot,
  until,
  probe,
  feature,
  durationMs,
}) {
  assert(AIM_WORKLOADS.includes(feature));
  assert(
    probe && Array.isArray(probe.eye) && Array.isArray(probe.target),
    "Aiming needs the source-bound Lightning camera probe.",
  );
  const started = performance.now(),
    deadline = started + durationMs;
  const phases = [],
    samples = [];
  const status = async () => (await snapshot()).status;
  const frame = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  assert(frame);
  const canvas = frame.locator("#world");
  const locked = () =>
    frame.evaluate(
      () => document.pointerLockElement === document.querySelector("#world"),
    );
  const unlock = async () => {
    await frame.evaluate(() => document.exitPointerLock());
    await until(async () => !(await locked()), "Pointer lock did not exit.");
  };
  const pointer = async (x, y) => {
    const b = await canvas.boundingBox();
    assert(b);
    await page.mouse.move(b.x + b.width * x, b.y + b.height * y);
  };
  const mark = async (phase) => {
    const s = await status();
    phases.push({
      phase,
      atMs: performance.now() - started,
      frame: s.frame,
      revision: s.revision,
      player: s.character?.position,
      selected: s.gameplay.selected,
      target: s.gameplay.target,
      aimTriangles: s.gameplay.rendered.aimTriangles,
      casts: s.gameplay.casts,
      impacts: s.gameplay.impacts,
      lastImpact: s.gameplay.lastImpact,
      locked: await locked(),
      perspective: s.character?.perspective,
    });
    return s;
  };
  const select = async (ability, key) => {
    await canvas.focus();
    if ((await status()).gameplay.selected !== ability)
      await receipt(() => page.keyboard.press(`Digit${key}`));
    await until(
      async () => (await status()).gameplay.selected === ability,
      "The real number key did not select its spell.",
    );
  };
  try {
    await unlock();
    await receipt(() => invoke("selectAbility", null));
    await receipt(() =>
      invoke("cameraPose", [6, 3.37, 20], [6, 2, 26.5], 1, "fly"),
    );
    await receipt(() => invoke("resetGameplay"));
    await receipt(() =>
      invoke(
        "cameraPose",
        feature === "aim-sky" ? [100, 50, 100] : probe.eye,
        feature === "aim-sky" ? [100, 150, 90] : probe.target,
        1,
        feature === "aim-sky" ? "fly" : "walk",
      ),
    );
    if (feature !== "aim-sky")
      await until(async () => {
        const s = await status();
        return (
          s.character?.grounded && Math.abs(s.character.velocity[1]) < 0.02
        );
      }, "The native walk camera did not settle for real aiming.");
    const perspective = feature.includes("third") ? "third" : "first";
    if ((await status()).character.perspective !== perspective) {
      await canvas.focus();
      await receipt(() => page.keyboard.press("KeyV"));
    }
    await until(
      async () => (await status()).character.perspective === perspective,
      "The V key did not change perspective.",
    );
    await page.waitForTimeout(400);
    const wantsLocked = feature.endsWith("locked");
    if (wantsLocked) {
      const b = await canvas.boundingBox();
      assert(b);
      await canvas.click({ position: { x: b.width / 2, y: b.height / 2 } });
      await until(
        locked,
        "The real canvas click did not acquire pointer lock.",
      );
    } else
      await pointer(
        feature === "aim-third-cursor" ? 0.6 : 0.5,
        feature === "aim-third-cursor" ? 0.48 : 0.5,
      );
    const [ability, key, scatter] =
      feature === "aim-third-cursor"
        ? ["lightning_bolt", "2", 0]
        : feature === "aim-first-locked"
          ? ["thunderbolt", "3", 0]
          : ["meteor_shower", "1", 6.1];
    await select(ability, key);
    const before = await mark("real-input-aim-selected");
    const projected = async () => {
      const s = await status();
      if (feature === "aim-sky") {
        assert(!s.gameplay.target);
        assert.equal(s.gameplay.rendered.aimTriangles, 0);
        return s;
      }
      assert(s.gameplay.target, "The selected spell lost its native target.");
      assert.equal(
        s.gameplay.rendered.aimTriangles,
        128,
        "The world target ring was not submitted.",
      );
      const box = await canvas.boundingBox();
      assert(box);
      const actual = projectWorldTarget({
        eye: s.camera,
        viewEye: s.view.eye,
        target: s.view.target,
        point: s.gameplay.target.point,
        width: box.width,
        height: box.height,
      });
      const expected =
        feature === "aim-third-locked"
          ? [0, 0.22]
          : feature === "aim-third-cursor"
            ? [0.2, 0.04]
            : [0, 0];
      assert(
        actual.every((v, i) => Math.abs(v - expected[i]) < 0.025),
        `The world target misses the real cursor: ${actual} / ${expected}`,
      );
      samples.push({
        atMs: performance.now() - started,
        frame: s.frame,
        actualNDC: actual,
        expectedNDC: expected,
        target: s.gameplay.target,
        camera: s.camera,
        aimTriangles: s.gameplay.rendered.aimTriangles,
      });
      return s;
    };
    await until(async () => {
      try {
        return await projected();
      } catch {
        return false;
      }
    }, "The actual camera/cursor and native target ring did not agree.");
    const aimed = await projected();
    await mark("cast-through-real-control-requested");
    if (feature === "aim-third-cursor") {
      const b = await canvas.boundingBox();
      await receipt(() =>
        canvas.click({ position: { x: b.width * 0.6, y: b.height * 0.48 } }),
      );
    } else
      await receipt(() => page.keyboard.press("Enter"), feature !== "aim-sky");
    if (feature === "aim-sky") {
      assert.equal((await status()).gameplay.casts, before.gameplay.casts);
      await mark("sky-cast-rejected");
    } else {
      const hit = await until(
        async () => {
          const s = await status();
          return (
            s.gameplay.casts === aimed.gameplay.casts + 1 &&
            s.gameplay.impacts > aimed.gameplay.impacts &&
            s.gameplay.lastImpact &&
            s
          );
        },
        "The cast did not reach the displayed target.",
        25000,
      );
      assert(
        distance(aimed.gameplay.target.point, hit.gameplay.lastImpact.point) <
          scatter + 0.08,
        "The spell missed the real displayed ring.",
      );
      assert.equal(
        await locked(),
        wantsLocked,
        "Casting changed pointer lock.",
      );
      assert.equal(hit.revision, aimed.revision);
      assert(
        distance(hit.character.position, aimed.character.position) < 0.15,
        "Casting moved the player without input.",
      );
      await mark("impact-matches-world-ring");
    }
    // The selected ring stays active during steady-state frames after the real cast.
    while (performance.now() < deadline - 800) {
      // Destruction can change the target after impact; retain native/render counts without
      // pretending the old surface still exists at the old pixel.
      const s = await status();
      samples.push({
        atMs: performance.now() - started,
        frame: s.frame,
        target: s.gameplay.target,
        aimTriangles: s.gameplay.rendered.aimTriangles,
        casts: s.gameplay.casts,
        impacts: s.gameplay.impacts,
      });
      await page.waitForTimeout(
        Math.min(250, Math.max(0, deadline - 800 - performance.now())),
      );
    }
    await mark("restore-requested");
    await unlock();
    await receipt(() => invoke("selectAbility", null));
    await receipt(() => invoke("resetGameplay"));
    await mark("restored-after-real-aiming");
    await page.waitForTimeout(Math.max(0, deadline - performance.now()));
    return {
      feature,
      phases,
      samples,
      controls:
        "Real V/number keys, pointer lock/cursor and canvas click/Enter",
      samplingHz: 4,
    };
  } finally {
    await unlock();
    await receipt(() => invoke("selectAbility", null));
  }
}

export { runEnemyWorkload };
