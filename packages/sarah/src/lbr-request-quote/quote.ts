/**
 * Sarah community quote templates over NIP-LBR feedback kind 7000.
 *
 * Quotes bind to a request and work unit. They do not reserve funds and do
 * not grant settlement authority.
 */

import {
  LBR_FEEDBACK_KIND,
  LbrProtocolError,
  decodeLbrQuoteEvent,
  lbrQuoteToDraft,
  makeLbrQuote,
  type LbrQuote,
  type LbrUnsignedEventDraft,
} from "@openagentsinc/nip90";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import {
  assertLanePublicSafe,
  assertNotSarahGrant,
  ensurePublicRef,
  failLane,
  nowUnixSeconds,
  paramValues,
  requiredParam,
} from "./guards.ts";
import type { SarahLbrWorkRequest } from "./request.ts";
import {
  SARAH_LBR_PARAM,
  SARAH_LBR_QUOTE_ALT,
  SARAH_LBR_REQUEST_QUOTE_SCHEMA,
  SARAH_LBR_SETTLEMENT_MODE_V1,
  type SarahLbrQuoteInput,
  decodeSarahLbrQuoteInput,
} from "./types.ts";

export type SarahLbrQuote = Readonly<{
  schema: typeof SARAH_LBR_REQUEST_QUOTE_SCHEMA;
  settlementMode: typeof SARAH_LBR_SETTLEMENT_MODE_V1;
  kind: typeof LBR_FEEDBACK_KIND;
  labor: LbrQuote;
  requestId: string;
  requesterPubkey: string;
  workUnitRef: string;
  amountMsats: number;
  providerRef: string;
  capabilityRefs: ReadonlyArray<string>;
  quoteRef: string;
  operatorRef?: string;
  expiresAtRef?: string;
}>;

export type SarahLbrQuoteBuild = Readonly<{
  quote: SarahLbrQuote;
  draft: LbrUnsignedEventDraft;
  template: SarahNostrEventTemplate;
}>;

/**
 * Build a NIP-LBR quote for a Sarah community work request.
 * Amount is in millisatoshis and must later be checked against the request bid.
 */
const decodeQuoteInput = (raw: unknown): SarahLbrQuoteInput => {
  try {
    return decodeSarahLbrQuoteInput(raw, { onExcessProperty: "error" });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (/unsafe|\/Users\/|\/home\/|SECRET|TOKEN|preimage|lnbc|raw prompt/i.test(message)) {
      return failLane("unsafe_material", message);
    }
    if (/pattern|public-safe|RegExp|workUnit|providerRef|quoteRef/i.test(message)) {
      return failLane("invalid_ref", message);
    }
    return failLane("invalid_input", message);
  }
};

export const buildSarahLbrQuote = (
  raw: SarahLbrQuoteInput | unknown,
): SarahLbrQuoteBuild => {
  const input = decodeQuoteInput(raw);
  assertLanePublicSafe(input);
  assertNotSarahGrant(input.workUnitRef, "workUnitRef");
  assertNotSarahGrant(input.providerRef, "providerRef");
  assertNotSarahGrant(input.quoteRef, "quoteRef");
  if (input.operatorRef !== undefined) {
    assertNotSarahGrant(input.operatorRef, "operatorRef");
  }

  let labor: LbrQuote;
  try {
    labor = makeLbrQuote({
      requestId: input.requestId,
      requesterPubkey: input.requesterPubkey,
      amountMsats: input.amountMsats,
      providerRef: input.providerRef,
      capabilityRefs: input.capabilityRefs,
      quoteRef: input.quoteRef,
      ...(input.expiresAtRef === undefined
        ? {}
        : { expiresAt: input.expiresAtRef }),
      ...(input.requestRelay === undefined
        ? {}
        : { requestRelay: input.requestRelay }),
    });
  } catch (error) {
    if (error instanceof LbrProtocolError) {
      failLane(error.code, error.message);
    }
    throw error;
  }

  const baseDraft = lbrQuoteToDraft(labor);
  if (baseDraft.content !== "") {
    failLane("unsafe_content", "LBR quote content must be empty");
  }

  const tags: Array<readonly string[]> = [
    ...baseDraft.tags,
    ["alt", SARAH_LBR_QUOTE_ALT],
    ["param", SARAH_LBR_PARAM.workUnitRef, input.workUnitRef],
    [
      "param",
      SARAH_LBR_PARAM.settlementMode,
      SARAH_LBR_SETTLEMENT_MODE_V1,
    ],
  ];
  if (input.operatorRef !== undefined) {
    tags.push(["param", SARAH_LBR_PARAM.operatorRef, input.operatorRef]);
  }

  assertLanePublicSafe(tags);

  const draft: LbrUnsignedEventDraft = {
    kind: LBR_FEEDBACK_KIND,
    tags,
    content: "",
  };

  const createdAt = input.createdAt ?? nowUnixSeconds();
  const template: SarahNostrEventTemplate = {
    kind: draft.kind,
    created_at: createdAt,
    tags: draft.tags.map((tag) => [...tag]),
    content: "",
  };

  const quote: SarahLbrQuote = {
    schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
    settlementMode: SARAH_LBR_SETTLEMENT_MODE_V1,
    kind: LBR_FEEDBACK_KIND,
    labor,
    requestId: labor.requestId,
    requesterPubkey: labor.requesterPubkey,
    workUnitRef: input.workUnitRef,
    amountMsats: labor.amountMsats,
    providerRef: labor.providerRef,
    capabilityRefs: labor.capabilityRefs,
    quoteRef: labor.quoteRef,
    ...(input.operatorRef === undefined
      ? {}
      : { operatorRef: input.operatorRef }),
    ...(labor.expiresAt === undefined ? {} : { expiresAtRef: labor.expiresAt }),
  };

  return { quote, draft, template };
};

