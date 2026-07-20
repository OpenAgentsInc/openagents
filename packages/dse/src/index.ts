/**
 * `@openagentsinc/dse` — the Effect v4 DSE successor (AFS-08).
 *
 * The root export is the portable contract plus the offline-resolution runtime.
 * It deliberately does NOT re-export `./optimizer`: a runtime app resolves and
 * verifies a released artifact and runs `Predict`, but never links the compiler,
 * evaluator, search, or promotion authority. Import `@openagentsinc/dse/optimizer`
 * explicitly for the offline compile side.
 *
 * The whole package is offline and portable: it imports no Apple FM, Desktop,
 * Pylon, Blueprint, provider SDK, cloud client, or Node host. Its output is a
 * checked-in compiled artifact and a release record, resolved offline from bytes.
 */
export * from "./contract/index.js";
export * from "./runtime/index.js";
