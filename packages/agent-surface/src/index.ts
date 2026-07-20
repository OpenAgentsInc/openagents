import { Schema as S } from "effect";

import {
  MAX_TURN_OUTPUT_CHARS,
  RouteDecision,
  SafeMessageChainEntry,
  SafeTurnProjection,
  TurnRefusalReason,
  type AgentCardState,
  type TurnCostClass,
  type TurnDataDestination,
  type TurnProviderCandidate,
  type TurnStageKind,
  type TurnUsageTruth,
} from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/agent-surface` — the UI-neutral surface projectors (AFS-00
 * reservation, AFS-04 implementation).
 *
 * AFS-04 adds the first real projectors that turn canonical turn facts into safe
 * cards and message chains. This package owns pure projectors and surface-intent
 * helpers. It must not own schemas, renderers, or providers. It imports its
 * schemas from `@openagentsinc/agent-runtime-schema`; it must not define a second
 * wire contract.
 *
 * The redaction boundary lives here as CODE, not only as a schema shape. A
 * provider run reports an observed activity, and this module selects ONLY the
 * safe, bounded fields (role, bounded message text, a bounded tool LABEL, a file-
 * change COUNT, and a command-output BYTE COUNT). It never reads a raw command
 * argument, raw command output, a local path, a token, or a secret. An observed
 * activity may carry those fields; the projector structurally ignores every
 * field it does not name, so raw content can never reach a card or inspector.
 */
export const AGENT_SURFACE_PACKAGE = "@openagentsinc/agent-surface" as const;

const decodeMessageChainEntry = S.decodeUnknownSync(SafeMessageChainEntry);

/**
 * A safe, bounded card model a renderer shows for one turn or delegated agent.
 * It is display only. It is never proof, acceptance, delivery, or release. A
 * card cannot show `running` before a host start receipt: `cardState` is derived
 * deterministically from the driver-neutral projection, whose lifecycle only
 * reaches a running state after the provider start receipt exists.
 */
export interface SafeAgentCard {
  readonly requestRef: string;
  readonly threadRef: string;
  readonly providerTurnRef: string | null;
  readonly cardState: AgentCardState;
  readonly stage: TurnStageKind;
  readonly provider: TurnProviderCandidate | null;
  readonly dataDestination: TurnDataDestination;
  readonly usageTruth: TurnUsageTruth;
  readonly localOnly: boolean;
  readonly updatedAt: string;
  readonly messageCount: number;
}

/**
 * The projector surface reserved by AFS-00. A projector reads a safe turn
 * projection and emits a bounded card. The projector is a pure function; it
 * acquires no schema, renderer, or provider authority.
 */
export interface SafeCardProjector {
  readonly project: (projection: SafeTurnProjection) => {
    readonly cardState: AgentCardState;
    readonly stage: TurnStageKind;
  };
}

/** The AFS-04 card is always a `card` stage: a bounded UI projection, never an action or a release. */
const CARD_STAGE: TurnStageKind = "card";

/** Build the AFS-00 `SafeCardProjector`. It derives the bounded card facts only. */
export const makeSafeCardProjector = (): SafeCardProjector => ({
  project: (projection) => ({ cardState: projection.cardState, stage: CARD_STAGE }),
});

/** Project a safe turn projection into the richer bounded card model. */
export const projectSafeAgentCard = (projection: SafeTurnProjection): SafeAgentCard => ({
  requestRef: projection.requestRef,
  threadRef: projection.threadRef,
  providerTurnRef: projection.providerTurnRef ?? null,
  cardState: projection.cardState,
  stage: CARD_STAGE,
  provider: projection.candidate ?? null,
  dataDestination: projection.dataDestination,
  usageTruth: projection.usageTruth,
  localOnly: projection.localOnly,
  updatedAt: projection.updatedAt,
  messageCount: projection.messageChain.length,
});

/** True when a card is in a terminal (non-live) state. */
export const isTerminalCardState = (state: AgentCardState): boolean =>
  state === "done" || state === "refused" || state === "failed" || state === "cancelled";

/** True when a card may show a live running/queued affordance. */
export const isLiveCardState = (state: AgentCardState): boolean =>
  state === "queued" || state === "running";

/**
 * A pre-safe observed activity from a provider run. The projector reads ONLY the
 * named safe fields. Every other property — a raw command, raw output text, a
 * local path, a token, or any secret — is structurally ignored and can never
 * reach a card or inspector. The caller (a provider adapter) selects the safe
 * fields; this projector is the final gate that bounds and decodes them.
 */
export interface ObservedAgentActivity {
  readonly role: SafeMessageChainEntry["role"];
  /** A bounded message text (agent/user/system prose or a reasoning summary). */
  readonly text?: string;
  /** A bounded tool LABEL, never the tool arguments (for example `shell`, `apply_patch`). */
  readonly toolLabel?: string;
  /** The number of files a change touched, never the file contents or paths. */
  readonly fileChangeCount?: number;
  /** The byte COUNT of command output, never the output text. */
  readonly commandOutputByteCount?: number;
}

/** Deterministic, stable entry ref for a projected message-chain entry. */
const entryRef = (requestRef: string, index: number): string => `${requestRef}.chain.${index}`;

const boundedText = (value: string): string => value.slice(0, MAX_TURN_OUTPUT_CHARS);

const boundedLabel = (value: string): string => value.slice(0, 120);

const safeCount = (value: number): number => (Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0);

/**
 * Build ONE safe message-chain entry from an observed activity. It reads only
 * the named safe fields and bounds each. Any other field on `input` is ignored.
 * The result is decoded through the frozen `SafeMessageChainEntry` schema, so it
 * cannot carry a field outside the safe shape.
 */
export const buildSafeMessageChainEntry = (
  requestRef: string,
  index: number,
  input: ObservedAgentActivity,
): SafeMessageChainEntry =>
  decodeMessageChainEntry({
    entryRef: entryRef(requestRef, index),
    role: input.role,
    text: input.text === undefined ? "" : boundedText(input.text),
    ...(input.toolLabel === undefined ? {} : { toolLabel: boundedLabel(input.toolLabel) }),
    ...(input.fileChangeCount === undefined ? {} : { fileChangeCount: safeCount(input.fileChangeCount) }),
    ...(input.commandOutputByteCount === undefined
      ? {}
      : { commandOutputByteCount: safeCount(input.commandOutputByteCount) }),
  });

/**
 * Project a list of observed activities into a bounded safe message chain. The
 * chain is capped at the frozen retained-segment bound (256). It is the redacted,
 * owner-only message chain the right pane renders.
 */
export const projectSafeMessageChain = (
  requestRef: string,
  activities: ReadonlyArray<ObservedAgentActivity>,
): ReadonlyArray<SafeMessageChainEntry> =>
  activities.slice(0, 256).map((activity, index) => buildSafeMessageChainEntry(requestRef, index, activity));

/**
 * The safe message chain already carried by a turn projection. Every entry is
 * already the frozen safe shape; this is the read-side helper the inspector uses.
 */
export const safeMessageChainOf = (
  projection: SafeTurnProjection,
): ReadonlyArray<SafeMessageChainEntry> => projection.messageChain;

/**
 * AFS-12 cross-surface read/compose reader.
 *
 * The web and mobile hosts are READ/COMPOSE surfaces. They decode the SAME safe
 * projection, route decision, and recovery facts that Desktop decodes, and they
 * compose the SAME bounded cards and message chains — WITHOUT any turn
 * execution, provider dispatch, or action authority. This reader is the one
 * shared compose path all three surfaces reuse. It only decodes bytes with the
 * frozen schema and reuses the safe projectors above; it can never carry a
 * helper secret, a raw argument, raw output, a local path, or a token because it
 * structurally reads only the safe projection shape.
 */

/** The safe route-disclosure facts a read surface may show. Never a secret. */
export interface SafeRouteDisclosureFacts {
  readonly outcome: "admitted" | "closed";
  readonly decisionReason: string;
  readonly dataDestination: TurnDataDestination | null;
  readonly costClass: TurnCostClass | null;
  readonly localOnly: boolean | null;
  readonly providerRef: string | null;
  readonly selected: string | null;
  readonly effective: string | null;
  readonly admittedCandidateSet: ReadonlyArray<string>;
  readonly contextManifestRef: string;
  readonly dispositions: ReadonlyArray<{
    readonly candidate: string;
    readonly disposition: string;
    readonly reason: string;
  }>;
}

/** The safe recovery facts a read surface may show for a refused/failed/cancelled turn. */
export interface SafeTurnRecoveryFacts {
  readonly cardState: AgentCardState;
  readonly terminal: boolean;
  readonly refusalReason: TurnRefusalReason | null;
}

/** One decoded scenario's safe facts, equal across Desktop, web, and mobile. */
export interface SafeSurfaceScenarioFacts {
  readonly scenario: string;
  readonly card: SafeAgentCard;
  readonly messageChain: ReadonlyArray<SafeMessageChainEntry>;
  readonly route: SafeRouteDisclosureFacts | null;
  readonly recovery: SafeTurnRecoveryFacts;
}

/** The safe byte input for one cross-surface scenario. All fields are safe refs. */
export interface SafeSurfaceScenarioInput {
  readonly scenario: string;
  readonly projection: unknown;
  readonly routeDecision?: unknown;
  readonly refusalReason?: string;
}

const decodeSafeTurnProjection = S.decodeUnknownSync(SafeTurnProjection);
const decodeRouteDecision = S.decodeUnknownSync(RouteDecision);
const decodeRefusalReason = S.decodeUnknownSync(TurnRefusalReason);

/** Decode a safe route decision into bounded, secret-free disclosure facts. */
export const readSafeRouteDisclosure = (input: unknown): SafeRouteDisclosureFacts => {
  const decision = decodeRouteDecision(input);
  const dispositions = decision.dispositions.map((disposition) => ({
    candidate: disposition.candidate,
    disposition: disposition.disposition,
    reason: disposition.reason,
  }));
  if (decision.outcome === "admitted") {
    return {
      outcome: "admitted",
      decisionReason: decision.decisionReason,
      dataDestination: decision.disclosure.dataDestination,
      costClass: decision.disclosure.costClass,
      localOnly: decision.disclosure.localOnly,
      providerRef: decision.disclosure.providerRef ?? null,
      selected: decision.selected,
      effective: decision.effective,
      admittedCandidateSet: [...decision.admittedCandidateSet],
      contextManifestRef: decision.contextManifestRef,
      dispositions,
    };
  }
  return {
    outcome: "closed",
    decisionReason: decision.decisionReason,
    dataDestination: null,
    costClass: null,
    localOnly: null,
    providerRef: null,
    selected: null,
    effective: null,
    admittedCandidateSet: [],
    contextManifestRef: decision.contextManifestRef,
    dispositions,
  };
};

/**
 * Decode one scenario's safe bytes into the facts every surface must agree on.
 * It reuses `projectSafeAgentCard` and `safeMessageChainOf`, so a card can never
 * show `running` before a host start receipt exists.
 */
export const readSafeSurfaceScenario = (input: SafeSurfaceScenarioInput): SafeSurfaceScenarioFacts => {
  const projection = decodeSafeTurnProjection(input.projection);
  return {
    scenario: input.scenario,
    card: projectSafeAgentCard(projection),
    messageChain: safeMessageChainOf(projection),
    route: input.routeDecision === undefined ? null : readSafeRouteDisclosure(input.routeDecision),
    recovery: {
      cardState: projection.cardState,
      terminal: isTerminalCardState(projection.cardState),
      refusalReason: input.refusalReason === undefined ? null : decodeRefusalReason(input.refusalReason),
    },
  };
};

/** A compact, human-readable summary of one scenario's safe facts. */
export interface SurfaceFactSummary {
  readonly scenario: string;
  readonly cardState: AgentCardState;
  readonly terminal: boolean;
  readonly refusalReason: string | null;
  readonly messageCount: number;
  readonly provider: TurnProviderCandidate | null;
  readonly dataDestination: TurnDataDestination;
  readonly localOnly: boolean;
  readonly usageTruth: TurnUsageTruth;
  readonly routeOutcome: "admitted" | "closed" | null;
  readonly routeSelected: string | null;
  readonly routeDataDestination: TurnDataDestination | null;
  readonly routeLocalOnly: boolean | null;
  readonly contextManifestRef: string | null;
}

/** Reduce decoded safe facts to the compact cross-surface comparison tuple. */
export const summarizeSurfaceFacts = (facts: SafeSurfaceScenarioFacts): SurfaceFactSummary => ({
  scenario: facts.scenario,
  cardState: facts.card.cardState,
  terminal: facts.recovery.terminal,
  refusalReason: facts.recovery.refusalReason,
  messageCount: facts.card.messageCount,
  provider: facts.card.provider,
  dataDestination: facts.card.dataDestination,
  localOnly: facts.card.localOnly,
  usageTruth: facts.card.usageTruth,
  routeOutcome: facts.route?.outcome ?? null,
  routeSelected: facts.route?.selected ?? null,
  routeDataDestination: facts.route?.dataDestination ?? null,
  routeLocalOnly: facts.route?.localOnly ?? null,
  contextManifestRef: facts.route?.contextManifestRef ?? null,
});

/**
 * The safe-fact forbidden-material patterns. A decoded surface fact must never
 * contain a raw prompt, raw output, local path, token, or secret. A read surface
 * asserts this against the JSON of its decoded facts as a privacy-fence oracle.
 */
export const SAFE_SURFACE_FORBIDDEN_KEY_PATTERN =
  /(?:rawArgs|rawInput|rawOutput|commandOutput(?!ByteCount)|prompt|token|secret|apiKey|auth|mnemonic|localPath|helperPath|loopbackUrl|filePath|absolutePath)/iu;

export const SAFE_SURFACE_FORBIDDEN_VALUE_PATTERN =
  /(?:\/Users\/|\/home\/|sk-[a-z0-9]|ghp_|gho_|bearer\s|-----BEGIN|access[_-]?token|api[_-]?key)/iu;

/** True when decoded surface facts carry no forbidden key or value shape. */
export const surfaceFactsAreSecretFree = (facts: unknown): boolean => {
  const serialized = JSON.stringify(facts);
  if (serialized === undefined) return true;
  if (SAFE_SURFACE_FORBIDDEN_VALUE_PATTERN.test(serialized)) return false;
  for (const key of collectKeys(facts)) {
    if (SAFE_SURFACE_FORBIDDEN_KEY_PATTERN.test(key)) return false;
  }
  return true;
};

const collectKeys = (value: unknown): ReadonlyArray<string> => {
  if (Array.isArray(value)) return value.flatMap((item) => collectKeys(item));
  if (value !== null && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) => [
      key,
      ...collectKeys(child),
    ]);
  }
  return [];
};
