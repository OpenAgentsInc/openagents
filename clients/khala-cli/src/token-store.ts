import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { homedir } from "node:os"
import { Effect } from "effect"
import { mintFreeKey, toKhalaCliError } from "./client.js"
import { DEFAULT_BASE_URL } from "./types.js"

export interface StoredTokenOptions {
  readonly baseUrl: string
  readonly env: Record<string, string | undefined>
  readonly explicitToken?: string | undefined
}

export function traceTokenPath(env: Record<string, string | undefined>): string {
  const override = env.KHALA_TOKEN_PATH?.trim()
  if (override) return override
  const configHome = env.XDG_CONFIG_HOME?.trim() || join(homedir(), ".config")
  return join(configHome, "khala", "agent-token")
}

export async function ensureStoredAgentToken(options: StoredTokenOptions): Promise<string> {
  const explicit = options.explicitToken?.trim()
  if (isAgentToken(explicit)) return explicit

  const stored = await readStoredAgentToken(options.env)
  if (stored !== undefined) return stored

  const minted = await Effect.runPromise(mintFreeKey({ baseUrl: options.baseUrl || DEFAULT_BASE_URL }))
    .catch(error => {
      throw toKhalaCliError(error, "Could not mint a Khala trace token.")
    })
  const token = minted.credential.token.trim()
  if (!isAgentToken(token)) {
    throw new Error("Free key response did not include an oa_agent_ token.")
  }
  await writeStoredAgentToken(options.env, token)
  return token
}

export async function readStoredAgentToken(env: Record<string, string | undefined>): Promise<string | undefined> {
  try {
    const token = (await readFile(traceTokenPath(env), "utf8")).trim()
    return isAgentToken(token) ? token : undefined
  } catch {
    return undefined
  }
}

async function writeStoredAgentToken(env: Record<string, string | undefined>, token: string): Promise<void> {
  const path = traceTokenPath(env)
  await mkdir(dirname(path), { mode: 0o700, recursive: true })
  await writeFile(path, `${token}\n`, { mode: 0o600 })
}

function isAgentToken(token: string | undefined): token is string {
  return token !== undefined && token.startsWith("oa_agent_")
}
