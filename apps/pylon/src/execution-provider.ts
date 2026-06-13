export type PylonExecutionProviderKind =
  | "local_process"
  | "static_ssh"
  | "openagents_cloud"

export type PylonExecutionProviderSpec = {
  kind: PylonExecutionProviderKind
  displayRef: string
  features: {
    sync: boolean
    remoteRun: boolean
    artifacts: boolean
  }
}

export const PYLON_EXECUTION_PROVIDER_KINDS: PylonExecutionProviderKind[] = [
  "local_process",
  "static_ssh",
  "openagents_cloud",
]

export function describeExecutionProvider(
  kind: PylonExecutionProviderKind,
): PylonExecutionProviderSpec {
  switch (kind) {
    case "local_process":
      return {
        kind,
        displayRef: "Local process",
        features: {
          sync: false,
          remoteRun: false,
          artifacts: true,
        },
      }
    case "static_ssh":
      return {
        kind,
        displayRef: "Static SSH",
        features: {
          sync: true,
          remoteRun: true,
          artifacts: true,
        },
      }
    case "openagents_cloud":
      return {
        kind,
        displayRef: "OpenAgents Cloud",
        features: {
          sync: true,
          remoteRun: true,
          artifacts: true,
        },
      }
  }
}
