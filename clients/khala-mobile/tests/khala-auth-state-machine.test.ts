import { describe, expect, test } from "bun:test"

import {
  initialKhalaAuthMachineState,
  reduceKhalaAuthMachine,
  signedOutPrimaryActions,
} from "../src/auth/khala-auth-state-machine"

describe("Khala auth state machine", () => {
  test("fresh installs become signed out without entering Tailnet discovery", () => {
    const state = reduceKhalaAuthMachine(initialKhalaAuthMachineState, {
      devCredentials: null,
      storedCredentials: null,
      type: "stored_credentials_loaded",
    })

    expect(state).toEqual({
      credentials: null,
      messageSafe: null,
      status: "signed_out",
    })
    expect(signedOutPrimaryActions(state)).toEqual(["github"])
  })

  test("stored credentials still restore the session", () => {
    const state = reduceKhalaAuthMachine(initialKhalaAuthMachineState, {
      devCredentials: null,
      storedCredentials: { ownerUserId: "github:12345", token: "token-1" },
      type: "stored_credentials_loaded",
    })

    expect(state).toEqual({
      credentials: { ownerUserId: "github:12345", token: "token-1" },
      messageSafe: null,
      status: "signed_in",
    })
    expect(signedOutPrimaryActions(state)).toEqual([])
  })

  test("GitHub sign-in moves through pending, failure, and success states", () => {
    const pending = reduceKhalaAuthMachine(
      { credentials: null, messageSafe: null, status: "signed_out" },
      { type: "github_sign_in_started" },
    )
    expect(pending.status).toBe("signing_in")
    expect(signedOutPrimaryActions(pending)).toEqual(["github"])

    const failed = reduceKhalaAuthMachine(pending, {
      messageSafe: "GitHub sign-in: sign-in required",
      type: "github_sign_in_failed",
    })
    expect(failed).toEqual({
      credentials: null,
      messageSafe: "GitHub sign-in: sign-in required",
      status: "signed_out",
    })

    const signedIn = reduceKhalaAuthMachine(failed, {
      credentials: { ownerUserId: "github:12345", token: "mobile-sync-token" },
      type: "github_sign_in_succeeded",
    })
    expect(signedIn).toEqual({
      credentials: { ownerUserId: "github:12345", token: "mobile-sync-token" },
      messageSafe: null,
      status: "signed_in",
    })
  })
})
