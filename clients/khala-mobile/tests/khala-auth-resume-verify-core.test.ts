import { describe, expect, mock, test } from "bun:test"

import { resolveVerifiedStoredCredentials } from "../src/auth/khala-auth-resume-verify-core"

const credentials = { ownerUserId: "owner-1", token: "token-1" }

// Oracle for khala_mobile.auth.stored_credential_revalidated_on_launch.v1
describe("resolveVerifiedStoredCredentials", () => {
  test("no stored credential: returns null, never calls validate or clear", async () => {
    const validate = mock(async () => ({ ok: true as const }))
    const clearStoredCredentials = mock(async () => undefined)

    const result = await resolveVerifiedStoredCredentials(null, {
      clearStoredCredentials,
      validate,
    })

    expect(result).toBeNull()
    expect(validate).not.toHaveBeenCalled()
    expect(clearStoredCredentials).not.toHaveBeenCalled()
  })

  test("a credential that validates: returned unchanged, clear never called", async () => {
    const validate = mock(async () => ({ ok: true as const }))
    const clearStoredCredentials = mock(async () => undefined)

    const result = await resolveVerifiedStoredCredentials(credentials, {
      clearStoredCredentials,
      validate,
    })

    expect(result).toEqual(credentials)
    expect(validate).toHaveBeenCalledTimes(1)
    expect(validate).toHaveBeenCalledWith(credentials)
    expect(clearStoredCredentials).not.toHaveBeenCalled()
  })

  test("a credential that fails validation (e.g. a leftover pre-pivot or revoked token): cleared and returns null", async () => {
    const validate = mock(async () => ({
      messageSafe: "session expired",
      ok: false as const,
    }))
    const clearStoredCredentials = mock(async () => undefined)

    const result = await resolveVerifiedStoredCredentials(credentials, {
      clearStoredCredentials,
      validate,
    })

    expect(result).toBeNull()
    expect(validate).toHaveBeenCalledTimes(1)
    expect(clearStoredCredentials).toHaveBeenCalledTimes(1)
  })
})
