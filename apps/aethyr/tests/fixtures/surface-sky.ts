import type { SurfaceSkyManifest } from "../../packages/renderer/surface-sky.ts";
const H = "a".repeat(64);
export function surfaceFixture(): SurfaceSkyManifest {
  return {
    schema: "aethyrforge.surface-sky.v1",
    profile: "static-surface-split-sky-sh9-v1",
    selections: [
      {
        actor: "Owned plant",
        component: "Mesh",
        mesh: "owned-triangle",
        model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        materialIds: ["owned-leaf"],
        firstVertex: 0,
        vertexCount: 3,
        recordStart: 0,
      },
    ],
    vertexCount: 3,
    records: { strideFloats: 72, primaryOffset: 0, constantOffset: 36 },
    transport: {
      nearMeters: 10,
      farMeters: 5000,
      normalOffsetMeters: 0.002,
      rays: 512,
      alphaPolicy: "alpha-tested-material-v1",
      staticPose: true,
      lowerHemisphere: "world-up-black",
    },
    scenarios: {
      day: {
        url: "/private-assets/owned.bin",
        sha256: H,
        bytes: 3 * 72 * 4,
        sourceSceneSha256: H,
        sourceEnvironmentSha256: H,
      },
    },
    sourceBlobs: {
      day: ["vertices", "indices", "colors"].map((n) => ({
        url: "/private-assets/" + n + ".bin",
        sha256: H,
        bytes: 16,
      })),
    },
  };
}
