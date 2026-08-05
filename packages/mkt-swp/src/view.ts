/**
 * The presentation entry point (SWAP-0, openagents#9315).
 *
 * A rendering surface needs the typed state, the composition order, and the
 * primary-action law — and nothing that talks to a relay or an engine. This
 * module is that narrow surface, and its module graph deliberately excludes
 * the relay stack: `@openagentsinc/mkt-swp` (the default entry) reaches the
 * engine boundary and `nip-mkt`; `@openagentsinc/mkt-swp/view` does not.
 *
 * Import this from markup. Import the default entry from the host that owns
 * the engine layer and the relay subscription.
 */
export * from "./compose.js";
export * from "./primary-action.js";
export * from "./widget-state.js";
