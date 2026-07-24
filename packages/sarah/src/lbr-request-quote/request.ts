/**
 * Sarah community work-request templates over NIP-LBR kind 5934.
 *
 * Builds and validates budgeted work requests that bind a narrow work-unit
 * grant. Protocol decode stays in `@openagentsinc/nip90`.
 */

import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_OUTPUT_DELIVERY_POLICY,
  LbrProtocolError,
  decodeLbrAgenticCodingRequestEvent,
  lbrAgenticCodingRequestToDraft,
  makeLbrAgenticCodingRequest,
  type LbrAgenticCodingRequest,
  type LbrUnsignedEventDraft,
} from "@openagentsinc/nip90";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import {
  assertLanePublicSafe,
  assertWorkUnitGrantFence,
  ensurePublicRef,
  ensurePublicRefs,
  failLane,
  nowUnixSeconds,
  paramValues,
  requiredParam,
  tagValues,
} from "./guards.ts";
import {
  SARAH_LBR_JOB_TYPE,
  SARAH_LBR_PARAM,
  SARAH_LBR_REQUEST_ALT,
  SARAH_LBR_REQUEST_QUOTE_SCHEMA,
  SARAH_LBR_SETTLEMENT_MODE_V1,
  type SarahLbrWorkRequestInput,
  type SarahLbrWorkUnitGrant,
  decodeSarahLbrWorkRequestInput,
} from "./types.ts";

export type SarahLbrWorkRequest = Readonly<{
  schema: typeof SARAH_LBR_REQUEST_QUOTE_SCHEMA;
  settlementMode: typeof SARAH_LBR_SETTLEMENT_MODE_V1;
  jobType: typeof SARAH_LBR_JOB_TYPE;
  kind: typeof LBR_AGENTIC_CODING_REQUEST_KIND;
  labor: LbrAgenticCodingRequest;
  workUnit: SarahLbrWorkUnitGrant;
  objectiveRef: string;
  verificationCommandRef: string;
  requiredCapabilityRefs: ReadonlyArray<string>;
  groupId?: string;
  groupRef?: string;
  forumTopicRef?: string;
  deadlineRef?: string;
}>;

export type SarahLbrWorkRequestBuild = Readonly<{
  request: SarahLbrWorkRequest;
  draft: LbrUnsignedEventDraft;
  template: SarahNostrEventTemplate;
}>;

const toNip90Input = (input: SarahLbrWorkRequestInput) => ({
  objectiveRef: input.objectiveRef,
  repositoryRefs: input.workUnit.repositoryRefs,
  verificationCommandRef: input.verificationCommandRef,
  requiredCapabilityRefs: input.requiredCapabilityRefs,
  bidMsats: input.workUnit.budgetMsats,
  ...(input.deadlineRef === undefined ? {} : { deadline: input.deadlineRef }),
  ...(input.forumTopicRef === undefined
    ? {}
    : { forumTopicRef: input.forumTopicRef }),
  ...(input.relays === undefined ? {} : { relays: input.relays }),
});

const appendLaneTags = (
  baseTags: ReadonlyArray<readonly string[]>,
  input: SarahLbrWorkRequestInput,
): ReadonlyArray<readonly string[]> => {
  const laneTags: Array<readonly string[]> = [
    ["alt", SARAH_LBR_REQUEST_ALT],
    ["param", SARAH_LBR_PARAM.workUnitRef, input.workUnit.workUnitRef],
    ["param", SARAH_LBR_PARAM.grantRef, input.workUnit.grantRef],
    ["param", SARAH_LBR_PARAM.idempotencyRef, input.workUnit.idempotencyRef],
    [
      "param",
      SARAH_LBR_PARAM.settlementMode,
      SARAH_LBR_SETTLEMENT_MODE_V1,
    ],
    ...input.workUnit.allowedActionRefs.map(
      (ref): readonly string[] => [
        "param",
        SARAH_LBR_PARAM.allowedActionRef,
        ref,
      ],
    ),
    ["expiration", String(input.workUnit.expiresAtUnix)],
  ];
  if (input.groupId !== undefined) {
    laneTags.push(["h", input.groupId]);
  }
  if (input.groupRef !== undefined) {
    laneTags.push(["param", SARAH_LBR_PARAM.groupRef, input.groupRef]);
  }
  return [...baseTags, ...laneTags];
};

/**
 * Build a NIP-LBR agentic-coding request for one Sarah community work unit.
 * Content is empty and ref-only. Settlement mode is forced to `no_spend`.
 */
