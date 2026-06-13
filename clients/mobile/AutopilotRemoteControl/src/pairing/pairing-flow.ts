import {
  createCredentialStore,
  decodeBootstrapPayload,
  isCredentialUsable,
  resolveBaseUrls,
  type CredentialStore,
  type PairingCredentialClaims,
} from "@openagentsinc/autopilot-control-protocol"

import {
  buildPairingExchangeRequest,
  pairingStatusView,
  type PairingExchangeRequestDescriptor,
  type PairingStatusState,
  type PairingStatusViewModel,
} from "./pairing-view-model"

export type PairingFlowState = {
  status: PairingStatusState
  statusView: PairingStatusViewModel
  exchangeRequest?: PairingExchangeRequestDescriptor
  baseUrls?: string[]
  credential?: PairingCredentialClaims
}

export type PairingFlowOptions = {
  clientId: string
  credentialStore?: CredentialStore
}

export type PairingFlow = {
  startPairing(qrOrCode: string): PairingFlowState
  completePairing(claims: PairingCredentialClaims): PairingFlowState
  failPairing(reason: string): PairingFlowState
  getCredential(): PairingCredentialClaims | undefined
  isCredentialUsable(nowMs: number): boolean
}

export function createPairingFlow(options: PairingFlowOptions): PairingFlow {
  const store = options.credentialStore ?? createCredentialStore()

  return {
    startPairing(qrOrCode) {
      try {
        const payload = decodeBootstrapPayload(qrOrCode.trim())
        const baseUrls = resolveBaseUrls(payload.addresses)
        const baseUrl = baseUrls[0]
        if (!baseUrl) {
          return stateFromStatus({ phase: "error", error: "No reachable bootstrap address" })
        }

        return stateFromStatus(
          { phase: "pairing" },
          {
            baseUrls,
            exchangeRequest: buildPairingExchangeRequest({
              baseUrl,
              bootstrapId: payload.bootstrapId,
              clientId: options.clientId,
              secret: payload.secret,
            }),
          },
        )
      } catch (error) {
        return stateFromStatus({ phase: "error", error: errorMessage(error) })
      }
    },

    completePairing(claims) {
      store.set(claims)
      return stateFromStatus(
        { phase: "paired", pairingRef: claims.pairingRef },
        { credential: claims },
      )
    },

    failPairing(reason) {
      return stateFromStatus({ phase: "error", error: reason })
    },

    getCredential() {
      return store.get()
    },

    isCredentialUsable(nowMs) {
      const credential = store.get()
      return credential ? isCredentialUsable(credential, nowMs) : false
    },
  }
}

function stateFromStatus(
  status: PairingStatusState,
  rest: Omit<PairingFlowState, "status" | "statusView"> = {},
): PairingFlowState {
  return {
    ...rest,
    status,
    statusView: pairingStatusView(status),
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Pairing failed"
}
