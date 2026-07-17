import { Schema } from "effect"

import {
  FullAutoRunMobileProjectionSchema,
  type FullAutoRunProjectionResult,
} from "./full-auto-run-projection"

/**
 * The one clearly-named module boundary #8982 asks for: everything above
 * this file (thread-selection priority, the state header) depends only on
 * `FullAutoRunProjectionSource`, never on how the projection is obtained.
 * Swapping in the real #8981 client (once it lands) means changing only this
 * file's `fetchFullAutoRunMobileProjection` body — or replacing it with the
 * real `@openagentsinc/khala-sync-client` helper #8981 publishes, mirroring
 * `fetchFleetRunClientProjection`.
 */
export type FullAutoRunProjectionSource = () => Promise<FullAutoRunProjectionResult>

const ResponseEnvelope = Schema.Struct({
  ok: Schema.Literal(true),
  fullAutoRun: Schema.NullOr(FullAutoRunMobileProjectionSchema),
})

export type FullAutoRunProjectionFetch = (
  input: URL | RequestInfo,
  init?: RequestInit,
) => Promise<Response>

/**
 * Best-effort real fetch against the documented (but not-yet-landed) mobile
 * projection endpoint. Naming follows the neutral `/api/fleet-runs` /
 * `/api/full-auto-runs` sibling pattern; #8981 owns the real route and
 * response envelope. Until #8981 lands this call 404s or network-errors and
 * safely resolves `{ state: "unavailable" }`, which mobile treats exactly
 * like "no active run" (falls through to existing default behavior — no
 * regression). Once #8981 lands, reconcile the path and
 * `ResponseEnvelope`/`FullAutoRunMobileProjectionSchema` shape against the
 * real contract; no caller of `FullAutoRunProjectionSource` needs to change.
 */
export const fetchFullAutoRunMobileProjection = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetchImpl?: FullAutoRunProjectionFetch
}>): Promise<FullAutoRunProjectionResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL("/api/full-auto-runs/mine", input.baseUrl),
      {
        method: "GET",
        headers: { authorization: `Bearer ${input.accessToken}` },
        cache: "no-store",
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) return { state: "unavailable" }
    const envelope = Schema.decodeUnknownSync(ResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return envelope.fullAutoRun === null
      ? { state: "none" }
      : { state: "active", projection: envelope.fullAutoRun }
  } catch {
    return { state: "unavailable" }
  }
}
