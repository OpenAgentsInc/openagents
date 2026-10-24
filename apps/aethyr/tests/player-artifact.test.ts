import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  PLAYER_ARTIFACT_SCHEMA,
  playerFeatureClosure,
  playerProfileFeatures,
  validatePlayerArtifact,
  verifyPlayerArtifact,
  type PlayerArtifact,
  type PlayerFeature,
  type PlayerProfile,
} from "../packages/runtime/player-artifact.ts";
import type { StreamPackageManifest } from "../packages/runtime/cell-stream.ts";

const hash = (value: string) =>
  createHash("sha256").update(value).digest("hex");
function sorted(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sorted);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, child]) => [key, sorted(child)]),
    );
  return value;
}
function fixture(profile: PlayerProfile = "room"): PlayerArtifact {
  const artifact: PlayerArtifact = {
    schema: PLAYER_ARTIFACT_SCHEMA,
    abi: 1,
    profile,
    worldId: "owned-courtyard",
    sourceIdentity: hash("owned source"),
    runtime: {
      abi: 1,
      url: "/assets/player-123.js",
      sha256: hash("player js"),
      bytes: 9,
    },
    features: playerProfileFeatures(profile),
    resources: {},
  };
  if (profile === "empty") return artifact;
  const files = [
    "scene-day.json",
    "scene-night.json",
    "world.json",
    "colliders.json",
    "game.json",
    "mesh.bin",
    ...(profile === "character" ? ["character.bin", "sky.json"] : []),
  ].map((name, i) => ({
    url: `/starter/${name}`,
    sha256: hash(name),
    bytes: i + 1,
    gzipBytes: i + 21,
  }));
  const total = (key: "bytes" | "gzipBytes") =>
    files.reduce((sum, file) => sum + file[key], 0);
  const cooked: StreamPackageManifest = {
    schema: "aethyrforge.stream-package.v1",
    abi: 1,
    identityScope: "stream-descriptor-v1",
    sha256: "0".repeat(64),
    entryCell: "entry",
    cells: [
      {
        id: "entry",
        quality: "source",
        bounds: { min: [-8, -8], max: [8, 8] },
        scenes: { day: files[0].url, night: files[1].url },
        resources: structuredClone(files),
      },
    ],
    files,
    totals: {
      rawBytes: total("bytes"),
      gzipBytes: total("gzipBytes"),
      uniqueRawBytes: total("bytes"),
      uniqueGzipBytes: total("gzipBytes"),
    },
  };
  const { schema, abi, entryCell, cells, totals } = cooked;
  cooked.sha256 = hash(
    JSON.stringify(sorted({ schema, abi, entryCell, cells, files, totals })),
  );
  artifact.kernel = {
    abi: 1,
    url: "/kernel.wasm",
    sha256: hash("kernel"),
    bytes: 8,
  };
  artifact.content = {
    package: cooked,
    entryCell: "entry",
    worldURL: "/starter/world.json",
  };
  for (const name of artifact.features) artifact.resources[name] = [];
  artifact.resources.render = [
    files[0].url,
    files[1].url,
    files[2].url,
    files[5].url,
  ];
  artifact.resources.materials = [files[0].url, files[1].url];
  artifact.resources.collision = [files[3].url];
  artifact.resources.rules = [files[4].url];
  if (profile === "character") {
    artifact.resources.character = [files[6].url];
    artifact.resources.sky = [files[7].url];
  }
  return artifact;
}

test("player profiles resolve explicit runtime feature dependencies in deterministic order", () => {
  assert.deepEqual(playerProfileFeatures("empty"), ["render"]);
  assert.deepEqual(playerFeatureClosure(["character", "interaction", "sky"]), [
    "render",
    "materials",
    "collision",
    "controller",
    "rules",
    "interaction",
    "character",
    "sky",
  ]);
  assert.deepEqual(
    playerFeatureClosure(["character", "character"]),
    playerFeatureClosure(["character"]),
  );
  const room = playerProfileFeatures("room");
  assert.ok(
    room.includes("controller") &&
      room.includes("collision") &&
      room.includes("rules"),
  );
  assert.ok(!room.includes("character") && !room.includes("sky"));
  assert.throws(
    () => playerFeatureClosure(["scripts" as PlayerFeature]),
    /supported/,
  );
  assert.throws(
    () => playerProfileFeatures("unknown" as PlayerProfile),
    /profile/,
  );
});

test("all three profiles validate copied candidates and reuse the cooked descriptor identity", async () => {
  for (const profile of ["empty", "room", "character"] as const) {
    const input = fixture(profile),
      checked = await verifyPlayerArtifact(input);
    assert.deepEqual(checked, input);
    checked.features.length = 0;
    assert.ok(input.features.length > 0);
    if (checked.content) {
      checked.content.package.cells[0].bounds.min[0] = -99;
      checked.resources.collision!.length = 0;
      assert.equal(input.content!.package.cells[0].bounds.min[0], -8);
      assert.equal(input.resources.collision!.length, 1);
    }
  }
  const pendingInput = fixture(),
    pending = verifyPlayerArtifact(pendingInput);
  pendingInput.content!.package.files[0].sha256 = hash("changed while hashing");
  assert.notEqual(
    (await pending).content!.package.files[0].sha256,
    pendingInput.content!.package.files[0].sha256,
  );
});

