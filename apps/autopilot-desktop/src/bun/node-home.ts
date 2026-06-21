import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { readControlToken } from "./pylon-control.js"

// CL-45: discover the local Pylon node's home directory.
//
// The desktop launches from apps/autopilot-desktop, but the running node keeps
// its control token under a home like `.pylon-tailnet` or `.pylon-local` at the
// openagents repo root. Defaulting PYLON_HOME to `<cwd>/.pylon-local` made the
// app report a false "offline" whenever the live home was `.pylon-tailnet`.
//
// Discovery order:
//   1. an explicit PYLON_HOME env (highest priority — operator override),
//   2. then, walking up from cwd, each ancestor's known home subdir
//      (`.pylon-tailnet`, then `.pylon-local`),
//   3. then the canonical homes the standalone `pylon` runtime actually uses
//      (`apps/pylon/src/bootstrap.ts` `selectPylonHomeResolution`): the nested
//      `~/.openagents/pylon` first (the historical-config identity home a live
//      node uses), then bare `~/.pylon`.
//
// Returns the chosen home directory (the dir that directly contains
// `control-token`), or null when no readable token is found anywhere.
//
// CL-45b (control-401 fix): `discoverPylonHome` first-matches the FIRST home
// with a *readable* token. On a real machine a stale `<repo>/.pylon-tailnet/
// control-token` can shadow the canonical `~/.openagents/pylon/control-token`
// the running control server actually accepts, dead-ending auth at `control
// 401`. For control-plane CALLS, resolve through `resolveAcceptedControlToken`
// instead: it probes candidate-home tokens against the live server and uses the
// first one the server ACCEPTS (non-401), so a stale token no longer dead-ends.

export const KNOWN_HOME_SUBDIRS = [".pylon-tailnet", ".pylon-local"] as const

export type DiscoverPylonHomeOptions = {
  readonly env?: string | undefined
  readonly cwd: string
  // The user home dir; injectable for tests. Defaults to the real `homedir()`.
  readonly homeDir?: string | undefined
  // Injectable for tests; defaults to the real control-token reader.
  readonly readToken?: (home: string) => string | null
}

function ancestors(cwd: string): string[] {
  const out: string[] = []
  let cur = cwd
  // Guard against an unbounded loop on a malformed path; repo trees are shallow.
  for (let i = 0; i < 64; i++) {
    out.push(cur)
    const parent = dirname(cur)
    if (parent === cur) break
    cur = parent
  }
  return out
}

// The canonical homes the standalone `pylon` runtime uses for its control
// token, matched exactly to `selectPylonHomeResolution` in
// `apps/pylon/src/bootstrap.ts`: nested `~/.openagents/pylon` (preferred — the
// historical-config identity home a live node uses) then bare `~/.pylon`.
export function canonicalPylonHomes(homeDir: string): string[] {
  return [join(homeDir, ".openagents", "pylon"), join(homeDir, ".pylon")]
}

export function pylonHomeCandidates(opts: DiscoverPylonHomeOptions): string[] {
  const candidates: string[] = []
  if (opts.env && opts.env.length > 0) candidates.push(opts.env)
  for (const anc of ancestors(opts.cwd)) {
    for (const sub of KNOWN_HOME_SUBDIRS) candidates.push(join(anc, sub))
  }
  // The canonical standalone-pylon homes go last so an explicit env override and
  // an in-tree managed home still win the ordering, but discovery no longer
  // misses the home the running control server actually authenticates against.
  for (const home of canonicalPylonHomes(opts.homeDir ?? homedir())) {
    candidates.push(home)
  }
  // De-dup while preserving order.
  return candidates.filter((c, i) => candidates.indexOf(c) === i)
}

export function discoverPylonHome(opts: DiscoverPylonHomeOptions): string | null {
  const readToken = opts.readToken ?? readControlToken
  for (const home of pylonHomeCandidates(opts)) {
    const token = readToken(home)
    if (token !== null && token.length > 0) return home
  }
  return null
}

// ── CL-45b: server-validated control-token resolution ─────────────────────
//
// `discoverPylonHome` first-matches on token *readability*, which dead-ends
// auth when an earlier candidate home holds a stale token the live control
// server rejects (401). For control-plane CALLS we instead probe each candidate
// home's token against the running server and use the first one the server
// ACCEPTS. The result is cached so we don't probe on every command.

export type AcceptedControlToken = {
  readonly home: string
  readonly token: string
}

export type ResolveAcceptedControlTokenOptions = {
  readonly env?: string | undefined
  readonly cwd: string
  readonly homeDir?: string | undefined
  // Injectable for tests; defaults to the real control-token reader.
  readonly readToken?: (home: string) => string | null
  // Probe a single candidate token against the live control server.
  // Returns true when the server ACCEPTS it (any non-401 response), false on a
  // 401 reject. Bounded + side-effect-light: one cheap authenticated request.
  readonly probe: (token: string) => Promise<boolean>
}

// Resolve the first candidate-home control token the live server accepts.
// Walks the SAME ordered candidate list as discovery, but instead of stopping
// at the first *readable* token it stops at the first *accepted* one. Returns
// null when no candidate home has a token the server accepts (or none exist).
export async function resolveAcceptedControlToken(
  opts: ResolveAcceptedControlTokenOptions,
): Promise<AcceptedControlToken | null> {
  const readToken = opts.readToken ?? readControlToken
  const seen = new Set<string>()
  for (const home of pylonHomeCandidates({
    env: opts.env,
    cwd: opts.cwd,
    homeDir: opts.homeDir,
    readToken,
  })) {
    const token = readToken(home)
    if (token === null || token.length === 0) continue
    // Don't re-probe an identical token shared across two candidate homes.
    if (seen.has(token)) continue
    seen.add(token)
    if (await opts.probe(token)) return { home, token }
  }
  return null
}

// A small cache so the control-call path doesn't re-probe candidate homes on
// every command. We re-validate the cached token before reuse (cheap, and it
// catches a rotated/expired token), and re-resolve from scratch if it stops
// being accepted. Keyed by (env, cwd, homeDir) so a changed override re-resolves.
export type ControlTokenResolver = {
  resolve(): Promise<AcceptedControlToken | null>
  // Drop the cached token (e.g. after a control call still 401s downstream).
  invalidate(): void
}

export function createControlTokenResolver(
  optsFactory: () => ResolveAcceptedControlTokenOptions,
): ControlTokenResolver {
  let cached: { key: string; value: AcceptedControlToken } | null = null

  const keyFor = (o: ResolveAcceptedControlTokenOptions): string =>
    `${o.env ?? ""} ${o.cwd} ${o.homeDir ?? ""}`

  return {
    async resolve() {
      const opts = optsFactory()
      const key = keyFor(opts)
      if (cached !== null && cached.key === key) {
        // Re-validate the cached token; reuse it only while still accepted.
        if (await opts.probe(cached.value.token)) return cached.value
        cached = null
      }
      const resolved = await resolveAcceptedControlToken(opts)
      if (resolved !== null) cached = { key, value: resolved }
      return resolved
    },
    invalidate() {
      cached = null
    },
  }
}
