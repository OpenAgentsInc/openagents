import { dirname, join } from "node:path"
import { readControlToken } from "./pylon-control"

// CL-45: discover the local Pylon node's home directory.
//
// The desktop launches from apps/autopilot-desktop, but the running node keeps
// its control token under a home like `.pylon-tailnet` or `.pylon-local` at the
// openagents repo root. Defaulting PYLON_HOME to `<cwd>/.pylon-local` made the
// app report a false "offline" whenever the live home was `.pylon-tailnet`.
//
// Discovery order:
//   1. an explicit PYLON_HOME env (highest priority — operator override),
//   2. then, walking up from cwd, the first ancestor that has a readable
//      `control-token` under a known home subdir (`.pylon-tailnet`, then
//      `.pylon-local`).
//
// Returns the chosen home directory (the dir that directly contains
// `control-token`), or null when no readable token is found anywhere.

export const KNOWN_HOME_SUBDIRS = [".pylon-tailnet", ".pylon-local"] as const

export type DiscoverPylonHomeOptions = {
  readonly env?: string | undefined
  readonly cwd: string
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

export function pylonHomeCandidates(opts: DiscoverPylonHomeOptions): string[] {
  const candidates: string[] = []
  if (opts.env && opts.env.length > 0) candidates.push(opts.env)
  for (const anc of ancestors(opts.cwd)) {
    for (const sub of KNOWN_HOME_SUBDIRS) candidates.push(join(anc, sub))
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
