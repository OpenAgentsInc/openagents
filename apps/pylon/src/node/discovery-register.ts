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
  readonly controlToken: string
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

  toJSON(): Omit<NodeRegistration, "controlToken"> & { controlToken: "[redacted]" } {
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
