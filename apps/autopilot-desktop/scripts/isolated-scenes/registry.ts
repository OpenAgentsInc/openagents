import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import type { SceneRenderSignature } from "./render-gate.js"

export type IsolatedSceneName = "verse-arc" | "pylon-network"

export type IsolatedSceneDefinition = Readonly<{
  name: IsolatedSceneName
  title: string
  description: string
  entryModulePath: string
  defaultWidth: number
  defaultHeight: number
  defaultFrameSteps: number
  defaultFrameDeltaMs: number
  renderSignature: SceneRenderSignature
  issueRefs: ReadonlyArray<string>
}>

const here = dirname(fileURLToPath(import.meta.url))
const appRoot = join(here, "../..")

export const isolatedSceneDefinitions: ReadonlyArray<IsolatedSceneDefinition> = [
  {
    name: "verse-arc",
    title: "Verse Crackling Arc",
    description:
      "Evidence-bound Khala crackling arc rendered through the shared Verse spawned-scene layer.",
    entryModulePath: join(appRoot, "scripts/isolated-scenes/verse-arc-entry.ts"),
    defaultWidth: 960,
    defaultHeight: 540,
    defaultFrameSteps: 120,
    defaultFrameDeltaMs: 16,
    renderSignature: {
      label: "Verse crackling arc bright center-left band",
      minBrightPixels: 300,
      minDistinctLumaBuckets: 3,
      region: { x0: 0.28, y0: 0.22, x1: 0.62, y1: 0.58 },
    },
    issueRefs: [
      "github:OpenAgentsInc/openagents#6033",
      "github:OpenAgentsInc/openagents#6047",
    ],
  },
  {
    name: "pylon-network",
    title: "Pylon Network",
    description:
      "Synthetic pylon station network rendered through the desktop pylon network visualization mapper.",
    entryModulePath: join(appRoot, "scripts/isolated-scenes/pylon-network-entry.ts"),
    defaultWidth: 960,
    defaultHeight: 540,
    defaultFrameSteps: 90,
    defaultFrameDeltaMs: 16,
    renderSignature: {
      label: "Pylon network central station band",
      minBrightPixels: 500,
      minDistinctLumaBuckets: 4,
      region: { x0: 0.18, y0: 0.18, x1: 0.82, y1: 0.82 },
    },
    issueRefs: [
      "github:OpenAgentsInc/openagents#6033",
      "github:OpenAgentsInc/openagents#6047",
    ],
  },
]

export const isolatedSceneNames = (): ReadonlyArray<IsolatedSceneName> =>
  isolatedSceneDefinitions.map((definition) => definition.name)

export const findIsolatedSceneDefinition = (
  name: string,
): IsolatedSceneDefinition | null =>
  isolatedSceneDefinitions.find((definition) => definition.name === name) ?? null

export const isolatedSceneUsage = (): string =>
  `Known scenes: ${isolatedSceneNames().join(", ")}`
