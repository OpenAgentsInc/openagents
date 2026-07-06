import { personalScope } from "@openagentsinc/khala-sync"

import { mobileProblemMessageSafe, readOkMobileJsonResponse } from "../network/mobile-problem"
import { buildBootstrapRequestBody, buildBootstrapUrl } from "../sync/khala-sync-entities-core"

export type KhalaAuthValidation = Readonly<{ ok: true } | { ok: false; messageSafe: string }>

/** Hard cap on the launch credential re-validation network call. Without
 * this, a slow/hanging bootstrap request leaves the app's auth status stuck
 * at "loading" FOREVER on a bare spinner (this call runs on every launch
 * before the app decides signed-in vs signed-out — a real 2026-07-06 bug
 * where a hung fetch had no escape). On timeout the credential is treated
 * as unvalidated (`ok: false`), which clears it and lands the user on the
 * sign-in screen rather than an infinite spinner. 12s is generous for a
 * single bootstrap POST on mobile networks. */
const VALIDATE_TIMEOUT_MS = 12_000

/** Confirms a token/ownerUserId pair actually authenticates against Khala
 * Sync before saving it — a real bootstrap call against the owner's own
 * personal scope, not just a shape check. Bounded by `VALIDATE_TIMEOUT_MS`
 * so it can never hang the app's auth gate. */
export const validateKhalaCredentials = async (input: {
  baseUrl: string
  ownerUserId: string
  token: string
}): Promise<KhalaAuthValidation> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), VALIDATE_TIMEOUT_MS)
  try {
    const response = await fetch(buildBootstrapUrl(input.baseUrl), {
      body: JSON.stringify(
        buildBootstrapRequestBody(String(personalScope(input.ownerUserId)), "khala-mobile-sign-in-check")
      ),
      headers: {
        authorization: `Bearer ${input.token}`,
        "content-type": "application/json"
      },
      method: "POST",
      signal: controller.signal
    })
    await readOkMobileJsonResponse(response, "sign-in check")
    return { ok: true }
  } catch (error) {
    const messageSafe = mobileProblemMessageSafe(error, "sign-in check")
    return { ok: false, messageSafe }
  } finally {
    clearTimeout(timer)
  }
}
