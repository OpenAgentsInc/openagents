import assert from "node:assert/strict";
import { createHash, webcrypto } from "node:crypto";
import test from "node:test";
import {
  CharacterAnimator,
  characterClipTime,
  characterPalette,
  characterSlerp,
  checkedCharacterBlob,
  sampleCharacterLocals,
  validateCharacter,
  validateCharacterGeometry,
  validateCharacterPNG,
  CHARACTER_MOTIONS,
  QUICK_MMORPG_MOTIONS,
  characterTextureBytes,
  characterTextureLevels,
  type CharacterManifest,
} from "../packages/renderer/character.ts";

const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
function fixture(): CharacterManifest {
  const file = (byteLength: number) => ({
    url: "/private-assets/character-test.bin",
    sha256: "0".repeat(64),
    byteLength,
  });
  return {
    schema: "aethyrforge.character.v1",
    name: "fixture",
    geometry: {
      vertexCount: 3,
      indexCount: 3,
      vertices: file(192),
      indices: file(12),
    },
    joints: [
      {
        parent: -1,
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
        inverseBind: [...identity],
      },
      {
        parent: 0,
        translation: [0, 1, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
        inverseBind: identity.map((v, i) => (i === 13 ? -1 : v)),
      },
    ],
    clips: CHARACTER_MOTIONS.map((name) => ({
      name,
      duration: 2,
      distance: name === "walk" ? 1.3 : 0,
      tracks: [],
    })),
    materials: [
      {
        name: "fixture",
        color: [1, 1, 1, 1],
        alphaCutoff: 0,
        doubleSided: true,
      },
    ],
    draws: [{ material: 0, firstIndex: 0, indexCount: 3 }],
  };
}
const near = (a: ArrayLike<number>, b: ArrayLike<number>) => {
  assert.equal(a.length, b.length);
  for (let i = 0; i < a.length; i++)
    assert.ok(Math.abs(a[i] - b[i]) < 1e-5, `${i}: ${a[i]} != ${b[i]}`);
};

test("character gaits only advance with distance admitted by collision", () => {
  const f = fixture();
  assert.equal(characterClipTime(f, "walk", 10, 0), 0);
  assert.equal(characterClipTime(f, "walk", 10, 1.3), 2);
  assert.equal(characterClipTime(f, "idle", 10, 1.3), 10);
});

function quickFixture() {
  const f = fixture();
  f.controller = { profile: "quick-mmorpg-v1", worldScale: 1 / 6 };
  f.clips = QUICK_MMORPG_MOTIONS.map((name) => ({
    name,
    duration: 2,
    distance: 0,
    loop: name === "idle" || name === "walk" || name === "run",
    tracks: [
      {
        joint: 0,
        translation: [
          { time: 0, value: [0, 0, 0] },
          { time: 2, value: [name === "walk" ? 2 : 6, 0, 0] },
        ],
        rotation: [],
        scale: [],
      },
    ],
  }));
  return f;
}

test("the source controller accepts its six real clips without synthetic gaits", () => {
  const f = quickFixture();
  validateCharacter(f);
  assert.equal(f.clips.length, 6);
  assert.throws(() => validateCharacter({ ...f, controller: undefined }));
  for (const worldScale of [0.009, 1.001, NaN, Infinity])
    assert.throws(() =>
      validateCharacter({
        ...f,
        controller: { profile: "quick-mmorpg-v1", worldScale },
      }),
    );
  const missing = structuredClone(f);
  missing.clips.pop();
  assert.throws(() => validateCharacter(missing));
  const extra = structuredClone(f);
  extra.clips.push({ name: "jump", duration: 1, distance: 0, tracks: [] });
  assert.throws(() => validateCharacter(extra), /supported names/);
});

test("explicit transitions advance both source and incoming clips and finish without a second fade", () => {
  const f = quickFixture(),
    animator = new CharacterAnimator(f);
  animator.sample("idle", 0, 0);
  // At 0.5 seconds the outgoing walk has moved 0.5; the run has moved 1.5.
  assert.equal(
    animator.sample("run", 0.5, 1, {
      from: "walk",
      fromTime: 0.5,
      weight: 0.25,
    })[12],
    0.75,
  );
  // Both clips keep moving during the crossfade, rather than freezing the last rendered pose.
  assert.equal(
    animator.sample("run", 1, 1.1, {
      from: "walk",
      fromTime: 1,
      weight: 0.5,
    })[12],
    2,
  );
  assert.equal(
    animator.sample("run", 1.5, 1.2, {
      from: "walk",
      fromTime: 1.5,
      weight: 1,
    })[12],
    4.5,
  );
  assert.ok(Math.abs(animator.sample("run", 1.6, 1.3)[12] - 4.8) < 1e-5);
  assert.equal(animator.sample("walk", 1, 1.4)[12], 1);
  assert.throws(() =>
    animator.sample("run", 0, 2, { from: "walk", fromTime: NaN, weight: 0 }),
  );
  assert.throws(() =>
    animator.sample("run", 0, 2, { from: "walk", fromTime: 0, weight: 1.1 }),
  );
});

test("authored one-shot attack, dance and death stay on their final pose", () => {
  const f = quickFixture();
  for (const motion of ["attack", "dance", "death"] as const) {
    assert.equal(sampleCharacterLocals(f, motion, 99)[0].translation[0], 6);
    assert.equal(sampleCharacterLocals(f, motion, -1)[0].translation[0], 0);
  }
  assert.equal(sampleCharacterLocals(f, "run", 2.5)[0].translation[0], 1.5);
});

test("character PBR admission counts shared textures and every allocated mip", () => {
  assert.equal(characterTextureLevels(2048, 2048), 12);
  assert.equal(characterTextureBytes(2048, 2048), 22369620);
  assert.equal(characterTextureBytes(8, 2), 92);
  const f = fixture(),
    texture = {
      url: "/private-assets/character-pbr.png",
      sha256: "1".repeat(64),
      byteLength: 1024,
      width: 2048,
      height: 2048,
    };
  f.materials[0] = {
    ...f.materials[0],
    texture,
    normalTexture: texture,
    metallic: 0.5,
    roughness: 0.5527864,
  };
  f.materials.push({ ...f.materials[0], name: "shared" });
  validateCharacter(f); // Two 2048 maps are ~42.7 MiB, even when several draws reuse them.
  const over = structuredClone(f);
  over.materials.push({
    ...over.materials[0],
    name: "third",
    texture: { ...texture, sha256: "2".repeat(64) },
  });
  // Three full 2048 mip chains still fit; a fourth crosses the fixed 64 MiB budget.
  validateCharacter(over);
  over.materials.push({
    ...over.materials[0],
    name: "fourth",
    texture: { ...texture, sha256: "3".repeat(64) },
  });
  assert.throws(() => validateCharacter(over), /texture budget/);
  for (const key of ["roughness", "metallic", "normalScale"] as const) {
    const bad = structuredClone(f);
    bad.materials[0][key] = -1;
    assert.throws(() => validateCharacter(bad));
  }
});

test("character PNG dimensions are checked before decoding", () => {
  const bytes = new ArrayBuffer(33),
    data = new Uint8Array(bytes),
    view = new DataView(bytes);
  data.set([137, 80, 78, 71, 13, 10, 26, 10]);
  view.setUint32(8, 13);
  view.setUint32(12, 0x49484452);
  view.setUint32(16, 512);
  view.setUint32(20, 256);
  validateCharacterPNG(bytes, 512, 256);
  assert.throws(() => validateCharacterPNG(bytes, 2048, 256));
  assert.throws(() => validateCharacterPNG(new ArrayBuffer(0), 512, 256));
});

test("character bind pose cancels inverse binds through the hierarchy", () => {
  const f = validateCharacter(fixture());
  const palette = characterPalette(f, sampleCharacterLocals(f, "idle", 0));
  near(palette.subarray(0, 16), identity);
  near(palette.subarray(32, 48), identity);
});
test("character clips interpolate local rotations on the shortest unit arc and wrap", () => {
  const f = fixture();
  f.clips[1].tracks.push({
    joint: 1,
    translation: [],
    rotation: [
      { time: 0, value: [0, 0, 0, 1] },
      { time: 2, value: [0, 1, 0, 0] },
    ],
    scale: [],
  });
  validateCharacter(f);
  const pose = sampleCharacterLocals(f, "walk", 1),
    q = [0, Math.SQRT1_2, 0, Math.SQRT1_2];
  near(pose[1].rotation, q);
  near(sampleCharacterLocals(f, "walk", 3)[1].rotation, q);
  near(
    characterSlerp(
      q,
      q.map((v) => -v),
      0.5,
    ),
    q,
  );
  near(pose[1].translation, [0, 1, 0]);
});
test("character palette uses inverse-transpose normals under nonuniform scale", () => {
  const f = fixture();
  f.joints[0].scale = [2, 3, 4];
  const p = characterPalette(f, sampleCharacterLocals(f, "idle", 0));
  near([p[0], p[5], p[10]], [2, 3, 4]);
  near([p[16], p[21], p[26]], [0.5, 1 / 3, 0.25]);
});
test("character motion changes blend local poses without an immediate jump", () => {
  const f = fixture();
  f.clips[1].tracks.push({
    joint: 0,
    translation: [{ time: 0, value: [4, 0, 0] }],
    rotation: [],
    scale: [],
  });
  const animator = new CharacterAnimator(f);
  assert.equal(animator.sample("idle", 0, 0)[12], 0);
  assert.equal(animator.sample("walk", 0, 1)[12], 0);
  assert.ok(Math.abs(animator.sample("walk", 0, 1.06)[12] - 2) < 1e-5);
  assert.equal(animator.sample("walk", 0, 1.2)[12], 4);
  assert.equal(animator.sample("idle", 0, 0)[12], 0);
});
test("character manifest rejects cycles, bad tracks, nonlocal files, and partial draws", () => {
  for (const change of [
    (f: CharacterManifest) => {
      f.joints[1].parent = 1;
    },
    (f: CharacterManifest) => {
      f.joints[0].rotation = [0, 0, 0, 0];
    },
    (f: CharacterManifest) => {
      f.geometry.vertices.url = "https://untrusted.example/model";
    },
    (f: CharacterManifest) => {
      f.clips.pop();
    },
    (f: CharacterManifest) => {
      f.draws[0].firstIndex = 3;
    },
    (f: CharacterManifest) => {
      f.clips[0].tracks.push({
        joint: 0,
        translation: [
          { time: 1, value: [0, 0, 0] },
          { time: 0, value: [0, 0, 0] },
        ],
        rotation: [],
        scale: [],
      });
    },
  ]) {
    const f = fixture();
    change(f);
    assert.throws(() => validateCharacter(f));
  }
});
test("character geometry validates joints, weights, normals, and triangle addresses", () => {
  const f = fixture(),
    v = new Float32Array(48),
    indices = new Uint32Array([0, 1, 2]);
  for (let i = 0; i < 48; i += 16) {
    v[i + 4] = 1;
    v[i + 12] = 1;
  }
  validateCharacterGeometry(f, v, indices);
  const bad = v.slice();
  bad[8] = 2;
  assert.throws(() => validateCharacterGeometry(f, bad, indices));
  bad[8] = 0;
  bad[12] = 0.5;
  assert.throws(() => validateCharacterGeometry(f, bad, indices));
  assert.throws(() =>
    validateCharacterGeometry(f, v, new Uint32Array([0, 1, 3])),
  );
});
test("character resources are checked against both byte length and SHA-256", async () => {
  if (!globalThis.crypto)
    Object.defineProperty(globalThis, "crypto", { value: webcrypto });
  const bytes = new Uint8Array([1, 2, 3, 4]).buffer;
  const description = {
    url: "/private-assets/character-test.bin",
    byteLength: 4,
    sha256: createHash("sha256").update(new Uint8Array(bytes)).digest("hex"),
  };
  assert.equal(
    await checkedCharacterBlob(description, async () => bytes),
    bytes,
  );
  await assert.rejects(
    checkedCharacterBlob(
      { ...description, sha256: "0".repeat(64) },
      async () => bytes,
    ),
  );
  await assert.rejects(
    checkedCharacterBlob(description, async () => new ArrayBuffer(3)),
  );
});
