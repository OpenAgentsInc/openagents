import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { homedir } from "node:os"

import type { GrokSessionRef } from "./types.ts"

const SCHEMA = "khala-code.grok-sessions.v1"

type StoreFile = {
  schema: typeof SCHEMA
  sessions: Record<string, GrokSessionRef>
}

export type GrokSessionStore = {
  readonly path: string
  readonly get: (desktopSessionId: string) => Promise<GrokSessionRef | null>
  readonly put: (entry: GrokSessionRef) => Promise<GrokSessionRef>
  readonly list: () => Promise<readonly GrokSessionRef[]>
}

export function resolveGrokSessionStorePath(
  env: Readonly<Record<string, string | undefined>> = process.env,
): string {
  const override = env.KHALA_CODE_DESKTOP_GROK_STATE_PATH?.trim()
  if (override) return override
  const home = env.HOME?.trim() || homedir()
  return join(home, ".khala-code", "grok-sessions.json")
}

async function readStore(path: string): Promise<StoreFile> {
  try {
    const raw = await readFile(path, "utf8")
    const parsed = JSON.parse(raw) as StoreFile
    if (parsed.schema !== SCHEMA || typeof parsed.sessions !== "object") {
      return { schema: SCHEMA, sessions: {} }
    }
    return { schema: SCHEMA, sessions: { ...parsed.sessions } }
  } catch {
    return { schema: SCHEMA, sessions: {} }
  }
}

async function writeStore(path: string, store: StoreFile): Promise<void> {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(store, null, 2)}\n`, "utf8")
}

export function createGrokSessionStore(options: {
  readonly path?: string
  readonly env?: Readonly<Record<string, string | undefined>>
} = {}): GrokSessionStore {
  const path = options.path ?? resolveGrokSessionStorePath(options.env)

  return {
    path,
    async get(desktopSessionId) {
      const store = await readStore(path)
      return store.sessions[desktopSessionId] ?? null
    },
    async put(entry) {
      const store = await readStore(path)
      store.sessions[entry.desktopSessionId] = entry
      await writeStore(path, store)
      return entry
    },
    async list() {
      const store = await readStore(path)
      return Object.values(store.sessions)
    },
  }
}
