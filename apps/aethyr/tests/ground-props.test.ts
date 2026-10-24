import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { AssetIntegrityError } from "../packages/assets/src/asset-download.ts";
import type { CatalogAsset } from "../packages/assets/src/scene.ts";
import {
  GROUND_PROPS_LIMITS,
  loadGroundCatalog,
  loadGroundProps,
  validateGroundCatalog,
  validateGroundDraft,
  validateGroundProps,
} from "../packages/runtime/ground-props.ts";

const entity = (id = "prop-1") => ({
  id,
  kind: "quaternius:fantasy-standard:cauldron",
  position: [2, 0, -3],
  yaw: 0.4,
  scale: 1.5,
  collidable: false,
  halfExtents: [1, 2, 3],
  collisionOffset: [0.1, 2, 0.2],
});
const document = () => ({
  schema: "aethyrforge.ground-props.v1",
  entities: [entity()],
  sourceKit: "Quaternius Fantasy Props MegaKit Standard",
  license: "CC0-1.0",
  placements: 1,
});
function catalog(): Map<string, CatalogAsset> {
  const asset: CatalogAsset = {
    id: entity().kind,
    name: "Cauldron",
    aliases: [],
    mesh: 2,
    dimensions: [2, 3, 4],
    bounds: { min: [-1, 0, -2], max: [1, 3, 2] },
    kind: "prop",
    materialProfile: "gltf-pbr",
    wallParts: [],
  };
  return new Map([[asset.id, asset]]);
}
const encode = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value));
const path = (bytes: Uint8Array) =>
  `/private-assets/ground-props-${createHash("sha256").update(bytes).digest("hex")}.json`;

test("prepared decorations are bounded, copied and limited to noncolliding props", () => {
  const source = document(),
    parsed = validateGroundProps(source);
  assert.deepEqual(parsed, source);
  parsed.entities[0].position[0] = 100;
  assert.equal(source.entities[0].position[0], 2);
  const maximum = {
    ...document(),
    entities: Array.from({ length: 128 }, (_, i) => entity(`prop-${i}`)),
    placements: 128,
  };
  assert.equal(validateGroundProps(maximum).entities.length, 128);
  for (const patch of [
    { id: "x".repeat(97) },
    { id: "bad id" },
    { kind: "building:house" },
    { kind: "quaternius:fantasy-standard:Upper" },
    { collidable: true },
    { collidable: undefined },
    { position: [0, NaN, 0] },
    { position: [20001, 0, 0] },
    { position: [0, 0] },
    { position: Array(3) },
    { yaw: Infinity },
    { scale: 0.249 },
    { scale: 4.001 },
    { halfExtents: [0, 1, 1] },
    { halfExtents: [1, 101, 1] },
    { collisionOffset: [0, Infinity, 0] },
  ])
    assert.throws(() =>
      validateGroundProps({
        ...document(),
        entities: [{ ...entity(), ...patch }],
      }),
    );
  for (const patch of [
    { entities: Array(1), placements: 1 },
    { entities: [entity(), entity()], placements: 2 },
    { entities: [...maximum.entities, entity("overflow")], placements: 129 },
    { placements: 2 },
    { sourceKit: {} },
    { license: "bad\nmetadata" },
    { colliders: [{}] },
    { structures: [{}] },
    { rooms: [{}] },
    { removedWalls: ["wall"] },
  ])
    assert.throws(() => validateGroundProps({ ...document(), ...patch }));
});

test("draft validation derives bounds from the known mesh and discards cached fields", () => {
  const source = {
    entities: [
      {
        ...entity(),
        halfExtents: [NaN],
        collisionOffset: "stale",
        transient: true,
      },
    ],
    rooms: [],
    removedWalls: [],
    revision: 42,
  };
  const draft = validateGroundDraft(source, catalog());
  assert.deepEqual(draft, {
    entities: [
      { ...entity(), halfExtents: [1, 1.5, 2], collisionOffset: [0, 1.5, 0] },
    ],
    rooms: [],
    removedWalls: [],
  });
  draft.entities[0].position[0] = 50;
  assert.equal(source.entities[0].position[0], 2);
  const flat = catalog();
  flat.get(entity().kind)!.bounds = { min: [-1, 0, 0], max: [1, 2, 0] };
  assert.deepEqual(
    validateGroundDraft({ entities: [entity()] }, flat).entities[0].halfExtents,
    [1, 1, 0.01],
  );
});

