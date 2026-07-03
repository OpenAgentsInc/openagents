import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import { Schema as S } from "effect"

const CLAUDE_SESSION_STORE_SCHEMA = "khala-code-desktop.claude-sessions.v1"
const CLAUDE_SESSION_ORIGIN = "khala-code-desktop"

const SessionEntry = S.Struct({
  sessionId: S.String,
  lastTurnId: S.optional(S.String),
  origin: S.optional(S.Literal(CLAUDE_SESSION_ORIGIN)),
  updatedAt: S.String,
})

const SessionStoreFile = S.Struct({
  schema: S.Literal(CLAUDE_SESSION_STORE_SCHEMA),
  sessions: S.Record(S.String, SessionEntry),
})

export type ClaudeDesktopSessionEntry = typeof SessionEntry.Type
type ClaudeSessionStoreFile = typeof SessionStoreFile.Type

export type ClaudeSessionStore = Readonly<{
  get: (desktopSessionId: string) => Promise<ClaudeDesktopSessionEntry | null>
  put: (desktopSessionId: string, entry: {
    readonly sessionId: string
    readonly lastTurnId?: string
  }) => Promise<ClaudeDesktopSessionEntry>
  path: string
}>

export type CreateClaudeSessionStoreOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly now?: () => Date
  readonly path?: string
}

const emptyStore = (): ClaudeSessionStoreFile => ({
  schema: CLAUDE_SESSION_STORE_SCHEMA,
  sessions: {},
})

export function resolveClaudeSessionStorePath(
  env: Readonly<Record<string, string | undefined>> = Bun.env,
): string {
  const override = env.KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH?.trim()
  if (override !== undefined && override.length > 0) return override
  const home = env.HOME?.trim() || homedir()
  return join(home, ".khala-code", "claude-sessions.json")
}

export function resolveClaudeConfigDir(
  env: Readonly<Record<string, string | undefined>> = Bun.env,
): string {
  const override = env.KHALA_CODE_DESKTOP_CLAUDE_CONFIG_DIR?.trim()
  if (override !== undefined && override.length > 0) return override
  const home = env.HOME?.trim() || homedir()
  return join(home, ".khala-code", "claude-config")
}

export async function ensureClaudeConfigDir(path: string): Promise<void> {
  await mkdir(path, { recursive: true })
}

async function readStore(path: string): Promise<ClaudeSessionStoreFile> {
  try {
    const raw = await readFile(path, "utf8")
    const decoded = S.decodeUnknownSync(SessionStoreFile)(JSON.parse(raw))
    return { schema: decoded.schema, sessions: { ...decoded.sessions } }
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    if (code === "ENOENT") return emptyStore()
    return emptyStore()
  }
}

async function writeStore(path: string, store: ClaudeSessionStoreFile): Promise<void> {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(store, null, 2)}\n`, "utf8")
}

export function createClaudeSessionStore(
  options: CreateClaudeSessionStoreOptions = {},
): ClaudeSessionStore {
  const path = options.path ?? resolveClaudeSessionStorePath(options.env)
  const now = options.now ?? (() => new Date())

  return {
    path,
    async get(desktopSessionId) {
      const store = await readStore(path)
      return store.sessions[desktopSessionId] ?? null
    },
    async put(desktopSessionId, entry) {
      const store = await readStore(path)
      const updated: ClaudeDesktopSessionEntry = {
        sessionId: entry.sessionId,
        ...(entry.lastTurnId === undefined ? {} : { lastTurnId: entry.lastTurnId }),
        origin: CLAUDE_SESSION_ORIGIN,
        updatedAt: now().toISOString(),
      }
      await writeStore(path, {
        schema: store.schema,
        sessions: { ...store.sessions, [desktopSessionId]: updated },
      })
      return updated
    },
  }
}
