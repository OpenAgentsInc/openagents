export interface NodeRegistration {
  nodeRef: string
  // Nodes register with an ISO-8601 string (Date#toISOString); older callers
  // may send epoch millis. Both are accepted and normalized when comparing.
  updatedAt: string | number
  controlToken?: string
  [key: string]: unknown
}

// Normalize either timestamp form to epoch millis (NaN if unparseable).
function toEpochMs(updatedAt: string | number): number {
  return typeof updatedAt === "number" ? updatedAt : Date.parse(updatedAt)
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
        toEpochMs(registration.updatedAt) >= toEpochMs(current.updatedAt)
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
          const ts = toEpochMs(registration.updatedAt)
          // Drop entries whose timestamp is unparseable (NaN) or older than the
          // allowed age — so the phone only ever sees nodes that recently
          // heartbeated, never stale/dead registrations.
          if (Number.isNaN(ts) || nowMs - ts > maxAgeMs) {
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