test("local drafts cannot add rooms, wall removals, collisions or unknown structural assets", () => {
  for (const patch of [
    { rooms: [{}] },
    { rooms: {} },
    { removedWalls: ["wall"] },
    { removed_walls: ["wall"] },
    { colliders: [{}] },
    { structures: [{}] },
  ])
    assert.throws(() =>
      validateGroundDraft({ entities: [entity()], ...patch }, catalog()),
    );
  assert.throws(() =>
    validateGroundDraft(
      { entities: [{ ...entity(), collidable: true }] },
      catalog(),
    ),
  );
  assert.throws(() => validateGroundDraft({ entities: [entity()] }, new Map()));
  for (const patch of [
    { kind: "building" },
    { mesh: -1 },
    { mesh: NaN },
    { id: "other" },
    { plan: {} },
    { wallParts: [{ wallId: "wall", primitiveIndices: [0] }] },
    { bounds: { min: [2, 0, 0], max: [1, 1, 1] } },
    { bounds: { min: [0, 0, 0], max: [201, 1, 1] } },
  ]) {
    const assets = catalog();
    Object.assign(assets.get(entity().kind)!, patch);
    assert.throws(() => validateGroundDraft({ entities: [entity()] }, assets));
  }
});

test("the immutable prop loader verifies streamed bytes before accepting the document", async () => {
  const value = document(),
    bytes = encode(value),
    url = path(bytes);
  let calls = 0;
  const loaded = await loadGroundProps(url, {
    fetcher: async (requested) => {
      calls++;
      assert.equal(requested, url);
      return new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(bytes.slice(0, 57));
            controller.enqueue(bytes.slice(57));
            controller.close();
          },
        }),
      );
    },
  });
  assert.deepEqual(loaded, value);
  assert.equal(calls, 1);
  // A hash mismatch is detected even when the received bytes are not JSON.
  await assert.rejects(
    loadGroundProps(url, { fetcher: async () => new Response("not json") }),
    AssetIntegrityError,
  );
});

test("the prop loader rejects unsafe paths, HTTP failures and invalid verified JSON", async () => {
  let calls = 0;
  for (const url of [
    "/private-assets/ground-props.json",
    "/private-assets/../ground-props-" + "a".repeat(64) + ".json",
    "https://other.test/private-assets/ground-props-" +
      "a".repeat(64) +
      ".json",
    "/private-assets/ground-props-" + "A".repeat(64) + ".json",
  ]) {
    await assert.rejects(
      loadGroundProps(url, {
        fetcher: async () => {
          calls++;
          return new Response(encode(document()));
        },
      }),
    );
  }
  assert.equal(calls, 0);
  const bytes = encode(document()),
    url = path(bytes);
  await assert.rejects(
    loadGroundProps(url, {
      fetcher: async () => {
        calls++;
        return new Response("missing", { status: 404 });
      },
    }),
    /could not load/,
  );
  assert.equal(calls, 1);
  for (const invalid of [
    encode({ ...document(), schema: "other" }),
    new TextEncoder().encode("{"),
    new Uint8Array([0xff]),
  ])
    await assert.rejects(
      loadGroundProps(path(invalid), {
        fetcher: async () => new Response(invalid),
      }),
    );
});

test("oversized prop downloads stop and cancel before parsing or collecting more data", async () => {
  const url = path(encode(document()));
  for (const declared of [false, true]) {
    let cancelled = false;
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        if (!declared)
          controller.enqueue(new Uint8Array(GROUND_PROPS_LIMITS.bytes + 1));
      },
      cancel() {
        cancelled = true;
      },
    });
    await assert.rejects(
      loadGroundProps(url, {
        fetcher: async () =>
          new Response(
            stream,
            declared
              ? {
                  headers: {
                    "content-length": String(GROUND_PROPS_LIMITS.bytes + 1),
                  },
                }
              : undefined,
          ),
      }),
      /too large/,
    );
    assert(cancelled);
  }
});

