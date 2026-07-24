/**
 * Sarah NIP-LBR request and quote lane — SARAH-CW-04.
 *
 * Thin composition over `@openagentsinc/nip90` LBR helpers. Does not reimplement
 * NIP-90/NIP-LBR protocol validation. Does not wire payment or settlement.
 *
 * @see docs/nips/LBR.md
 * @see docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §32
 */

export {
  assertLanePublicSafe,
  assertNotSarahGrant,
  assertWorkUnitGrantFence,
  ensureNoUnsafeMaterial,
  ensurePublicRef,
  ensurePublicRefs,
  failLane,
  nowUnixSeconds,
} from "./guards.ts";

export {
  assertSarahLbrRequestNotExpired,
  buildSarahLbrWorkRequest,
  decodeSarahLbrWorkRequestEvent,
  type SarahLbrWorkRequest,
  type SarahLbrWorkRequestBuild,
} from "./request.ts";

export {
  buildSarahLbrQuote,
  decodeSarahLbrQuoteEvent,
  validateSarahLbrQuoteAgainstRequest,
  type SarahLbrQuote,
  type SarahLbrQuoteBuild,
} from "./quote.ts";

export {
  SARAH_LBR_FORBIDDEN_GRANT_PREFIXES,
  SARAH_LBR_JOB_TYPE,
  SARAH_LBR_PARAM,
  SARAH_LBR_PUBLIC_REF_PATTERN,
  SARAH_LBR_QUOTE_ALT,
  SARAH_LBR_REQUEST_ALT,
  SARAH_LBR_REQUEST_QUOTE_ISSUE,
  SARAH_LBR_REQUEST_QUOTE_PACKET,
  SARAH_LBR_REQUEST_QUOTE_SCHEMA,
  SARAH_LBR_SETTLEMENT_MODE_V1,
  SarahLbrLaneError,
  SarahLbrQuoteInput,
  SarahLbrWorkRequestInput,
  SarahLbrWorkUnitGrant,
  decodeSarahLbrQuoteInput,
  decodeSarahLbrWorkRequestInput,
  decodeSarahLbrWorkUnitGrant,
  type SarahLbrQuoteInput as SarahLbrQuoteInputType,
  type SarahLbrWorkRequestInput as SarahLbrWorkRequestInputType,
  type SarahLbrWorkUnitGrant as SarahLbrWorkUnitGrantType,
} from "./types.ts";

/** Re-export pinned LBR kinds from the canonical package for lane consumers. */
export {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  LBR_OUTPUT_DELIVERY_POLICY,
  LBR_RESERVED_LABOR_KIND_MAX,
  LBR_RESERVED_LABOR_KIND_MIN,
} from "@openagentsinc/nip90";
