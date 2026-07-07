import { describe, expect, test } from "bun:test"

/**
 * Oracle for khala_mobile.auth.signout_ends_web_session_for_account_switch.v1
 *
 * Source-level assertion (the auth context reaches into
 * expo-auth-session/expo-web-browser native modules that have no meaning under
 * `bun test`, so a mounted-component oracle is not available here; this pins
 * the exact shipped flow instead). It proves:
 *
 *  1. The GitHub OAuth prompt runs in an EPHEMERAL web session
 *     (`prefersEphemeralWebBrowserSession: true`) — a non-persistent
 *     ASWebAuthenticationSession on iOS that does NOT reuse Safari's cookies,
 *     so the OpenAuth issuer session (auth.openagents.com) and GitHub's own
 *     session are never silently reused. Every login presents a fresh GitHub
 *     login / account picker.
 *  2. Sign-out still revokes the server session (`deleteMobileOpenAuthSession`).
 *
 * Together these are the fix for the owner report (2026-07-07) that after Sign
 * out, "Log in with GitHub" silently re-authenticated the same account and
 * could not switch to a different (test) account.
 */
const repoPath = (ref: string): string => new URL(`../../../${ref}`, import.meta.url).pathname

describe("contract khala_mobile.auth.signout_ends_web_session_for_account_switch.v1", () => {
  test("signout_ephemeral_web_session.source — OAuth prompt is ephemeral and sign-out revokes the server session", async () => {
    const source = await Bun.file(
      repoPath("clients/khala-mobile/src/auth/khala-auth-context.tsx"),
    ).text()

    // (1) Ephemeral web session on the prompt — the reliable "let the user
    // choose the account" fix. Must be passed to promptAsync. (expo-auth-session
    // spells the ASWebAuthenticationSession ephemeral flag `preferEphemeralSession`.)
    expect(source).toContain("preferEphemeralSession: true")
    expect(source).toMatch(/promptAsync\([^)]*discovery[\s\S]*preferEphemeralSession/)

    // (2) Sign-out still revokes the server session (kept from before).
    expect(source).toContain("deleteMobileOpenAuthSession")

    // Guard: the earlier state_mismatch fix (one imperative AuthRequest that
    // both opens and validates) must remain — do not regress it.
    expect(source).toContain("new AuthRequest(")
    expect(source).toContain("codeVerifier")
  })
})
