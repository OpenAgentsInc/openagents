import assert from "node:assert/strict";
import { test } from "node:test";
import type { SceneLook } from "../packages/assets/src/scene.ts";
import { sourceBloomStrength } from "../packages/renderer/source-bloom.ts";

const volume = (value: unknown, properties: Record<string, unknown> = {}) => ({
  class: "PostProcessVolume",
  properties: {
    enabled: true,
    unbound: true,
    blend_weight: 1,
    priority: 0,
    settings: `(bOverride_BloomIntensity=True,BloomIntensity=${value})`,
    ...properties,
  },
});
const look = (...environment: unknown[]) =>
  ({
    scenarios: { day: { environment }, night: { environment: [] } },
  }) as unknown as SceneLook;

test("source day bloom zero overrides the owned default while absent night settings inherit", () => {
  const source = look(volume(0));
  assert.equal(sourceBloomStrength(source, "day", 0.12), 0);
  assert.equal(sourceBloomStrength(source, "night", 0.12), 0.12);
  assert.equal(sourceBloomStrength(undefined, "day", 0.12), 0.12);
});

test("source admission ignores inactive and unsupported records and blends valid priorities", () => {
  const source = look(
    volume(0.8, { priority: 2, blend_weight: 0.5 }),
    volume(0),
    volume(1, { enabled: false }),
    volume(1, { unbound: false }),
    volume(1, { blend_weight: 2 }),
    volume(1, { priority: NaN }),
    volume(3),
    volume("NaN"),
    volume(""),
    volume(1, {
      settings: "(bOverride_BloomIntensity=False,BloomIntensity=1)",
    }),
  );
  assert.equal(sourceBloomStrength(source, "day", 0.12), 0.4);
  assert.throws(() => sourceBloomStrength(source, "day", NaN));
});
