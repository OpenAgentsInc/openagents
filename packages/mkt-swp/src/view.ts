/**
 * The presentation entry point (SWAP-0, openagents#9315).
 *
 * A rendering surface needs the typed state, the composition order, and the
 * primary-action law — and no transport or engine invocation. This module is
 * that narrow surface. Its `AwaitingFunding` state shares the production ABI's
 * exact prepared-effect schema, but it cannot load WASM or perform an effect.
 *
 * Import this from markup. Import the default entry from the host that owns
 * the engine layer and the relay subscription.
 */
export * from "./compose.js";
export * from "./primary-action.js";
export * from "./widget-state.js";