test("a broken first prop transfer is retried as a complete new file", async () => {
  const bytes = encode(document()),
    url = path(bytes);
  let calls = 0;
  const loaded = await loadGroundProps(url, {
    fetcher: async () =>
      new Response(++calls === 1 ? bytes.slice(0, 10) : bytes),
  });
  assert.equal(calls, 2);
  assert.deepEqual(loaded, document());
});

const catalogDocument = () => ({
  schema: "aethyrforge.private-catalog.v1",
  assets: [...catalog().values()],
  source: "Quaternius Fantasy Props MegaKit Standard",
});
test("the ground catalog accepts only bounded known prop meshes and copies their metadata", () => {
  const source = catalogDocument(),
    parsed = validateGroundCatalog(source);
  assert.deepEqual(parsed, { ...source, limitations: [] });
  parsed.assets[0].bounds.min[0] = 100;
  assert.equal(source.assets[0].bounds.min[0], -1);
  for (const patch of [
    { id: "unrelated:prop" },
    { kind: "building" },
    { plan: {} },
    { wallParts: [{}] },
    { mesh: -1 },
    { mesh: 4096 },
    { mesh: 0.2 },
    { materialProfile: "legacy-base-color-recipe" },
    { aliases: Array(33).fill("alias") },
    { aliases: ["a".repeat(65)] },
    { aliases: [{}] },
    { aliases: Array(1) },
    { dimensions: [1, NaN, 1] },
    { dimensions: [1, 2] },
    { bounds: { min: [0, 0, 0], max: [Infinity, 1, 1] } },
    { bounds: { min: [1, 0, 0], max: [0, 1, 1] } },
  ])
    assert.throws(() =>
      validateGroundCatalog({
        ...catalogDocument(),
        assets: [{ ...catalogDocument().assets[0], ...patch }],
      }),
    );
  for (const patch of [
    { assets: [catalogDocument().assets[0], catalogDocument().assets[0]] },
    {
      assets: Array.from({ length: 129 }, (_, i) => ({
        ...catalogDocument().assets[0],
        id: `quaternius:fantasy-standard:prop-${i}`,
      })),
    },
    { source: undefined },
    { source: {} },
    { limitations: [{}] },
    { limitations: Array(33).fill("limit") },
  ])
    assert.throws(() =>
      validateGroundCatalog({ ...catalogDocument(), ...patch }),
    );
});

test("catalog loading shares byte and identity checks while retaining its distinct limit", async () => {
  const source = catalogDocument(),
    bytes = encode(source),
    url = path(bytes).replace("ground-props-", "ground-catalog-");
  assert.deepEqual(
    await loadGroundCatalog(url, { fetcher: async () => new Response(bytes) }),
    { ...source, limitations: [] },
  );
  await assert.rejects(
    loadGroundCatalog(path(bytes), {
      fetcher: async () => new Response(bytes),
    }),
    /prepared/,
  );
  await assert.rejects(
    loadGroundCatalog(url, {
      fetcher: async () => new Response("missing", { status: 404 }),
    }),
    /could not load/,
  );
  await assert.rejects(
    loadGroundCatalog(url, { fetcher: async () => new Response(encode({})) }),
    AssetIntegrityError,
  );
  const invalid = encode({
    ...source,
    assets: [{ ...source.assets[0], kind: "building" }],
  });
  await assert.rejects(
    loadGroundCatalog(
      path(invalid).replace("ground-props-", "ground-catalog-"),
      { fetcher: async () => new Response(invalid) },
    ),
  );
  const padded = new TextEncoder().encode(
    JSON.stringify(source) + " ".repeat(GROUND_PROPS_LIMITS.bytes),
  );
  assert(
    padded.byteLength > GROUND_PROPS_LIMITS.bytes &&
      padded.byteLength < GROUND_PROPS_LIMITS.catalogBytes,
  );
  assert.deepEqual(
    await loadGroundCatalog(
      path(padded).replace("ground-props-", "ground-catalog-"),
      { fetcher: async () => new Response(padded) },
    ),
    { ...source, limitations: [] },
  );
  let cancelled = false;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new Uint8Array(GROUND_PROPS_LIMITS.catalogBytes + 1));
    },
    cancel() {
      cancelled = true;
    },
  });
  await assert.rejects(
    loadGroundCatalog(url, { fetcher: async () => new Response(stream) }),
    /too large/,
  );
  assert(cancelled);
});
