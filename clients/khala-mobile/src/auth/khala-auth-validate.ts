import { personalScope } from "@openagentsinc/khala-sync"

import { buildBootstrapRequestBody, buildBootstrapUrl } from "../sync/khala-sync-entities-core"

export type KhalaAuthValidation = Readonly<{ ok: true } | { ok: false; messageSafe: string }>

/** Confirms a token/ownerUserId pair actually authenticates against Khala
 * Sync before saving it — a real bootstrap call against the owner's own
 * personal scope, not just a shape check. */
export const validateKhalaCredentials = async (input: {
  baseUrl: string
  ownerUserId: string
  token: string
}): Promise<KhalaAuthValidation> => {
  try {
    const response = await fetch(buildBootstrapUrl(input.baseUrl), {
      body: JSON.stringify(
        buildBootstrapRequestBody(String(personalScope(input.ownerUserId)), "khala-mobile-sign-in-check")
      ),
      headers: {
        authorization: `Bearer ${input.token}`,
        "content-type": "application/json"
      },
      method: "POST"
    })
    if (response.ok) return { ok: true }
    const body: unknown = await response.json().catch(() => null)
    const messageSafe =
      typeof body === "object" && body !== null && "messageSafe" in body
        ? String((body as { messageSafe: unknown }).messageSafe)
        : `sign-in check failed (${response.status})`
    return { ok: false, messageSafe }
  } catch (error) {
    return { ok: false, messageSafe: error instanceof Error ? error.message : "sign-in check failed" }
  }
}
