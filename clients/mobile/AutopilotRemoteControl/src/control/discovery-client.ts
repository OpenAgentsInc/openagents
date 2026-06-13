import {
  resolveBaseUrls,
  type PairingAddresses,
} from "@openagentsinc/autopilot-control-protocol"

import type { ConnectInfo } from "./control-client"

export type NodeRegistration = {
  id?: string
  name?: string
  addresses: PairingAddresses
  controlToken: string
}

export function parseNodesResponse(raw: unknown): NodeRegistration[] {
  if (!isRecord(raw) || !Array.isArray(raw.nodes)) {
    throw new Error("bad nodes response")
  }

  return raw.nodes.map(parseNodeRegistration)
}

export function pickConnect(registration: NodeRegistration): ConnectInfo {
  const baseUrl = resolveBaseUrls(registration.addresses)[0]
  if (!baseUrl) {
    throw new Error("node registration has no reachable address")
  }

  return {
    baseUrl,
    token: registration.controlToken,
  }
}

function parseNodeRegistration(raw: unknown): NodeRegistration {
  if (!isRecord(raw)) {
    throw new Error("bad node registration")
  }

  const addresses = parseAddresses(raw.addresses)
  const controlToken = raw.controlToken
  if (typeof controlToken !== "string" || controlToken.length === 0) {
    throw new Error("bad node registration")
  }

  const registration: NodeRegistration = {
    addresses,
    controlToken,
  }

  if (typeof raw.id === "string") {
    registration.id = raw.id
  }
  if (typeof raw.name === "string") {
    registration.name = raw.name
  }

  return registration
}

function parseAddresses(raw: unknown): PairingAddresses {
  if (!isRecord(raw)) {
    throw new Error("bad node registration")
  }

  const addresses: PairingAddresses = {}
  for (const key of ["loopback", "lan", "tailnet"] as const) {
    const value = raw[key]
    if (typeof value === "string" && value.length > 0) {
      addresses[key] = value
    } else if (value !== undefined) {
      throw new Error("bad node registration")
    }
  }

  if (resolveBaseUrls(addresses).length === 0) {
    throw new Error("bad node registration")
  }

  return addresses
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
