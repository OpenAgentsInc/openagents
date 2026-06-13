export type BindInterfaces = {
  loopback: string
  lan?: string
  tailnet?: string
}

export type ResolveBindAddressesInput = {
  interfaces: BindInterfaces
  enableLan: boolean
  enableTailnet: boolean
}

export type BindAddress = {
  address: string
  requiresAuth: boolean
}

export type ResolveBindAddressesResult = {
  binds: BindAddress[]
}

export function isAuthRequiredForRemote(address: string, loopbackAddr: string): boolean {
  return address !== loopbackAddr
}

export function resolveBindAddresses(
  input: ResolveBindAddressesInput,
): ResolveBindAddressesResult {
  const { interfaces } = input
  const binds: BindAddress[] = [
    {
      address: interfaces.loopback,
      requiresAuth: false,
    },
  ]

  if (input.enableLan && interfaces.lan) {
    binds.push({
      address: interfaces.lan,
      requiresAuth: true,
    })
  }

  if (input.enableTailnet && interfaces.tailnet) {
    binds.push({
      address: interfaces.tailnet,
      requiresAuth: true,
    })
  }

  return { binds }
}
