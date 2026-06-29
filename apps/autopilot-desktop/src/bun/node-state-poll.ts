import { CONTROL_SCHEMA_TAG } from "@openagentsinc/autopilot-control-protocol"
import type { NodeStateMessage } from "../shared/rpc.js"

export type FetchNodeStateLike = () => Promise<NodeStateMessage>

export type NodeStatePollTimer = {
  setInterval(callback: () => void, intervalMs: number): unknown
  clearInterval(handle: unknown): void
}

export type NodeStatePoller = {
  pollOnce(): Promise<NodeStateMessage>
  start(): void
  stop(): void
}

export async function pollNodeStateOnce(input: {
  fetchNodeState: FetchNodeStateLike
  fallbackSchema?: string
}): Promise<NodeStateMessage> {
  try {
    const state = await input.fetchNodeState()
    return {
      ok: state.ok,
      schema: state.schema,
      sessions: [...state.sessions],
      ...(state.events ? { events: state.events } : {}),
      ...(state.accounts ? { accounts: state.accounts } : {}),
      ...(state.artifacts ? { artifacts: state.artifacts } : {}),
      ...(state.deploy ? { deploy: state.deploy } : {}),
      ...(state.intents ? { intents: state.intents } : {}),
      ...(state.approvals ? { approvals: state.approvals } : {}),
      ...(state.wallet !== undefined ? { wallet: state.wallet } : {}),
      ...(state.assignments ? { assignments: state.assignments } : {}),
      ...(state.coordinatorPaused !== undefined ? { coordinatorPaused: state.coordinatorPaused } : {}),
      // #5468: pass through the bounded auto-approve audit trail when present.
      ...(state.autoApprovals ? { autoApprovals: state.autoApprovals } : {}),
    }
  } catch {
    return offlineNodeState(input.fallbackSchema)
  }
}

export function createNodeStatePoller(input: {
  fetchNodeState: FetchNodeStateLike
  intervalMs: number
  onState: (message: NodeStateMessage) => void
  timer?: NodeStatePollTimer
  fallbackSchema?: string
}): NodeStatePoller {
  const timer = input.timer ?? {
    setInterval: globalThis.setInterval.bind(globalThis),
    clearInterval: (handle: unknown) =>
      globalThis.clearInterval(handle as ReturnType<typeof setInterval>),
  }
  let intervalHandle: unknown
  let active = false

  async function pollOnce(): Promise<NodeStateMessage> {
    const message = await pollNodeStateOnce({
      fetchNodeState: input.fetchNodeState,
      ...(input.fallbackSchema !== undefined
        ? { fallbackSchema: input.fallbackSchema }
        : {}),
    })
    input.onState(message)
    return message
  }

  return {
    pollOnce,
    start() {
      if (active) return
      active = true
      void pollOnce()
      intervalHandle = timer.setInterval(() => {
        void pollOnce()
      }, input.intervalMs)
    },
    stop() {
      if (!active) return
      active = false
      if (intervalHandle !== undefined) timer.clearInterval(intervalHandle)
      intervalHandle = undefined
    },
  }
}

function offlineNodeState(schema: string = CONTROL_SCHEMA_TAG): NodeStateMessage {
  return {
    ok: false,
    schema,
    sessions: [],
  }
}
