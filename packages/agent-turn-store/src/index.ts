/**
 * `@openagentsinc/agent-turn-store` — the driver-neutral turn journal.
 *
 * AFS-01 adds the driver-neutral persisted state, its single additive migration
 * path, and an in-memory adapter of the kernel `TurnJournal` port. Platform
 * drivers (Node, Expo, browser) live only in platform subpaths or app
 * composition roots, never in this root export. This package owns the
 * driver-neutral state and migrations. The turn state machine must not import a
 * concrete store; the store depends on the kernel, not the reverse.
 */
export const AGENT_TURN_STORE_PACKAGE = "@openagentsinc/agent-turn-store" as const;
export const AGENT_TURN_STORE_RESERVED = true as const;

export * from "./turn-journal-memory.js";
