import type {
  SourceFogCard,
  SourceFogCardsManifest,
} from "../../packages/renderer/source-fog-cards.ts";
export const fogCardFixture = (): SourceFogCard => ({
  id: "owned card",
  model: [1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 0, -2, 1],
  tint: [1, 0.5, 0.25],
  brightness: 25,
  intensity: 1,
  intensityExponent: 1,
  detailExponent: 2,
  detailTiling: 0.3,
  thickestDistance: 12,
  transitionDistance: 18.88,
  depthFadeDistance: 10,
  fresnelExponent: 0.17,
  speed: [0.001, -0.0002],
});
export const fogManifestFixture = (): SourceFogCardsManifest => ({
  schema: "aethyrforge.source-fog-cards.v1",
  source: {
    factsSha256: "a".repeat(64),
    graphSha256: "b".repeat(64),
    planeSha256: "c".repeat(64),
    randomPolicy: "owned-seed-hash-v1",
  },
  textures: {
    mask: {
      url: "/private-assets/mask.bin",
      sha256: "d".repeat(64),
      bytes: 1,
      width: 1,
      height: 1,
      format: "r8unorm",
    },
    detail: {
      url: "/private-assets/detail.bin",
      sha256: "e".repeat(64),
      bytes: 1,
      width: 1,
      height: 1,
      format: "r8unorm",
    },
  },
  scenarios: { day: { cards: [fogCardFixture()] } },
});
