/**
 * L4 HARNESS — the versioned coding-agent harness contract.
 *
 * Re-exports the full `@openagentsinc/agent-harness-contract` entry point:
 * the `AgentHarness` adapter shape, the session verbs (promptTurn,
 * suspendTurn, continueTurn, compact, detach, stop, destroy),
 * capability-by-method-presence, the slice runner, the readiness
 * projection, skills, host tools, the toolkit bridge (STREAM-07), and the
 * ACP and opencode adapters. This entry also carries the package's L2, L3,
 * and L5 modules — use `./event-log`, `./sandbox`, and `./ui-stream` for
 * the layer-scoped subsets.
 */
export * from "@openagentsinc/agent-harness-contract";
