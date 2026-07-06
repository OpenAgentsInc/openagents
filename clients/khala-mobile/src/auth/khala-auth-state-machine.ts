import type { KhalaStoredCredentials } from "./khala-auth-store"

export type KhalaAuthMachineStatus =
  | "loading"
  | "signed_out"
  | "signing_in"
  | "signed_in"

export type KhalaAuthMachineState = Readonly<{
  credentials: KhalaStoredCredentials | null
  messageSafe: string | null
  status: KhalaAuthMachineStatus
}>

export type KhalaAuthMachineEvent =
  | Readonly<{
      devCredentials: KhalaStoredCredentials | null
      storedCredentials: KhalaStoredCredentials | null
      type: "stored_credentials_loaded"
    }>
  | Readonly<{ type: "github_sign_in_started" }>
  | Readonly<{
      credentials: KhalaStoredCredentials
      type: "github_sign_in_succeeded"
    }>
  | Readonly<{ messageSafe: string; type: "github_sign_in_failed" }>
  | Readonly<{ type: "github_sign_in_cancelled" }>
  | Readonly<{ type: "signed_out" }>

export const initialKhalaAuthMachineState: KhalaAuthMachineState = {
  credentials: null,
  messageSafe: null,
  status: "loading",
}

export const reduceKhalaAuthMachine = (
  state: KhalaAuthMachineState,
  event: KhalaAuthMachineEvent,
): KhalaAuthMachineState => {
  switch (event.type) {
    case "stored_credentials_loaded": {
      const credentials = event.storedCredentials ?? event.devCredentials

      return credentials === null
        ? { credentials: null, messageSafe: null, status: "signed_out" }
        : { credentials, messageSafe: null, status: "signed_in" }
    }
    case "github_sign_in_started":
      return { credentials: null, messageSafe: null, status: "signing_in" }
    case "github_sign_in_succeeded":
      return {
        credentials: event.credentials,
        messageSafe: null,
        status: "signed_in",
      }
    case "github_sign_in_failed":
      return {
        credentials: null,
        messageSafe: event.messageSafe,
        status: "signed_out",
      }
    case "github_sign_in_cancelled":
      return { credentials: null, messageSafe: null, status: "signed_out" }
    case "signed_out":
      return { credentials: null, messageSafe: null, status: "signed_out" }
    default:
      return state
  }
}

export type KhalaSignedOutPrimaryAction = "github"

export const signedOutPrimaryActions = (
  state: KhalaAuthMachineState,
): ReadonlyArray<KhalaSignedOutPrimaryAction> =>
  state.status === "signed_out" || state.status === "signing_in"
    ? ["github"]
    : []
