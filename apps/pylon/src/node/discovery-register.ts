const inspectCustom = Symbol.for("nodejs.util.inspect.custom")

export type NodeRegistrationAddresses = {
  loopback?: string
  lan?: string
  tailnet?: string
}

export type BuildNodeRegistrationInput = {
  nodeRef: string
  binds: Array<{ address: string }>
  controlToken: string
  updatedAt: string
}

export class NodeRegistration {
  readonly nodeRef: string
  readonly addresses: NodeRegistrationAddresses
  // Assigned via Object.defineProperty in the constructor (non-enumerable for
  // redaction), so the compiler cannot see the assignment directly.
  readonly controlToken!: string
  readonly updatedAt: string

  constructor(input: {
    nodeRef: string
    addresses: NodeRegistrationAddresses
    controlToken: string
    updatedAt: string
  }) {
    this.nodeRef = input.nodeRef
    this.addresses = input.addresses
    Object.defineProperty(this, "controlToken", {
      value: input.controlToken,
      enumerable: false,
      writable: false,
      configurable: false,
    })
    this.updatedAt = input.updatedAt
  }

  toJSON(): {
    nodeRef: string
    addresses: NodeRegistrationAddresses
    controlToken: "[redacted]"
    updatedAt: string
  } {
    return {
      nodeRef: this.nodeRef,
      addresses: this.addresses,
      controlToken: "[redacted]",
      updatedAt: this.updatedAt,
    }
  }

  toString(): string {
    return `NodeRegistration ${JSON.stringify(this.toJSON())}`
  }

  [inspectCustom](): ReturnType<NodeRegistration["toJSON"]> {
    return this.toJSON()
  }
}

export function buildNodeRegistration(
  input: BuildNodeRegistrationInput,
): NodeRegistration {
  return new NodeRegistration({
    nodeRef: input.nodeRef,
    addresses: classifyAddresses(input.binds),
    controlToken: input.controlToken,
    updatedAt: input.updatedAt,
  })
}

export function registrationKey(ownerRef: string, nodeRef: string): string {
  return `${ownerRef}:${nodeRef}`
}

function classifyAddresses(
  binds: ReadonlyArray<{ address: string }>,
): NodeRegistrationAddresses {
  const addresses: NodeRegistrationAddresses = {}

  for (const bind of binds) {
    if (bind.address.startsWith("127.")) {
      addresses.loopback = bind.address
    } else if (bind.address.startsWith("100.")) {
      addresses.tailnet = bind.address
    } else {
      addresses.lan = bind.address
    }
  }

  return addresses
}

// The discovery broker stores whatever the node POSTs; the mobile app feeds the
// stored `addresses` straight into resolveBaseUrls(), which returns each value
// VERBATIM as the fetch base URL. So the wire addresses must be full
// `http://host:port` URLs (not bare IPs) for the phone to actually connect.
function toBaseUrl(host: string, port: number): string {
  return `http://${host}:${port}`
}

export type BrokerRegistrationHosts = {
  loopback?: string
  lan?: string
  tailnet?: string
}

export type BrokerRegistrationBody = {
  nodeRef: string
  name?: string
  addresses: NodeRegistrationAddresses
  controlToken: string
  updatedAt: string
}

// Builds the exact JSON the node POSTs to the broker. Unlike NodeRegistration's
// redacting toJSON(), this carries the REAL control token — it is the wire
// credential the phone needs. Never log this object directly.
export function buildBrokerRegistrationBody(input: {
  nodeRef: string
  name?: string
  hosts: BrokerRegistrationHosts
  port: number
  controlToken: string
  updatedAt: string
  // An explicit, externally-reachable base URL (e.g. a `tailscale serve` HTTPS
  // MagicDNS endpoint: https://host.tailnet.ts.net). When set it is registered
  // verbatim as the tailnet address — used as-is by the phone — bypassing the
  // host+port builder. HTTPS here also satisfies iOS ATS (no cleartext).
  publicUrl?: string
}): BrokerRegistrationBody {
  const addresses: NodeRegistrationAddresses = {}
  if (input.hosts.loopback) addresses.loopback = toBaseUrl(input.hosts.loopback, input.port)
  if (input.hosts.lan) addresses.lan = toBaseUrl(input.hosts.lan, input.port)
  if (input.hosts.tailnet) addresses.tailnet = toBaseUrl(input.hosts.tailnet, input.port)
  if (input.publicUrl) addresses.tailnet = input.publicUrl.replace(/\/+$/, "")

  const body: BrokerRegistrationBody = {
    nodeRef: input.nodeRef,
    addresses,
    controlToken: input.controlToken,
    updatedAt: input.updatedAt,
  }
  if (input.name) body.name = input.name
  return body
}

// POST the registration to `${brokerUrl}/${ownerRef}/nodes`. Returns true on a
// 2xx. Best-effort: discovery is a convenience, never load-bearing for the node.
export async function postNodeRegistration(args: {
  brokerUrl: string
  ownerRef: string
  body: BrokerRegistrationBody
  fetchImpl?: typeof fetch
}): Promise<boolean> {
  const doFetch = args.fetchImpl ?? fetch
  const base = args.brokerUrl.replace(/\/+$/, "")
  try {
    const res = await doFetch(`${base}/${encodeURIComponent(args.ownerRef)}/nodes`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(args.body),
    })
    return res.ok
  } catch {
    return false
  }
}
