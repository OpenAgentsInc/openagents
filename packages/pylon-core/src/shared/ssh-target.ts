export type SshTarget = {
  user: string
  host: string
  port: number
  fallbackPorts: number[]
  knownHostsFile: string | null
  proxyCommand: string | null
}

export type SshReadinessState =
  | "tcp_unreachable"
  | "auth_failed"
  | "ready"

export function normalizeSshTarget(input: {
  user?: string
  host: string
  port?: number
  fallbackPorts?: number[]
  knownHostsFile?: string | null
  proxyCommand?: string | null
}): SshTarget {
  if (!input.host || /\s/.test(input.host)) {
    throw new Error("SSH target host must be a non-empty string with no whitespace")
  }

  return {
    user: input.user ?? "root",
    host: input.host,
    port: input.port ?? 22,
    fallbackPorts: input.fallbackPorts ?? [],
    knownHostsFile: input.knownHostsFile ?? null,
    proxyCommand: input.proxyCommand ?? null,
  }
}

export function classifySshReadiness(probe: {
  tcpOpen: boolean
  authOk: boolean
}): SshReadinessState {
  if (!probe.tcpOpen) {
    return "tcp_unreachable"
  }

  if (!probe.authOk) {
    return "auth_failed"
  }

  return "ready"
}
