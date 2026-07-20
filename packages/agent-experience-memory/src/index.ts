/**
 * `@openagentsinc/agent-experience-memory` — optional, owner-local, redacted
 * experience memory for the Apple FM router and the coding loop (AFS-10).
 *
 * Memory is a conservative, default-OFF addition. With the flag off, no bank is
 * frozen, no record is read or written, and no recalled slice enters any prompt:
 * the router and the IDE behave byte-identically to a build without this package.
 * Memory is never a dependency of a router or IDE path — it is only ever an
 * advisory input a host MAY consult when the owner turns it on.
 *
 * Guarantees:
 * - Redacted: every stored fact and every recalled slice passes the existing
 *   ATIF redaction boundary (`@openagentsinc/atif`). A secret, wallet or payment
 *   value, local path, token, or email never enters memory or a recall result.
 * - Owner-scoped and project-scoped: one owner scope never reads another owner's
 *   memory, and recall stays inside one project without a separate scope grant.
 * - Local-only: this portable package holds no cloud, SQL, provider, or Node
 *   host; the durable adapter that writes private local app storage lives in the
 *   app composition root. The Apple FM path stays strictly on-device.
 * - Frozen and one-shot: exactly one eligible bank is frozen at turn start, and
 *   at most one pre-turn adaptation runs, bound by an effective adaptation digest,
 *   so current-turn data cannot change current-turn input.
 * - Structured to measure: a benefit report compares flag-off and flag-on
 *   acceptance and correction deltas. Version one ships OFF with NO measured live
 *   benefit; the harness exists so a future promotion rests on evidence.
 *
 * This package reuses the reviewed algorithm and test ideas of the unwired Pylon
 * TAS kit; it imports nothing from it. The TAS files carry no schema,
 * persistence, consent, delete, or owner-scope authority.
 */
export * from "./contract/index.js";
export * from "./redaction.js";
export * from "./ranking.js";
export * from "./store.js";
export * from "./memory.js";
export * from "./measurement.js";
