/**
 * L3 SANDBOX — the sandbox-provider seam.
 *
 * Re-exports the sandbox modules of
 * `@openagentsinc/agent-harness-contract`: the sandbox-provider contract,
 * the local (pass-through) provider, and the local-process provider. The
 * sandbox is optional per session — the SDK carries the owner-local danger
 * profile as explicit policy. The audited export union of the three modules
 * is collision-free.
 */
export * from "@openagentsinc/agent-harness-contract/sandbox";
export * from "@openagentsinc/agent-harness-contract/local-sandbox-provider";
export * from "@openagentsinc/agent-harness-contract/local-process-sandbox-provider";
