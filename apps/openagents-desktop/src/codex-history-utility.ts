import { fileURLToPath } from "node:url"

import type { CodexHistoryProcess } from "./codex-history-host.ts"

type UtilityProcessLike = Readonly<{
  postMessage: (value: unknown) => void
  on: (event: "message" | "exit" | "error", listener: (...args: never[]) => void) => unknown
  kill: () => boolean
}>

type ForkUtilityProcess = (
  modulePath: string,
  args: string[],
  options: Readonly<{
    env: Record<string, string>
    serviceName: string
    stdio: "ignore"
    allowLoadingUnsignedLibraries: false
  }>,
) => unknown

/**
 * Keep rollout parsing off the browser/main V8 isolate without inheriting
 * credentials or granting unsigned native-library loading to the child.
 */
export const makeCodexHistoryUtilityFactory = (
  workerUrl: URL,
  fork: ForkUtilityProcess,
): (() => CodexHistoryProcess) => () => {
  const child = fork(fileURLToPath(workerUrl), [], {
    env: {},
    serviceName: "OpenAgents History",
    stdio: "ignore",
    allowLoadingUnsignedLibraries: false,
  }) as UtilityProcessLike
  return {
    postMessage: value => child.postMessage(value),
    onMessage: listener => { child.on("message", listener) },
    onExit: listener => {
      child.on("exit", listener)
      child.on("error", listener)
    },
    terminate: () => { child.kill() },
  }
}
