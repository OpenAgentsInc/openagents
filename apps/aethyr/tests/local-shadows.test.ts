import assert from "node:assert/strict";
import { test } from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import type { SourceLight } from "../packages/assets/src/scene.ts";

const light = (id: string, kind: "point" | "spot" = "spot"): SourceLight => ({
  id,
  label: id,
  kind,
  position: [2, 3, 4],
  direction: [0, -1, 0],
  color: [1, 1, 1],
  intensity: 10,
  intensityUnits: "candelas",
  range: 12,
  innerConeAngleRadians: 0.3,
  outerConeAngleRadians: 0.6,
  castShadows: true,
  enabled: true,
  temperatureKelvin: 6500,
  useTemperature: false,
  raw: {},
});
// Inspect planning before GPU submission so redraw work can be asserted without a device.
const cache = () => new Renderer() as any;
const dirty = (renderer: any): number[] =>
  [...renderer.localDirtyViews].sort((a: number, b: number) => a - b);
const consume = (renderer: any): void => renderer.localDirtyViews.clear();

test("light reorder preserves shadow layers and causes no redraws", () => {
  const renderer = cache();
  const point = light("point", "point");
  const spots = Array.from({ length: 8 }, (_, i) => light(`spot-${i}`));
  renderer.prepareLocalShadows([point, ...spots]);
  const slots = new Map(renderer.localSlots);
  assert.equal(slots.get(point.id), 0);
  assert.deepEqual(dirty(renderer), [
    0,
    1,
    2,
    3,
    4,
    5,
    ...Array.from({ length: 8 }, (_, i) => i + 24),
  ]);
  assert.deepEqual(
    spots.map((spot) => slots.get(spot.id)),
    Array.from({ length: 8 }, (_, i) => i + 24),
  );
  consume(renderer);
  renderer.prepareLocalShadows([...spots].reverse().concat(point));
  assert.deepEqual(new Map(renderer.localSlots), slots);
  assert.deepEqual(dirty(renderer), []);
});

test("evicting one spot redraws its reused layer while retained lights keep their maps", () => {
  const renderer = cache();
  const point = light("point", "point");
  const spots = Array.from({ length: 8 }, (_, i) => light(`spot-${i}`));
  renderer.prepareLocalShadows([point, ...spots]);
  const evictedLayer = renderer.localSlots.get(spots[2].id);
  const retainedLayers = spots
    .filter((_, i) => i !== 2)
    .map((spot) => [spot.id, renderer.localSlots.get(spot.id)]);
  consume(renderer);
  const replacement = light("replacement");
  renderer.prepareLocalShadows([
    point,
    ...spots.filter((_, i) => i !== 2),
    replacement,
  ]);
  assert.equal(renderer.localSlots.has(spots[2].id), false);
  assert.equal(renderer.localSlots.get(replacement.id), evictedLayer);
  assert.deepEqual(dirty(renderer), [evictedLayer]);
  for (const [id, layer] of retainedLayers)
    assert.equal(renderer.localSlots.get(id), layer);
});

test("point changes redraw only its six faces and spots always use layers24through31", () => {
  const renderer = cache();
  const spot = light("spot");
  renderer.prepareLocalShadows([spot]);
  assert.equal(renderer.localSlots.get(spot.id), 24);
  consume(renderer);
  renderer.prepareLocalShadows([light("first-point", "point"), spot]);
  assert.deepEqual(dirty(renderer), [0, 1, 2, 3, 4, 5]);
  consume(renderer);
  renderer.prepareLocalShadows([light("second-point", "point"), spot]);
  assert.equal(renderer.localSlots.has("first-point"), false);
  assert.equal(renderer.localSlots.get("second-point"), 0);
  assert.equal(renderer.localSlots.get(spot.id), 24);
  assert.deepEqual(dirty(renderer), [0, 1, 2, 3, 4, 5]);
});

test("geometry and shadow projection changes invalidate maps but light color changes do not", () => {
  const renderer = cache();
  const point = light("point", "point");
  const spot = light("spot");
  renderer.prepareLocalShadows([point, spot]);
  consume(renderer);
  renderer.prepareLocalShadows([
    { ...point, direction: [1, 0, 0], color: [0, 1, 0] },
    { ...spot, intensity: 20 },
  ]);
  assert.deepEqual(dirty(renderer), []);
  renderer.prepareLocalShadows([point, { ...spot, position: [3, 4, 5] }]);
  assert.deepEqual(dirty(renderer), [24]);
  consume(renderer);
  renderer.geometryVersion++;
  renderer.prepareLocalShadows([point, { ...spot, position: [3, 4, 5] }]);
  assert.deepEqual(dirty(renderer), [0, 1, 2, 3, 4, 5, 24]);
  consume(renderer);
  renderer.prepareLocalShadows([]);
  assert.equal(renderer.localSlots.size, 0);
  assert.deepEqual(dirty(renderer), []);
  assert.ok(renderer.localViews.every((view: unknown) => view === undefined));
});

test("four points retain disjoint cube faces and only an evicted cube redraws", () => {
  const renderer = cache();
  const points = Array.from({ length: 4 }, (_, i) => light(`p${i}`, "point"));
  renderer.prepareLocalShadows(points);
  assert.deepEqual(
    points.map((p) => renderer.localSlots.get(p.id)),
    [0, 6, 12, 18],
  );
  assert.deepEqual(
    dirty(renderer),
    Array.from({ length: 24 }, (_, i) => i),
  );
  consume(renderer);
  renderer.prepareLocalShadows([...points].reverse());
  assert.deepEqual(dirty(renderer), []);
  const replacement = light("replacement", "point");
  renderer.prepareLocalShadows([points[0], points[2], points[3], replacement]);
  assert.equal(renderer.localSlots.get(replacement.id), 6);
  assert.deepEqual(dirty(renderer), [6, 7, 8, 9, 10, 11]);
});

test("the full budget assigns four point cubes and sixteen independent spots", () => {
  const renderer = cache();
  const points = Array.from({ length: 4 }, (_, i) => ({
    ...light(`p${i}`, "point"),
    range: 1,
  }));
  const spots = Array.from({ length: 16 }, (_, i) => light(`s${i}`));
  renderer.prepareLocalShadows([...points, ...spots]);
  assert.deepEqual(
    dirty(renderer),
    Array.from({ length: 40 }, (_, i) => i),
  );
  assert.deepEqual(
    points.map((p) => renderer.localSlots.get(p.id)),
    [0, 6, 12, 18],
  );
  assert.deepEqual(
    spots.map((p) => renderer.localSlots.get(p.id)),
    Array.from({ length: 16 }, (_, i) => i + 24),
  );
  consume(renderer);
  renderer.prepareLocalShadows([...spots, ...points]);
  assert.deepEqual(dirty(renderer), []);
});