const decodeRequestInput = (raw: unknown): SarahLbrWorkRequestInput => {
  try {
    return decodeSarahLbrWorkRequestInput(raw, {
      onExcessProperty: "error",
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (/unsafe|\/Users\/|\/home\/|SECRET|TOKEN|preimage|lnbc|raw prompt/i.test(message)) {
      return failLane("unsafe_material", message);
    }
    if (/pattern|public-safe|RegExp|objectiveRef|grantRef|workUnit/i.test(message)) {
      return failLane("invalid_ref", message);
    }
    return failLane("invalid_input", message);
  }
};

export const buildSarahLbrWorkRequest = (
  raw: SarahLbrWorkRequestInput | unknown,
): SarahLbrWorkRequestBuild => {
  const input = decodeRequestInput(raw);
  assertLanePublicSafe(input);
  assertWorkUnitGrantFence(input.workUnit);

  if (input.workUnit.expiresAtUnix <= 0) {
    failLane("invalid_expiration", "expiresAtUnix must be a positive unix second");
  }

  let labor: LbrAgenticCodingRequest;
  try {
    labor = makeLbrAgenticCodingRequest(toNip90Input(input));
  } catch (error) {
    if (error instanceof LbrProtocolError) {
      failLane(error.code, error.message);
    }
    throw error;
  }

  const baseDraft = lbrAgenticCodingRequestToDraft(labor);
  if (baseDraft.content !== "") {
    failLane("unsafe_content", "LBR request content must be empty");
  }

  const tags = appendLaneTags(baseDraft.tags, input);
  assertLanePublicSafe(tags);

  const draft: LbrUnsignedEventDraft = {
    kind: LBR_AGENTIC_CODING_REQUEST_KIND,
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

  const request: SarahLbrWorkRequest = {
    schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
    settlementMode: SARAH_LBR_SETTLEMENT_MODE_V1,
    jobType: SARAH_LBR_JOB_TYPE,
    kind: LBR_AGENTIC_CODING_REQUEST_KIND,
    labor,
    workUnit: {
      workUnitRef: input.workUnit.workUnitRef,
      grantRef: input.workUnit.grantRef,
      repositoryRefs: input.workUnit.repositoryRefs,
      allowedActionRefs: input.workUnit.allowedActionRefs,
      budgetMsats: input.workUnit.budgetMsats,
      expiresAtUnix: input.workUnit.expiresAtUnix,
      idempotencyRef: input.workUnit.idempotencyRef,
    },
    objectiveRef: input.objectiveRef,
    verificationCommandRef: input.verificationCommandRef,
    requiredCapabilityRefs: input.requiredCapabilityRefs,
    ...(input.groupId === undefined ? {} : { groupId: input.groupId }),
    ...(input.groupRef === undefined ? {} : { groupRef: input.groupRef }),
    ...(input.forumTopicRef === undefined
      ? {}
      : { forumTopicRef: input.forumTopicRef }),
    ...(input.deadlineRef === undefined
      ? {}
      : { deadlineRef: input.deadlineRef }),
  };

  return { request, draft, template };
};

const parseWorkUnitFromTags = (
  tags: ReadonlyArray<readonly string[]>,
  labor: LbrAgenticCodingRequest,
): SarahLbrWorkUnitGrant => {
  const workUnitRef = ensurePublicRef(
    requiredParam(tags, SARAH_LBR_PARAM.workUnitRef),
    "workUnitRef",
  );
  const grantRef = ensurePublicRef(
    requiredParam(tags, SARAH_LBR_PARAM.grantRef),
    "grantRef",
  );
  const idempotencyRef = ensurePublicRef(
    requiredParam(tags, SARAH_LBR_PARAM.idempotencyRef),
    "idempotencyRef",
  );
  const allowedActionRefs = ensurePublicRefs(
    paramValues(tags, SARAH_LBR_PARAM.allowedActionRef),
    "allowedActionRefs",
  );
  const expirationRaw = tagValues(tags, "expiration")[0];
  if (expirationRaw === undefined) {
    failLane("missing_expiration", "Sarah LBR request requires NIP-40 expiration");
  }
  const expiresAtUnix = Number(expirationRaw);
  if (!Number.isInteger(expiresAtUnix) || expiresAtUnix <= 0) {
    failLane("invalid_expiration", "expiration must be a positive unix second");
  }

  const workUnit: SarahLbrWorkUnitGrant = {
    workUnitRef,
    grantRef,
    repositoryRefs: [...labor.repositoryRefs],
    allowedActionRefs,
    budgetMsats: labor.bidMsats,
    expiresAtUnix,
    idempotencyRef,
  };
  assertWorkUnitGrantFence(workUnit);
  return workUnit;
};

/**
 * Decode a signed or unsigned NIP-01 event as a Sarah community work request.
 * Rejects paid settlement modes, Sarah grants, unsafe content, and non-LBR kinds.
 */
export const decodeSarahLbrWorkRequestEvent = (
  event: unknown,
): SarahLbrWorkRequest => {
  assertLanePublicSafe(event);

  let labor: LbrAgenticCodingRequest;
  try {
    labor = decodeLbrAgenticCodingRequestEvent(event);
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

  if (labor.outputDelivery !== LBR_OUTPUT_DELIVERY_POLICY) {
    failLane("invalid_output_delivery", "LBR output delivery must be output_only");
  }

  const workUnit = parseWorkUnitFromTags(tags, labor);
  const groupId = tagValues(tags, "h")[0];
  const groupRefRaw = paramValues(tags, SARAH_LBR_PARAM.groupRef)[0];

  return {
    schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
    settlementMode: SARAH_LBR_SETTLEMENT_MODE_V1,
    jobType: SARAH_LBR_JOB_TYPE,
    kind: LBR_AGENTIC_CODING_REQUEST_KIND,
    labor,
    workUnit,
    objectiveRef: labor.objectiveRef,
    verificationCommandRef: labor.verificationCommandRef,
    requiredCapabilityRefs: labor.requiredCapabilityRefs,
    ...(groupId === undefined ? {} : { groupId }),
    ...(groupRefRaw === undefined
      ? {}
      : { groupRef: ensurePublicRef(groupRefRaw, "groupRef") }),
    ...(labor.forumTopicRef === undefined
      ? {}
      : { forumTopicRef: labor.forumTopicRef }),
    ...(labor.deadline === undefined ? {} : { deadlineRef: labor.deadline }),
  };
};

/**
 * Refuse an expired work-unit grant. Expired units are not extended.
 */
export const assertSarahLbrRequestNotExpired = (
  request: SarahLbrWorkRequest,
  nowUnix: number = nowUnixSeconds(),
): void => {
  if (request.workUnit.expiresAtUnix <= nowUnix) {
    failLane(
      "grant_expired",
      `work unit ${request.workUnit.workUnitRef} expired at ${request.workUnit.expiresAtUnix}`,
    );
  }
};
