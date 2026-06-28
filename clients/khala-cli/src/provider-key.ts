import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { traceTokenPath } from "./token-store.js"

// BYOK (bring-your-own-key) provider-key store.
//
// Lets a user attach their own upstream provider API key so their Khala usage
// runs against their own provider account. The key is a secret: it lives only
// in a 0600 file beside the agent token, is never printed in full, and is only
// ever transmitted over TLS in the BYOK request header.

export const BYOK_PROVIDERS = [
  "openrouter",
  "openai",
  "anthropic",
  "fireworks",
  "google",
  "groq",
  "together",
  "deepseek",
  "xai",
  "mistral",
] as const

export type ByokProvider = (typeof BYOK_PROVIDERS)[number]

export const PROVIDER_KEY_SCHEMA = "openagents.khala.provider_key.v0.1"

export interface StoredProviderKey {
  readonly schemaVersion: string
  readonly provider: ByokProvider
  readonly key: string
  readonly addedAt: string
}

// Friendly aliases map onto the canonical provider id.
const PROVIDER_ALIASES: Readonly<Record<string, ByokProvider>> = {
  "open-router": "openrouter",
  openrouter: "openrouter",
  openai: "openai",
  "open-ai": "openai",
  gpt: "openai",
  anthropic: "anthropic",
  claude: "anthropic",
  fireworks: "fireworks",
  google: "google",
  gemini: "google",
  "google-gemini": "google",
  groq: "groq",
  together: "together",
  togetherai: "together",
  deepseek: "deepseek",
  xai: "xai",
  grok: "xai",
  mistral: "mistral",
}

export function normalizeProviderName(value: string | undefined): ByokProvider | undefined {
  const normalized = value?.trim().toLowerCase()
  if (normalized === undefined || normalized.length === 0) return undefined
  return PROVIDER_ALIASES[normalized]
}

// Shape gate only (mirrors the server's provider-account key gate): keys are
// otherwise opaque. The value is never logged or echoed.
export function validateProviderKeyShape(value: string | undefined): string {
  const key = value?.trim() ?? ""
  if (key.length < 8 || key.length > 512 || /\s/.test(key)) {
    throw new Error("That does not look like an API key. Pass the full provider key with no spaces.")
  }
  return key
}

export function providerKeyPath(env: Record<string, string | undefined>): string {
  return join(dirname(traceTokenPath(env)), "provider-key.json")
}

export async function readProviderKey(
  env: Record<string, string | undefined>,
): Promise<StoredProviderKey | undefined> {
  try {
    const raw = await readFile(providerKeyPath(env), "utf8")
    const parsed = JSON.parse(raw) as Partial<StoredProviderKey>
    const provider = normalizeProviderName(parsed.provider)
    const key = typeof parsed.key === "string" ? parsed.key.trim() : ""
    if (provider === undefined || key.length === 0) return undefined
    return {
      schemaVersion: typeof parsed.schemaVersion === "string" ? parsed.schemaVersion : PROVIDER_KEY_SCHEMA,
      provider,
      key,
      addedAt: typeof parsed.addedAt === "string" ? parsed.addedAt : new Date(0).toISOString(),
    }
  } catch {
    return undefined
  }
}

export async function writeProviderKey(
  env: Record<string, string | undefined>,
  input: { readonly provider: ByokProvider; readonly key: string },
): Promise<StoredProviderKey> {
  const record: StoredProviderKey = {
    schemaVersion: PROVIDER_KEY_SCHEMA,
    provider: input.provider,
    key: input.key,
    addedAt: new Date().toISOString(),
  }
  const path = providerKeyPath(env)
  await mkdir(dirname(path), { mode: 0o700, recursive: true })
  await writeFile(path, `${JSON.stringify(record, null, 2)}\n`, { mode: 0o600 })
  return record
}

export async function clearProviderKey(env: Record<string, string | undefined>): Promise<boolean> {
  try {
    await rm(providerKeyPath(env))
    return true
  } catch {
    return false
  }
}

// Redacted display: never show more than the last 4 characters of the key.
export function redactProviderKey(key: string): string {
  const trimmed = key.trim()
  if (trimmed.length <= 4) return "****"
  return `...${trimmed.slice(-4)}`
}

export function describeStoredProviderKey(record: StoredProviderKey): string {
  return `${record.provider} · ${redactProviderKey(record.key)}`
}
