// Thin re-export of the canonical provider-account runtime security contract.
//
// The single authority now lives at
// `@openagentsinc/provider-account-schema/runtime`
// (packages/provider-account-schema/src/runtime.ts). This file is preserved
// only so existing `../contracts/provider-account` import sites keep resolving
// to the same contract. Do NOT add contract definitions here — the drift guard
// (scripts/check-contract-drift.mjs) fails the build if this file stops being a
// pure re-export.
export * from "@openagentsinc/provider-account-schema/runtime";
