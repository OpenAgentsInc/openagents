import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

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
    issueRefs: ["github:OpenAgentsInc/openagents#6033"],
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
    issueRefs: ["github:OpenAgentsInc/openagents#6033"],
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
