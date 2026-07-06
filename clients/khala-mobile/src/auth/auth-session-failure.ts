/** Pure helper (no React / no native imports) so it is unit-testable.
 *
 * Turns a non-success `expo-auth-session` `promptAsync` outcome into a real,
 * user-readable reason. Includes the outcome `type` and any OAuth error the
 * issuer redirected back — so a stuck GitHub sign-in reports WHY instead of
 * looking like nothing happened (#8467). */
export type AuthSessionFailureResult = {
  type: string
  error?: { code?: string; description?: string | null } | null
  params?: Record<string, string>
}

export const describeAuthSessionFailure = (
  result: AuthSessionFailureResult,
): string => {
  const params = result.params ?? {}
  const oauthError = params.error_description ?? params.error
  const detail = result.error?.description ?? oauthError

  if (result.type === "error" && detail !== undefined && detail !== "") {
    return `GitHub sign-in failed: ${detail}`
  }
  if (result.type === "locked") {
    return "A GitHub sign-in is already in progress. Close it and try again."
  }
  if (detail !== undefined && detail !== "") {
    return `GitHub sign-in didn't complete (${result.type}): ${detail}`
  }
  return `GitHub sign-in didn't complete (${result.type}). Please try again.`
}
