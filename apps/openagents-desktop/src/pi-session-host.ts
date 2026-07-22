/**
 * Seven-agents (#9183): the Pi IN-PROCESS host session-factory seam.
 *
 * Pi (`@earendil-works/pi-coding-agent`) is NOT a subprocess and NOT an ACP/HTTP
 * peer — `createAgentSession` builds a full agent session INSIDE the caller's
 * process. So, unlike Goose (`goose acp` stdio) and OpenCode (`opencode serve`
 * HTTP/SSE), there is no live-transport spawner to drive: the desktop must own
 * the in-process session factory itself. This module is that host seam. It
 * constructs an owner-local {@link PiSessionFactory} that the SDK's dependency-
 * free `makePiHarnessAdapter` drives for a real turn, exactly the injection
 * point the adapter's structural `PiSessionSurface`/`PiCreateSessionOptions`
 * contract was designed for.
 *
 * Owner-local by construction, and honest about readiness:
 *  - The Pi library is an OPTIONAL desktop dependency, loaded through a
 *    NON-LITERAL dynamic import so a missing/incompatible package degrades this
 *    lane to `unavailable` (never a desktop-wide failure, and never a hard
 *    typecheck dependency on the heavy full-agent tree).
 *  - The model auth is the owner's live Gemini key, resolved in-process only
 *    from `GEMINI_API_KEY` or the developer's opencode auth store — never
 *    printed, never persisted anywhere else. No key ⇒ honest `unavailable`.
 *  - The agent directory is an ISOLATED per-account directory the desktop owns,
 *    NEVER the owner's live `~/.pi` tree (the adapter refuses a `.pi` segment,
 *    the Pi analogue of the `pylon auth` Codex-home isolation rule).
 *  - The default model is pinned to the owner's preference (gemini-3.6-flash)
 *    through Pi's OWN `settings.json` `defaultModel` — not an invented config
 *    field, exactly what `createAgentSession` reads.
 *
 * This module NEVER runs an install command, NEVER changes PATH, and NEVER runs
 * a login flow. It only constructs the in-process session; a turn is reachable
 * only when the library resolves AND an owner-local key is present.
 */

import type {
  PiCreateSessionOptions,
  PiSessionFactory,
  PiSessionSurface,
} from "@openagentsinc/agent-harness-contract"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

/** Owner preference (earlier session convention): gemini models use this. */
export const PI_DEFAULT_MODEL = "gemini-3.6-flash" as const

/** The optional Pi library specifier — read through a variable so `tsc`/rollup
 * never statically resolve it (the optional-dependency pattern). */
const PI_MODULE_SPECIFIER = "@earendil-works/pi-coding-agent"

/**
 * The narrow slice of `@earendil-works/pi-coding-agent` this seam consumes. The
 * dynamic import is cast to this shape (the live SDK smoke uses the same cast),
 * so the heavy Pi type tree never enters the desktop typecheck and a version
 * skew fails only this lane at runtime.
 */
interface PiCodingAgentModule {
  readonly createAgentSession: (options: {
    readonly agentDir?: string
    readonly cwd?: string
    readonly tools?: ReadonlyArray<string>
    readonly customTools?: ReadonlyArray<unknown>
  }) => Promise<{ readonly session: PiSessionSurface; readonly modelFallbackMessage?: string }>
}

export type PiSessionHostResolution =
  | Readonly<{ state: "ready"; createSession: PiSessionFactory; agentDir: string; model: string }>
  | Readonly<{ state: "unavailable"; reason: string }>

export type PiSessionHost = Readonly<{
  resolve: () => Promise<PiSessionHostResolution>
}>

/** Load the optional Pi library, degrading honestly when it is not installed. */
const defaultLoadModule = async (): Promise<PiCodingAgentModule> => {
  const specifier = PI_MODULE_SPECIFIER
  return (await import(specifier)) as unknown as PiCodingAgentModule
}

/**
 * Resolve an owner-local Gemini API key IN-PROCESS ONLY. Preference order:
 *  1. `GEMINI_API_KEY` already in the environment (owner-set).
 *  2. The developer's opencode auth store (`~/.local/share/opencode/auth.json`,
 *     provider `google`) — the same owner-local source the SDK's live Pi smoke
 *     reuses. Never printed, never copied elsewhere.
 */
const defaultLoadApiKey = (environment: Readonly<Record<string, string | undefined>>): string | null => {
  const fromEnv = environment.GEMINI_API_KEY
  if (typeof fromEnv === "string" && fromEnv.length > 0) return fromEnv
  return null
}

/** Best-effort async read of the opencode google key (kept off the hot path). */
const readOpencodeGoogleKey = async (): Promise<string | null> => {
  try {
    const raw = await readFile(join(homedir(), ".local", "share", "opencode", "auth.json"), "utf8")
    const parsed = JSON.parse(raw) as Record<string, { key?: unknown } | undefined>
    const key = parsed.google?.key
    return typeof key === "string" && key.length > 0 ? key : null
  } catch {
    return null
  }
}

