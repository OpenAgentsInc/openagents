import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { build } from "vite";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";
import { Kernel } from "../apps/preview/kernel.ts";
import {
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
  EnvironmentProfiles,
} from "../packages/runtime/environment.ts";
import * as legacyEnvironment from "../packages/editor-services/protocol.ts";
import { EnvironmentProfiles as LegacyProfiles } from "../packages/editor-services/environment.ts";
import {
  checkMaterialDocument,
  savedMaterialDocuments,
} from "../packages/runtime/material-source.ts";
import * as legacyMaterials from "../packages/editor-services/creator-material.ts";
import {
  buildRendererShaders,
  MATERIAL,
  SKY,
} from "../packages/renderer/shaders.ts";
import { selectedRendererFeatures as fullFeatures } from "../packages/renderer/player-features.ts";
import { roomRendererFeatures } from "../packages/renderer/room-features.ts";

test("editor compatibility exports share pure runtime environment and material owners", () => {
  assert.equal(legacyEnvironment.DEFAULT_ENVIRONMENT, DEFAULT_ENVIRONMENT);
  assert.equal(legacyEnvironment.validateEnvironment, validateEnvironment);
  assert.equal(LegacyProfiles, EnvironmentProfiles);
  assert.equal(legacyMaterials.checkMaterialDocument, checkMaterialDocument);
  assert.equal(legacyMaterials.savedMaterialDocuments, savedMaterialDocuments);
  assert.deepEqual(
    validateEnvironment(DEFAULT_ENVIRONMENT),
    DEFAULT_ENVIRONMENT,
  );
});

test("feature specialization preserves full shaders and removes generated and dome sky from room shaders", () => {
  const full = buildRendererShaders(fullFeatures),
    room = buildRendererShaders(roomRendererFeatures);
  assert.equal(full.MATERIAL, MATERIAL);
  assert.equal(full.SKY, SKY);
  assert.match(full.MATERIAL, /fn generatedSky\(/);
  assert.match(full.SKY, /fn skyDomeDistance\(/);
  assert.equal(room.SKY, "");
  assert.doesNotMatch(
    room.COMMON + room.MATERIAL,
    /fn (generatedSky|skyDomeDistance|fbm)\(/,
  );
  assert.match(
    room.MATERIAL,
    /fn sky\(direction:vec3<f32>\)->vec3<f32>\{return vec3\(0\.0\);\}/,
  );
  assert.match(room.MATERIAL, /@vertex fn vs/);
  assert.match(room.MATERIAL, /@fragment fn fs/);
});

test("published transport and editor wrapper execute the same current Rust module and fixed steps", async () => {
  const bytes = await readFile(
    new URL("../public/kernel.wasm", import.meta.url),
  );
  const runtime = await RuntimeKernel.fromBytes(bytes),
    preview = await Kernel.fromModules("baseline", bytes);
  const requests: object[] = [
    {
      type: "init",
      spawn: { position: [0, 0, 3], yaw: 0, pitch: 0, mode: "walk" },
      entities: [
        {
          id: "barrier",
          kind: "box",
          position: [0, 0, 0],
          yaw: 0,
          scale: 1,
          collidable: true,
          halfExtents: [1, 1, 0.5],
        },
      ],
    },
    ...Array.from({ length: 120 }, () => ({
      type: "step",
      elapsed: 1 / 60,
      input: { forward: 1 },
    })),
    { type: "snapshot" },
  ];
  for (const request of requests)
    assert.deepEqual(runtime.request(request), preview.request(request));
  const snapshot = runtime.request<any>({ type: "snapshot" });
  assert.ok(
    snapshot.player.position[2] > 0.5,
    "The shared controller remains blocked by the same collision.",
  );
  assert.equal(runtime.memoryBytes, preview.benchmarkInfo().authorWasmBytes);
  const measured = runtime.benchmarkInfo();
  assert.equal(measured.authorCalls, requests.length + 1);
  assert.equal(measured.moduleBytes, bytes.length);
  assert.ok(measured.compileMs >= 0 && measured.instantiateMs >= 0);
  await assert.rejects(
    RuntimeKernel.fromBytes(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])),
    /compatible/,
  );
});

test("emitted renderer graphs omit editor/experiments and remove character and sky implementations from room", async () => {
  const root = fileURLToPath(new URL("..", import.meta.url));
  const featurePath = resolve(root, "packages/renderer/features.ts");
  for (const profile of ["room", "player"] as const) {
    const result = await build({
      configFile: false,
      logLevel: "silent",
      plugins: [
        {
          name: "test-renderer-feature-profile",
          enforce: "pre",
          resolveId(source, importer) {
            if (importer && resolve(dirname(importer), source) === featurePath)
              return resolve(root, `packages/renderer/${profile}-features.ts`);
          },
        },
      ],
      build: {
        write: false,
        minify: false,
        target: "esnext",
        lib: {
          entry: resolve(root, "packages/renderer/renderer.ts"),
          formats: ["es"],
          fileName: "renderer",
        },
      },
    });
    const bundles = Array.isArray(result) ? result : [result];
    const chunks = bundles
      .flatMap((bundle) => ("output" in bundle ? bundle.output : []))
      .filter((item) => item.type === "chunk");
    assert.ok(chunks.length);
    // Match package-relative paths: the package itself lives under apps/.
    const modules = chunks.flatMap((chunk) =>
      Object.keys(chunk.modules).map((id) =>
        id.startsWith(root) ? id.slice(root.length - 1) : id,
      ),
    );
    assert.equal(
      modules.some((id) => /\/(editor-services|experiments|apps)\//.test(id)),
      false,
      `${profile} contains editor or experiment modules: ${modules.filter((id) => /\/(editor-services|experiments|apps)\//.test(id)).join(", ")}`,
    );
    const code = chunks.map((chunk) => chunk.code).join("\n");
    if (profile === "room") {
      assert.equal(
        modules.some((id) =>
          /\/(character-renderer|character|sky-shader|sky)\.ts$/.test(id),
        ),
        false,
      );
      assert.doesNotMatch(
        code,
        /fn generatedSky\(|fn skyDomeDistance\(|struct Joint \{/,
      );
    } else {
      assert.ok(modules.some((id) => id.endsWith("/character-renderer.ts")));
      assert.ok(modules.some((id) => id.endsWith("/sky-shader.ts")));
      assert.match(code, /fn generatedSky\(/);
      assert.match(code, /struct Joint \{/);
    }
  }
});
