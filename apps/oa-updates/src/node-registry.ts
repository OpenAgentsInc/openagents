export interface NodeRegistration {
  nodeRef: string
  updatedAt: number
  controlToken?: string
  [key: string]: unknown
}

export interface NodeRegistry {
  register: (ownerRef: string, registration: NodeRegistration) => void
  listForOwner: (ownerRef: string) => NodeRegistration[]
  pruneStale: (nowMs: number, maxAgeMs: number) => void
}

const cloneRegistration = (
  registration: NodeRegistration,
): NodeRegistration => ({ ...registration })

export function createNodeRegistry(): NodeRegistry {
  const ownerNodes = new Map<string, Map<string, NodeRegistration>>()

  return {
    register(ownerRef, registration) {
      const nodes = ownerNodes.get(ownerRef) ?? new Map<string, NodeRegistration>()
      const current = nodes.get(registration.nodeRef)

      if (
        current === undefined ||
        registration.updatedAt >= current.updatedAt
      ) {
        nodes.set(registration.nodeRef, cloneRegistration(registration))
      }

      ownerNodes.set(ownerRef, nodes)
    },

    listForOwner(ownerRef) {
      const nodes = ownerNodes.get(ownerRef)

      return nodes === undefined
        ? []
        : [...nodes.values()].map(cloneRegistration)
    },

    pruneStale(nowMs, maxAgeMs) {
      for (const [ownerRef, nodes] of ownerNodes.entries()) {
        for (const [nodeRef, registration] of nodes.entries()) {
          if (nowMs - registration.updatedAt > maxAgeMs) {
            nodes.delete(nodeRef)
          }
        }

        if (nodes.size === 0) {
          ownerNodes.delete(ownerRef)
        }
      }
    },
  }
}