test("incompatible profiles, runtime pins, URLs, and unsupported fields fail before activation", () => {
  const invalid: ((artifact: any) => void)[] = [
    (a) => {
      a.schema = "aethyrforge.release.v1";
    },
    (a) => {
      a.abi = 2;
    },
    (a) => {
      a.profile = "town";
    },
    (a) => {
      a.worldId = "../world";
    },
    (a) => {
      a.sourceIdentity = "wrong";
    },
    (a) => {
      a.runtime.abi = 2;
    },
    (a) => {
      a.runtime.bytes = Infinity;
    },
    (a) => {
      a.runtime.bytes = 0;
    },
    (a) => {
      a.runtime.bytes = 64 * 1024 * 1024 + 1;
    },
    (a) => {
      a.runtime.sha256 = "F".repeat(64);
    },
    (a) => {
      a.runtime.url = "https://other.test/player.js";
    },
    (a) => {
      a.runtime.url = "//other.test/player.js";
    },
    (a) => {
      a.runtime.url = "/assets/../player.js";
    },
    (a) => {
      a.runtime.url = "/assets/%2e%2e/player.js";
    },
    (a) => {
      a.runtime.url = "/assets/player.js?token=secret";
    },
    (a) => {
      a.kernel.url = "/kernel.js";
    },
    (a) => {
      a.runtime.token = "unused";
    },
    (a) => {
      a.features.pop();
    },
    (a) => {
      a.features.push(a.features[0]);
    },
    (a) => {
      a.features.push("shader-editor");
    },
    (a) => {
      a.extra = true;
    },
    (a) => {
      delete a.kernel;
    },
    (a) => {
      delete a.content;
    },
  ];
  const active = fixture(),
    initial = structuredClone(active);
  for (const change of invalid) {
    const candidate = structuredClone(active);
    change(candidate);
    assert.throws(() => validatePlayerArtifact(candidate));
    assert.deepEqual(active, initial);
  }
  for (const key of ["kernel", "content"] as const) {
    const candidate = fixture("empty");
    Object.assign(candidate, { [key]: fixture()[key] });
    assert.throws(() => validatePlayerArtifact(candidate), /profile/);
  }
});

test("feature resource declarations cover the cooked inventory and first-play collision, rules, and world settings", () => {
  const cases: ((a: PlayerArtifact) => void)[] = [
    (a) => {
      delete a.resources.controller;
    },
    (a) => {
      a.resources.sky = [];
    },
    (a) => {
      a.resources.render!.push("/starter/missing.bin");
    },
    (a) => {
      a.resources.render!.push(a.resources.render![0]);
    },
    (a) => {
      a.resources.render!.pop();
    },
    (a) => {
      a.resources.collision = [];
    },
    (a) => {
      a.resources.rules = [];
    },
    (a) => {
      a.content!.entryCell = "other";
    },
    (a) => {
      a.content!.worldURL = "https://other.test/world.json";
    },
    (a) => {
      a.content!.worldURL = "/starter/missing-world.json";
    },
    (a) => {
      a.content!.package.cells[0].resources =
        a.content!.package.cells[0].resources.filter(
          (f) => f.url !== a.content!.worldURL,
        );
    },
    (a) => {
      a.content!.package.cells[0].resources =
        a.content!.package.cells[0].resources.filter(
          (f) => f.url !== a.resources.collision![0],
        );
    },
  ];
  for (const change of cases) {
    const candidate = fixture();
    change(candidate);
    assert.throws(() => validatePlayerArtifact(candidate));
  }
});

test("exact runtime and kernel identities are required even when the ABI remains compatible", () => {
  const artifact = fixture();
  assert.deepEqual(
    validatePlayerArtifact(artifact, {
      profile: "room",
      worldId: artifact.worldId,
      runtime: artifact.runtime,
      kernel: artifact.kernel,
    }),
    artifact,
  );
  for (const compatibility of [
    { profile: "character" as const },
    { worldId: "another-world" },
    { runtime: { abi: 1 as const, sha256: hash("new runtime") } },
    { kernel: { abi: 1 as const, sha256: hash("new kernel") } },
  ])
    assert.throws(
      () => validatePlayerArtifact(artifact, compatibility),
      /pinned/,
    );
});

test("cooked descriptor tampering fails cryptographic verification, including collision bounds", async () => {
  const cases: ((a: PlayerArtifact) => void)[] = [
    (a) => {
      a.content!.package.cells[0].bounds.min[0]--;
    },
    (a) => {
      a.content!.package.sha256 = hash("wrong descriptor");
    },
    (a) => {
      a.content!.package.cells[0].quality = "staged-flat-outside-entry";
    },
  ];
  for (const change of cases) {
    const candidate = fixture();
    change(candidate);
    validatePlayerArtifact(candidate);
    await assert.rejects(verifyPlayerArtifact(candidate), /descriptor.*hash/);
  }
});