/**
 * Decode a signed or unsigned NIP-01 event as a Sarah community quote.
 */
export const decodeSarahLbrQuoteEvent = (event: unknown): SarahLbrQuote => {
  assertLanePublicSafe(event);

  let labor: LbrQuote;
  try {
    labor = decodeLbrQuoteEvent(event);
  } catch (error) {
    if (error instanceof LbrProtocolError) {
      failLane(error.code, error.message);
    }
    throw error;
  }

  const tags = Array.isArray((event as { tags?: unknown }).tags)
    ? ((event as { tags: ReadonlyArray<readonly string[]> }).tags)
    : failLane("invalid_event", "event tags must be an array");

  const settlementMode = requiredParam(tags, SARAH_LBR_PARAM.settlementMode);
  if (settlementMode !== SARAH_LBR_SETTLEMENT_MODE_V1) {
    failLane(
      "settlement_forbidden",
      `v1 Sarah LBR lane requires settlement mode ${SARAH_LBR_SETTLEMENT_MODE_V1}`,
    );
  }

  const workUnitRef = ensurePublicRef(
    requiredParam(tags, SARAH_LBR_PARAM.workUnitRef),
    "workUnitRef",
  );
  assertNotSarahGrant(workUnitRef, "workUnitRef");

  const operatorRaw = paramValues(tags, SARAH_LBR_PARAM.operatorRef)[0];

  return {
    schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
    settlementMode: SARAH_LBR_SETTLEMENT_MODE_V1,
    kind: LBR_FEEDBACK_KIND,
    labor,
    requestId: labor.requestId,
    requesterPubkey: labor.requesterPubkey,
    workUnitRef,
    amountMsats: labor.amountMsats,
    providerRef: labor.providerRef,
    capabilityRefs: labor.capabilityRefs,
    quoteRef: labor.quoteRef,
    ...(operatorRaw === undefined
      ? {}
      : {
          operatorRef: (() => {
            const ref = ensurePublicRef(operatorRaw, "operatorRef");
            assertNotSarahGrant(ref, "operatorRef");
            return ref;
          })(),
        }),
    ...(labor.expiresAt === undefined
      ? {}
      : { expiresAtRef: labor.expiresAt }),
  };
};

/**
 * Bind a quote to its request: matching request id, work unit, requester, and
 * amount within the request budget. Quotes do not settle.
 */
export const validateSarahLbrQuoteAgainstRequest = (
  quote: SarahLbrQuote,
  request: SarahLbrWorkRequest,
  options: Readonly<{
    /** Hex event id of the request when known (required for live binding). */
    requestEventId?: string;
    nowUnix?: number;
  }> = {},
): void => {
  if (options.requestEventId !== undefined) {
    if (!/^[0-9a-f]{64}$/.test(options.requestEventId)) {
      failLane("invalid_request_id", "requestEventId must be 32-byte hex");
    }
    if (quote.requestId !== options.requestEventId.toLowerCase()) {
      failLane(
        "request_mismatch",
        "quote requestId must equal the request event id",
      );
    }
  }

  if (quote.workUnitRef !== request.workUnit.workUnitRef) {
    failLane(
      "work_unit_mismatch",
      "quote workUnitRef must equal the request work unit",
    );
  }

  if (quote.amountMsats > request.workUnit.budgetMsats) {
    failLane(
      "over_budget",
      `quote amount ${quote.amountMsats} exceeds request budget ${request.workUnit.budgetMsats}`,
    );
  }

  if (quote.settlementMode !== SARAH_LBR_SETTLEMENT_MODE_V1) {
    failLane("settlement_forbidden", "quote settlement mode must be no_spend");
  }
  if (request.settlementMode !== SARAH_LBR_SETTLEMENT_MODE_V1) {
    failLane(
      "settlement_forbidden",
      "request settlement mode must be no_spend",
    );
  }

  const now = options.nowUnix ?? nowUnixSeconds();
  if (request.workUnit.expiresAtUnix <= now) {
    failLane(
      "grant_expired",
      `cannot quote expired work unit ${request.workUnit.workUnitRef}`,
    );
  }

  // Member-submitted quote material is untrusted data, never Sarah instructions.
  assertLanePublicSafe({
    providerRef: quote.providerRef,
    quoteRef: quote.quoteRef,
    capabilityRefs: quote.capabilityRefs,
    ...(quote.operatorRef === undefined
      ? {}
      : { operatorRef: quote.operatorRef }),
  });
};
