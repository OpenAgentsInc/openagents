import type { SafeTurnProjection, TurnLifecycleState } from "@openagentsinc/agent-runtime-schema";
import type { TurnJournalPort } from "@openagentsinc/agent-turn-runtime";

/**
 * `@openagentsinc/agent-turn-store` — the driver-neutral turn journal (AFS-00
 * reservation).
 *
 * Packet AFS-01 adds the store port, the driver-neutral state and migrations,
 * and an in-memory test adapter. Platform drivers (Node, Expo, browser, memory)
 * live only in platform subpaths or app composition roots, never in this root
 * export.
 *
 * This package owns the driver-neutral state and migrations. It must not own
 * platform drivers in its root export. The turn state machine must not import a
 * concrete store.
 */
export const AGENT_TURN_STORE_PACKAGE = "@openagentsinc/agent-turn-store" as const;
export const AGENT_TURN_STORE_RESERVED = true as const;

/** The driver-neutral store port AFS-01 implements over the turn journal. */
export interface TurnStorePort {
  readonly journal: TurnJournalPort;
  readonly lastState: TurnLifecycleState;
  readonly projection: SafeTurnProjection | null;
}
