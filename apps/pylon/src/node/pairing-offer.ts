import {
  type BootstrapPayload,
  type Capability,
  encodeBootstrapPayload,
  type ProjectionLevel,
} from "@openagentsinc/autopilot-control-protocol"

import type { BindAddress } from "./bind-config.js"

export type BuildPairingOfferInput = {
  binds: Array<Pick<BindAddress, "address" | "requiresAuth">>
  bootstrapId: string
  secret: string
  projectionLevel: ProjectionLevel
  capabilities: Capability[]
}

export type PairingOffer = {
  payload: BootstrapPayload
  qr: string
}

export function buildPairingOffer(input: BuildPairingOfferInput): PairingOffer {
  const payload: BootstrapPayload = {
    version: 1,
    addresses: classifyAddresses(input.binds),
    bootstrapId: input.bootstrapId,
    secret: input.secret,
    projectionLevel: input.projectionLevel,
    capabilities: input.capabilities,
  }

  return {
    payload,
    qr: encodeBootstrapPayload(payload),
  }
}

function classifyAddresses(
  binds: ReadonlyArray<Pick<BindAddress, "address" | "requiresAuth">>,
): BootstrapPayload["addresses"] {
  const addresses: { -readonly [K in keyof BootstrapPayload["addresses"]]?: BootstrapPayload["addresses"][K] } = {}

  for (const bind of binds) {
    if (bind.address.startsWith("127.")) {
      addresses.loopback = bind.address
    } else if (bind.address.startsWith("100.")) {
      addresses.tailnet = bind.address
    } else {
      addresses.lan = bind.address
    }
  }

  return addresses as BootstrapPayload["addresses"]
}
