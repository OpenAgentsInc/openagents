import assert from "node:assert/strict";
import test from "node:test";
import {
  fogCardPhase,
  fogCardSample,
  fogCardEdgeFactor,
  validateFogCards,
  type SourceFogCardsManifest,
} from "../packages/renderer/source-fog-cards.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
import {
  fogCardFixture,
  fogManifestFixture,
} from "./fixtures/source-fog-cards.ts";

test("source cloud density affects emission independently of opacity", () => {
  const c = fogCardFixture(),
    dark = fogCardSample(c, 0, 1, 50, 100, 1),
    bright = fogCardSample(c, 1, 1, 50, 100, 1),
    half = fogCardSample({ ...c, intensity: 0.5 }, 1, 0.5, 50, 100, 1);
  assert.equal(dark.alpha, 1 - 1e-4 ** c.fresnelExponent);
  assert.deepEqual(dark.emission, [0, 0, 0]);
  assert.equal(bright.alpha, dark.alpha);
  assert.deepEqual(bright.emission, [25, 12.5, 6.25]);
  assert.deepEqual(half.emission, [3.125, 1.5625, 0.78125]);
  assert.equal(half.alpha, bright.alpha);
});
test("near fade, opaque intersections and view angle retain separate source factors", () => {
  const c = fogCardFixture();
  assert.equal(fogCardSample(c, 1, 1, 12, 100, 1).alpha, 0);
  assert.equal(fogCardSample(c, 1, 1, 50, 49, 1).alpha, 0);
  assert.equal(fogCardSample(c, 1, 1, 50, 100, 0).alpha, 0);
  assert.ok(
    Math.abs(
      fogCardSample(c, 1, 1, 21.44, 26.44, 1).alpha -
        0.25 * (1 - 1e-4 ** c.fresnelExponent),
    ) < 1e-12,
  );
  assert.ok(
    Math.abs(fogCardSample(c, 1, 1, 50, 100, 0.5).alpha - (1 - 2 ** -0.17)) <
      1e-12,
  );
});
test("owned border is explicit, zero on every edge, smooth, and leaves the interior unchanged", () => {
  const policy = "owned-soft-border-v1";
  for (const uv of [
    [0, 0.5],
    [1, 0.5],
    [0.5, 0],
    [0.5, 1],
    [0, 0],
  ] as [number, number][]) {
    assert.equal(fogCardEdgeFactor(uv), 1);
    assert.equal(fogCardEdgeFactor(uv, policy), 0);
  }
  assert.equal(fogCardEdgeFactor([0.0025, 0.5], policy), 0.5);
  assert.equal(fogCardEdgeFactor([0.005, 0.5], policy), 1);
  assert.equal(fogCardEdgeFactor([0.5, 0.5], policy), 1);
  const nearEdge = fogCardEdgeFactor([1e-7, 0.5], policy);
  assert.ok(nearEdge >= 0 && nearEdge < 1e-8);
  assert.throws(() => fogCardEdgeFactor([NaN, 0.5], policy));
  assert.throws(() => fogCardEdgeFactor([0.5, 0.5], "unknown" as any));
});
test("UV animation stays finite and reverse evaluation does not alter phase through one year", () => {
  const speed: [number, number] = [0.001, -0.0002],
    initial = fogCardPhase(speed, 0);
  assert.deepEqual(initial, [0, 0]);
  assert.deepEqual(fogCardPhase(speed, 5000), [0, 0]);
  const phase = fogCardPhase(speed, MAX_WORLD_ANIMATION_TIME);
  assert.ok(phase.every((n) => n >= 0 && n < 1));
  assert.deepEqual(fogCardPhase(speed, 0), initial);
  assert.throws(() => fogCardPhase(speed, MAX_WORLD_ANIMATION_TIME + 1));
  assert.throws(() => fogCardPhase([NaN, 0], 3));
});
test("manifest validates finite transforms, closed budgets, source hashes and modes", () => {
  validateFogCards(fogManifestFixture());
  for (const patch of [
    (m: SourceFogCardsManifest) =>
      m.scenarios.day!.cards.push(...Array(32).fill(fogCardFixture())),
    (m: SourceFogCardsManifest) => m.scenarios.day!.cards[0].model.fill(0),
    (m: SourceFogCardsManifest) =>
      (m.scenarios.day!.cards[0].brightness = Infinity),
    (m: SourceFogCardsManifest) => (m.textures.mask.bytes = 4),
    (m: SourceFogCardsManifest) =>
      (m.textures.mask.url = "https://example.com/private.bin"),
    (m: SourceFogCardsManifest) => (m.source.graphSha256 = "wrong"),
    (m: SourceFogCardsManifest) => ((m.scenarios as any).other = { cards: [] }),
    (m: SourceFogCardsManifest) => ((m as any).edgeTreatment = "arbitrary"),
  ]) {
    const m = fogManifestFixture();
    patch(m);
    assert.throws(() => validateFogCards(m));
  }
});