/** Write Pi's own `settings.json` `defaultModel` into the isolated agent dir
 * (merging any existing settings), so the pinned model is honored without an
 * invented config field. Best-effort: a write failure is not fatal to detection
 * but is surfaced by the caller as the honest reason. */
const prepareAgentDir = async (agentDir: string, model: string): Promise<void> => {
  await mkdir(agentDir, { recursive: true })
  const settingsPath = join(agentDir, "settings.json")
  let existing: Record<string, unknown> = {}
  try {
    existing = JSON.parse(await readFile(settingsPath, "utf8")) as Record<string, unknown>
  } catch {
    existing = {}
  }
  if (existing.defaultModel === model) return
  await writeFile(settingsPath, `${JSON.stringify({ ...existing, defaultModel: model }, null, 2)}\n`, "utf8")
}

/**
 * Build the Pi in-process session host. `agentDir` MUST be an isolated
 * per-account directory (never the owner's live `~/.pi`); the caller derives it
 * under the desktop user-data root. All external effects are injectable so the
 * lane test drives the full seam (host → factory → `makePiHarnessAdapter` →
 * lowering) with a scripted library and no real key.
 */
export const makePiSessionHost = (input: Readonly<{
  agentDir: string
  resolveWorkspace: () => string
  model?: string
  environment?: Readonly<Record<string, string | undefined>>
  /** Injectable optional-library loader (defaults to the dynamic import). */
  loadModule?: () => Promise<PiCodingAgentModule>
  /** Injectable synchronous key resolver (env-first). */
  loadApiKey?: (environment: Readonly<Record<string, string | undefined>>) => string | null
  /** Injectable async key fallback (defaults to the opencode auth store). */
  loadApiKeyAsync?: () => Promise<string | null>
  /** Injectable agent-dir preparer (defaults to writing `settings.json`). */
  prepareAgentDir?: (agentDir: string, model: string) => Promise<void>
}>): PiSessionHost => {
  const environment = input.environment ?? process.env
  const model = input.model ?? PI_DEFAULT_MODEL
  const loadModule = input.loadModule ?? defaultLoadModule
  const loadApiKey = input.loadApiKey ?? defaultLoadApiKey
  const loadApiKeyAsync = input.loadApiKeyAsync ?? readOpencodeGoogleKey
  const prepare = input.prepareAgentDir ?? prepareAgentDir

  // Memoize ONLY a fully-ready resolution: installing Pi or configuring a key
  // after launch is picked up on the next capability refresh.
  let ready: Extract<PiSessionHostResolution, { state: "ready" }> | null = null

  const resolve = async (): Promise<PiSessionHostResolution> => {
    if (ready !== null) return ready

    // 1. Owner-local key (the honest "config detected" gate). Never printed.
    const apiKey = loadApiKey(environment) ?? (await loadApiKeyAsync())
    if (apiKey === null) {
      return {
        state: "unavailable",
        reason:
          "Pi needs an owner-local Gemini API key. Set GEMINI_API_KEY or configure the google provider (opencode auth).",
      }
    }

    // 2. The optional in-process Pi library (the honest "host constructs" gate).
    let mod: PiCodingAgentModule
    try {
      mod = await loadModule()
      if (typeof mod.createAgentSession !== "function") throw new Error("no createAgentSession export")
    } catch {
      return {
        state: "unavailable",
        reason:
          "The Pi in-process library (@earendil-works/pi-coding-agent) is not installed as a desktop dependency.",
      }
    }

    // 3. Isolated agent dir + pinned default model.
    try {
      await prepare(input.agentDir, model)
    } catch {
      return {
        state: "unavailable",
        reason: `Pi could not prepare its isolated agent directory (${input.agentDir}).`,
      }
    }

    const createSession: PiSessionFactory = async (
      options: PiCreateSessionOptions,
    ): Promise<PiSessionSurface> => {
      // Owner-local, in-process only: expose the resolved key to Pi's model
      // runtime for this process, never persisting it. Mirrors the SDK live smoke.
      if (environment === process.env) process.env.GEMINI_API_KEY = apiKey
      const created = await mod.createAgentSession({
        agentDir: options.agentDir,
        cwd: options.workspaceDir ?? input.resolveWorkspace(),
        ...(options.activeTools === undefined ? {} : { tools: [...options.activeTools] }),
        ...(options.customTools === undefined ? {} : { customTools: [...options.customTools] }),
      })
      return created.session
    }

    ready = { state: "ready", createSession, agentDir: input.agentDir, model }
    return ready
  }

  return { resolve }
}
