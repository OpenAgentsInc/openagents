import { Context, Effect } from "effect"

import type { LndWalletState } from "../contracts/lnd.js"
import type {
  LndWalletInitializeRequest,
  LndWalletRestoreRequest,
  LndWalletUnlockRequest,
} from "../contracts/rpc.js"
import type {
  LndServiceUnavailableError,
  LndWalletOperationError,
} from "../errors/lndErrors.js"

export type LndWalletServiceError = LndServiceUnavailableError | LndWalletOperationError

export type LndWalletApi = Readonly<{
  readonly getWalletState: () => Effect.Effect<LndWalletState, LndWalletServiceError>
  readonly initializeWallet: (
    input: LndWalletInitializeRequest,
  ) => Effect.Effect<LndWalletState, LndWalletServiceError>
  readonly unlockWallet: (
    input: LndWalletUnlockRequest,
  ) => Effect.Effect<LndWalletState, LndWalletServiceError>
  readonly restoreWallet: (
    input: LndWalletRestoreRequest,
  ) => Effect.Effect<LndWalletState, LndWalletServiceError>
  readonly lockWallet: () => Effect.Effect<LndWalletState, LndWalletServiceError>
}>

export class LndWalletService extends Context.Tag("@openagents/lnd-effect/LndWalletService")<
  LndWalletService,
  LndWalletApi
>() {}
