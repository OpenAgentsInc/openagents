import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import {
  collisionInstanceIdentity,
  installStaticCollision,
  loadStaticCollision,
  validateStaticCollision,
  type StaticCollisionManifest,
} from "../packages/runtime/static-collision.ts";
const digest = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
const scene = {
  vertices: "/private-assets/vertices.bin",
  indices: "/private-assets/indices.bin",
  instances: [
    { mesh: 3, model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 2, 3, 4, 1] },
  ],
  collisionURL: `/private-assets/collision-${"1".repeat(64)}.json`,
} as CompiledScene;
async function fixture() {
  const bytes = new Float32Array([0, 0, 0, 1, 0, 0, 0, 0, 1]);
  const manifest: StaticCollisionManifest = {
    schema: "aethyrforge.static-collision.v1",
    profile: "simplified-solid-surfaces-v1",
    source: {
      vertices: scene.vertices,
      indices: scene.indices,
      instancesSha256: await collisionInstanceIdentity(scene.instances),
    },
    triangles: {
      url: `/private-assets/collision-${digest(new Uint8Array(bytes.buffer))}.bin`,
      sha256: digest(new Uint8Array(bytes.buffer)),
      byteLength: 36,
      count: 1,
    },
  };
  return { bytes, manifest };
}
test("collision identity retains mesh and exact affine placement bytes", async () => {
  const bytes = Buffer.alloc(132);
  bytes.writeUInt32LE(3, 0);
  scene.instances[0].model.forEach((n, i) => bytes.writeDoubleLE(n, 4 + i * 8));
  assert.equal(await collisionInstanceIdentity(scene.instances), digest(bytes));
});
test("collision loading verifies source placement, byte count, and content hash", async () => {
  const { bytes, manifest } = await fixture();
  const fetcher = (async (url: string) =>
    new Response(
      url === scene.collisionURL ? JSON.stringify(manifest) : bytes,
    )) as typeof fetch;
  assert.deepEqual(await loadStaticCollision(scene, fetcher), {
    triangles: bytes,
  });
  await assert.rejects(
    loadStaticCollision(
      { ...scene, instances: [{ ...scene.instances[0], mesh: 4 }] },
      fetcher,
    ),
    /changing the town geometry/,
  );
  const changed = (async (url: string) =>
    new Response(
      url === scene.collisionURL
        ? JSON.stringify(manifest)
        : new Float32Array(9),
    )) as typeof fetch;
  await assert.rejects(loadStaticCollision(scene, changed), /incomplete/);
});
test("collision descriptions reject oversized, stale, and nonlocal geometry", async () => {
  const { manifest } = await fixture();
  assert.throws(() =>
    validateStaticCollision(
      { ...manifest, source: { ...manifest.source, vertices: "different" } },
      scene,
    ),
  );
  assert.throws(() =>
    validateStaticCollision(
      { ...manifest, triangles: { ...manifest.triangles, count: 2_000_001 } },
      scene,
    ),
  );
  assert.throws(() =>
    validateStaticCollision(
      {
        ...manifest,
        triangles: {
          ...manifest.triangles,
          url: "https://example.com/triangles.bin",
        },
      },
      scene,
    ),
  );
  await assert.rejects(
    loadStaticCollision(
      scene,
      (async () => new Response("x".repeat(16385))) as typeof fetch,
    ),
    /size limit/,
  );
});

test("breakable collision verifies owners and binds them to the admitted cell manifest", async () => {
  const { bytes, manifest } = await fixture();
  const owners = new Uint32Array([3]);
  const ownersHash = digest(new Uint8Array(owners.buffer));
  const destructibleScene = {
    ...scene,
    gameplayURL: `/private-assets/gameplay-${"2".repeat(64)}.json`,
  };
  const complete = {
    ...manifest,
    source: { ...manifest.source, gameplayURL: destructibleScene.gameplayURL },
    owners: {
      url: `/private-assets/collision-owners-${ownersHash}.bin`,
      sha256: ownersHash,
      byteLength: 4,
      count: 1,
    },
  };
  const fetcher = (async (url: string) =>
    new Response(
      url === scene.collisionURL
        ? JSON.stringify(complete)
        : url === complete.owners.url
          ? owners
          : bytes,
    )) as typeof fetch;
  assert.deepEqual(await loadStaticCollision(destructibleScene, fetcher), {
    triangles: bytes,
    owners,
  });
  assert.throws(() => validateStaticCollision(manifest, destructibleScene));
  assert.throws(() =>
    validateStaticCollision(complete, {
      ...destructibleScene,
      gameplayURL: "different",
    }),
  );
  const wrong = (async (url: string) =>
    new Response(
      url === scene.collisionURL
        ? JSON.stringify(complete)
        : url === complete.owners.url
          ? new Uint32Array([4])
          : bytes,
    )) as typeof fetch;
  await assert.rejects(
    loadStaticCollision(destructibleScene, wrong),
    /incomplete/,
  );
  const messages: any[] = [];
  installStaticCollision(
    {
      request: (value) => {
        messages.push(value);
        return { ok: true };
      },
    },
    { triangles: bytes, owners },
    4,
  );
  assert.deepEqual(messages[0].owners, [3]);
  assert.throws(
    () =>
      installStaticCollision(
        { request: () => ({ ok: true }) },
        { triangles: bytes, owners },
        3,
      ),
    /match its buildings/,
  );
});
test("collision uploads stay below the kernel message limit and replace before appending", () => {
  const messages: any[] = [];
  const kernel = {
    request: (message: object) => {
      messages.push(message);
      return { ok: true };
    },
  };
  installStaticCollision(kernel, new Float32Array(8200 * 9));
  assert.equal(messages.length, 2);
  assert.equal(messages[0].append, false);
  assert.equal(messages[1].append, true);
  assert.equal(messages[0].triangles.length, 8192);
  assert.equal(messages[1].triangles.length, 8);
  assert.ok(messages.every((m) => JSON.stringify(m).length < 4 * 1024 * 1024));
  installStaticCollision(kernel, new Float32Array());
  assert.deepEqual(messages.at(-1), {
    type: "set_static_collision",
    triangles: [],
    append: false,
  });
  assert.throws(
    () =>
      installStaticCollision(
        { request: () => ({ ok: false, error: { message: "bad mesh" } }) },
        undefined,
      ),
    /bad mesh/,
  );
});
