import { fetchFullAutoRunClientProjection } from "@openagentsinc/khala-sync-client"

import type { FullAutoRunProjectionResult } from "./full-auto-run-projection"

/**
 * The one clearly-named module boundary #8982 asks for: everything above
 * this file (thread-selection priority, the state header) depends only on
 * `FullAutoRunProjectionSource`, never on how the projection is obtained.
 *
 * Wired to the REAL, landed #8981 client (`fetchFullAutoRunClientProjection`
 * in `@openagentsinc/khala-sync-client`, FA-RUN-05) — #8981 landed on `main`
 * (`0dae0911bf758f294459acc0d1ef6379130b135a`) while this issue was in
 * progress, so this consumes the real Desktop-published projection rather
 * than a stub. This file only translates that package's envelope
 * (`available`/`unauthorized`/`unavailable` wrapping a nullable `run`) into
 * this app's `FullAutoRunProjectionResult` (`active`/`none`/`unauthorized`/
 * `unavailable`), so the rest of the mobile app never depends on the
 * upstream package's exact response shape.
 */
export type FullAutoRunProjectionSource = () => Promise<FullAutoRunProjectionResult>

export type FullAutoRunProjectionFetchImpl = Parameters<
  typeof fetchFullAutoRunClientProjection
>[0]["fetchImpl"]

export const fetchFullAutoRunMobileProjection = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetchImpl?: FullAutoRunProjectionFetchImpl
}>): Promise<FullAutoRunProjectionResult> => {
  const result = await fetchFullAutoRunClientProjection(input)
  switch (result.state) {
    case "unauthorized": return { state: "unauthorized" }
    case "unavailable": return { state: "unavailable" }
    case "available":
      return result.projection.run === null
        ? { state: "none" }
        : { state: "active", projection: result.projection.run }
  }
}
