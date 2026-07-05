import { personalScope } from "@openagentsinc/khala-sync"

import { mobileProblemMessageSafe, readOkMobileJsonResponse } from "../network/mobile-problem"
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
    await readOkMobileJsonResponse(response, "sign-in check")
    return { ok: true }
  } catch (error) {
    const messageSafe = mobileProblemMessageSafe(error, "sign-in check")
    return { ok: false, messageSafe }
  }
}
