/**
 * MC-6: Tailnet auto-auth handoff, pure discovery logic.
 *
 * Owner mandate (2026-07-04): "IF THERES A DEVICE ON TAILNET THATS AUTHED,
 * USE THAT AUTOMATICALLY - NO LOGIN SCREEN." When a desktop Khala Code
 * instance is signed in and reachable on the same Tailnet, the mobile app
 * should discover it and pull working Khala Sync credentials from it — zero
 * manual typing in the common case.
 *
 * This mirrors the shape of `../status/khala-code-connectivity-core.ts`
 * (MC-5): pure, dependency-injected, free of any native/expo import so it
 * stays unit-testable under `bun test`. The native-touching wrapper is
 * `./khala-mobile-pairing.ts`.
 *
 * The desktop endpoint this probes is
 * `clients/khala-code-desktop/src/bun/index.ts` `tailnetMobilePairingFetch`,
 * served on the SAME 0.0.0.0-bound Tailnet health beacon as `/health`
 * (`KHALA_CODE_TAILNET_HEALTH_PORT`) — Tailscale's own network ACL is the
 * real security boundary (only devices already authorized on this tailnet
 * can reach the port at all), so this stays a narrowly-scoped credential
 * read rather than a second auth layer.
 */

import {
  KHALA_CODE_TAILNET_CANDIDATE_HOSTS,
  KHALA_CODE_TAILNET_HEALTH_PORT,
} from "../status/khala-code-connectivity-core"

export const KHALA_MOBILE_PAIRING_PATH = "/khala-mobile-pairing"

/** Short per-host timeout so the common "nothing on the Tailnet" case fails
 * fast instead of hanging the launch screen. Candidates are probed
 * concurrently (see `discoverKhalaMobilePairingCredentials`), so total worst-
 * case wait is one timeout, not one timeout per candidate host. */
const PER_HOST_TIMEOUT_MS = 1500

export type KhalaMobilePairingCredentials = Readonly<{
  ownerUserId: string
  token: string
}>

export type KhalaMobilePairingProbeOutcome =
  | Readonly<{ state: "paired"; credentials: KhalaMobilePairingCredentials; hostname: string | null }>
  | Readonly<{ state: "reachable_not_signed_in"; hostname: string | null }>
  | Readonly<{ state: "unreachable" }>

export type PairingFetchLike = (
  url: string,
  init: { signal: AbortSignal },
) => Promise<{ ok: boolean; json: () => Promise<unknown> }>

export const khalaMobilePairingTargets = (
  isDevice: boolean,
  port: number = KHALA_CODE_TAILNET_HEALTH_PORT,
  tailnetHosts: ReadonlyArray<string> = KHALA_CODE_TAILNET_CANDIDATE_HOSTS,
): ReadonlyArray<string> =>
  isDevice
    ? tailnetHosts.map(host => `http://${host}:${port}${KHALA_MOBILE_PAIRING_PATH}`)
    // Simulator shares the host Mac's network stack, so localhost reaches
    // whatever is running the desktop app on this same machine.
    : [`http://127.0.0.1:${port}${KHALA_MOBILE_PAIRING_PATH}`]

const probePairingUrl = async (
  url: string,
  timeoutMs: number,
  fetchImpl: PairingFetchLike,
): Promise<KhalaMobilePairingProbeOutcome> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetchImpl(url, { signal: controller.signal })
    if (!response.ok) return { state: "unreachable" }
    const body = (await response.json()) as {
      hostname?: unknown
      ok?: unknown
      ownerUserId?: unknown
      token?: unknown
    }
    const hostname = typeof body.hostname === "string" ? body.hostname : null
    if (body.ok !== true) return { state: "reachable_not_signed_in", hostname }
    const ownerUserId = typeof body.ownerUserId === "string" ? body.ownerUserId.trim() : ""
    const token = typeof body.token === "string" ? body.token.trim() : ""
    if (ownerUserId.length === 0 || token.length === 0) {
      return { state: "reachable_not_signed_in", hostname }
    }
    return { state: "paired", credentials: { ownerUserId, token }, hostname }
  } catch {
    return { state: "unreachable" }
  } finally {
    clearTimeout(timer)
  }
}

/**
 * Probes every candidate concurrently (never serially — a serial scan would
 * multiply the timeout by candidate count and make the common "nothing
 * signed in" case feel like a hang) and returns the best outcome found:
 * a real credential pair beats "reachable but not signed in", which beats
 * "nothing reachable at all". Ties among multiple paired hosts resolve to
 * the first candidate in list order, matching the deterministic priority
 * `KHALA_CODE_TAILNET_CANDIDATE_HOSTS` already documents.
 */
export const discoverKhalaMobilePairingCredentials = async (
  targets: ReadonlyArray<string>,
  fetchImpl: PairingFetchLike,
  timeoutMs: number = PER_HOST_TIMEOUT_MS,
): Promise<KhalaMobilePairingProbeOutcome> => {
  if (targets.length === 0) return { state: "unreachable" }
  const outcomes = await Promise.all(
    targets.map(url => probePairingUrl(url, timeoutMs, fetchImpl)),
  )
  const paired = outcomes.find((outcome): outcome is Extract<KhalaMobilePairingProbeOutcome, { state: "paired" }> =>
    outcome.state === "paired")
  if (paired !== undefined) return paired
  const reachable = outcomes.find((
    outcome,
  ): outcome is Extract<KhalaMobilePairingProbeOutcome, { state: "reachable_not_signed_in" }> =>
    outcome.state === "reachable_not_signed_in")
  if (reachable !== undefined) return reachable
  return { state: "unreachable" }
}
