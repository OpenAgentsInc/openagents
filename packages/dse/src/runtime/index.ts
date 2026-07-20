/**
 * `@openagentsinc/dse/runtime` — resolve, verify, and predict.
 *
 * This subpath resolves and verifies released artifacts offline and runs the
 * `Predict` module through the injected model port. It has NO compile or
 * promotion authority and MUST NOT import `../optimizer`. It imports no Apple FM,
 * Desktop, Pylon, Blueprint, provider SDK, cloud client, or Node host.
 */
export * from "./model.js";
export * from "./predict.js";
export * from "./resolver.js";
