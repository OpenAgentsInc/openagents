import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { Schema as S } from "effect"

import type { KhalaCodeDesktopArchitectPlanArtifact } from "../shared/rpc.js"

const ARCHITECT_PLAN_STORE_SCHEMA = "khala-code-desktop.architect-plans.v1"

const StoreFile = S.Struct({
  schema: S.Literal(ARCHITECT_PLAN_STORE_SCHEMA),
  plans: S.Record(S.String, S.Unknown),
})

type StoreFile = typeof StoreFile.Type

type ChatEnv = Readonly<Record<string, string | undefined>>

export type ArchitectPlanStore = Readonly<{
  get: (
    sessionId: string,
    planRef: string,
  ) => Promise<KhalaCodeDesktopArchitectPlanArtifact | null>
  path: string
  put: (
    artifact: KhalaCodeDesktopArchitectPlanArtifact,
  ) => Promise<KhalaCodeDesktopArchitectPlanArtifact>
}>

export const resolveArchitectPlanStorePath = (env: ChatEnv): string => {
  const override = env.KHALA_CODE_DESKTOP_ARCHITECT_PLAN_STATE_PATH?.trim()
  if (override !== undefined && override.length > 0) return override
  const home = env.HOME?.trim() || homedir()
  return join(home, ".khala-code", "architect-plans.json")
}

const storeKey = (sessionId: string, planRef: string): string =>
  `${sessionId}:${planRef}`

const emptyStore = (): StoreFile => ({
  schema: ARCHITECT_PLAN_STORE_SCHEMA,
  plans: {},
})

const readStore = async (path: string): Promise<StoreFile> => {
  try {
    return S.decodeUnknownSync(StoreFile)(JSON.parse(await readFile(path, "utf8")))
  } catch {
    return emptyStore()
  }
}

const writeStore = async (path: string, store: StoreFile): Promise<void> => {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(store, null, 2)}\n`, "utf8")
}

export const createArchitectPlanStore = (
  input: { readonly env: ChatEnv; readonly path?: string },
): ArchitectPlanStore => {
  const path = input.path ?? resolveArchitectPlanStorePath(input.env)
  return {
    path,
    async get(sessionId, planRef) {
      const store = await readStore(path)
      const artifact = store.plans[storeKey(sessionId, planRef)]
      if (artifact === undefined) return null
      return artifact as KhalaCodeDesktopArchitectPlanArtifact
    },
    async put(artifact) {
      const store = await readStore(path)
      await writeStore(path, {
        schema: store.schema,
        plans: {
          ...store.plans,
          [storeKey(artifact.sessionId, artifact.planRef)]: artifact,
        },
      })
      return artifact
    },
  }
}
