import type { DecisionVerb } from "@openagentsinc/autopilot-control-protocol"

export type DecisionBroadcastMessage = "resolved" | "resolved_elsewhere"

export type DecisionResolutionBroadcastInput = {
  readonly requestId: string
  readonly resolvedVerb: DecisionVerb
  readonly resolvingClientRef: string
  readonly clientRefs: readonly string[]
}

export type DecisionCancellationBroadcastInput = {
  readonly requestId: string
  readonly clientRefs: readonly string[]
}

export type DecisionBroadcastTarget<Message extends string = string> = {
  readonly clientRef: string
  readonly message: Message
}

export function broadcastResolution(
  input: DecisionResolutionBroadcastInput,
): Array<DecisionBroadcastTarget<DecisionBroadcastMessage>> {
  return input.clientRefs.map((clientRef) => ({
    clientRef,
    message: clientRef === input.resolvingClientRef ? "resolved" : "resolved_elsewhere",
  }))
}

export function broadcastCancellation(
  input: DecisionCancellationBroadcastInput,
): Array<DecisionBroadcastTarget<"cancelled">> {
  return input.clientRefs.map((clientRef) => ({
    clientRef,
    message: "cancelled",
  }))
}
