import {
  probeClaudeAgentReadiness,
  type ClaudeAgentReadiness,
  type ClaudeAgentProbeOptions,
} from "../../../../apps/pylon/src/claude-agent.js"

export type KhalaCodeDesktopClaudeHarnessStatus = {
  readonly available: boolean
  readonly blockerRefs: readonly string[]
  readonly capability: "claude_harness"
  readonly credentialSourceRef: string | null
  readonly observedAt: string
  readonly reason: string
  readonly status: ClaudeAgentReadiness["state"]
}

export type InspectClaudeHarnessStatusOptions = ClaudeAgentProbeOptions & {
  readonly now?: () => Date
}

const reasonFor = (readiness: ClaudeAgentReadiness): string => {
  switch (readiness.state) {
    case "ready":
      return "ready"
    case "sdk_missing":
      return "Claude Agent SDK is not installed."
    case "credentials_missing":
      return "Claude credentials are missing."
    case "platform_unsupported":
      return "Claude Agent SDK is not supported on this platform."
    case "disabled_by_config":
      return "Claude harness is disabled by config."
  }
}

export async function inspectClaudeHarnessStatus(
  options: InspectClaudeHarnessStatusOptions = {},
): Promise<KhalaCodeDesktopClaudeHarnessStatus> {
  const readiness = await probeClaudeAgentReadiness(options)
  const available = readiness.state === "ready"
  return {
    available,
    blockerRefs: readiness.blockerRefs,
    capability: "claude_harness",
    credentialSourceRef: readiness.credentialSourceRef,
    observedAt: (options.now ?? (() => new Date()))().toISOString(),
    reason: reasonFor(readiness),
    status: readiness.state,
  }
}
