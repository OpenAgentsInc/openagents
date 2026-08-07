/**
 * The exported session view-model (SWAP-0, openagents#9315; rollout plan
 * §2.2).
 *
 * This is the one typed contract both renderers consume: the Effect Native /
 * React web components on `openagents.com`, and Omega's `market_ui` GPUI
 * components on the desktop. It is the artifact shared across surfaces in
 * place of one shared widget toolkit — the plan's original "one GPUI
 * component set" narrowed to this, because a WebGPU canvas is the wrong
 * thing on the page a user moves money on.
 *
 * It is a COMPOSITION, not a restatement. Status lanes, gaps, forks, rungs,
 * ladders, and closes are SWAP-6's `SwapProgressView`; the verify-before-fund
 * gate is SWAP-3's `FundingGate`; the funding typestate and exit package are
 * the engine's. Nothing that another package owns is redefined here, so a
 * renderer that holds this object holds exactly what those packages proved.
 *
 * What this module adds is what no single sibling can own: the identity of
 * the session, the widget state the surfaces must agree on, and the
 * always-rendered primary action.
 */
import { Schema } from "effect";
import type { FundingGate } from "@openagentsinc/mkt-swp-compare";
import type { SwapProgressView } from "@openagentsinc/mkt-swp-status";
import {
  ImmortalExitPackageInspectionSchema,
  ImmortalFundingRequestSchema,
} from "./immortal-browser-abi.js";
import type { PrimaryActionModel } from "./primary-action.js";
import type { SwapWidgetState } from "./widget-state.js";

export const SESSION_VIEW_MODEL_VERSION = "openagents.mkt_swp.session_view.v1" as const;

const Hex64Schema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));

/**
 * MKT-SWP §3.1 asset identity. Tickers, display names, symbols, and
 * provider-local pair codes are labels only and are never used for matching,
 * grouping, pricing, or replay identity.
 */
export const AssetIdSchema = Schema.String.check(
  Schema.isPattern(/^swp:1:bip122:[0-9a-f]{32}:btc:(chain|lightning)$/),
);
export type AssetId = typeof AssetIdSchema.Type;

/** MKT-SWP §3.2 canonical amount: decimal satoshi string, never a number. */
export const AtomicAmountSchema = Schema.String.check(Schema.isPattern(/^(0|[1-9][0-9]*)$/));
export type AtomicAmount = typeof AtomicAmountSchema.Type;

export const SwapRoleSchema = Schema.Literals(["requester", "provider"]);
export type SwapRole = typeof SwapRoleSchema.Type;

/** One ordered side of the pair, with the amount once it is known. */
export const AssetSideSchema = Schema.Struct({
  assetId: AssetIdSchema,
  amount: Schema.optionalKey(AtomicAmountSchema),
});
export interface AssetSide extends Schema.Schema.Type<typeof AssetSideSchema> {}

/** The identity and terms half of the session — the schema-checkable part. */
export const SwapSessionIdentitySchema = Schema.Struct({
  schemaVersion: Schema.Literal(SESSION_VIEW_MODEL_VERSION),
  sessionId: Hex64Schema,
  role: SwapRoleSchema,
  send: AssetSideSchema,
  receive: AssetSideSchema,
  quoteEventId: Schema.optionalKey(Hex64Schema),
  orderEventId: Schema.optionalKey(Hex64Schema),
  exitPackageInspection: Schema.optionalKey(ImmortalExitPackageInspectionSchema),
  fundingRequest: Schema.optionalKey(ImmortalFundingRequestSchema),
});
export interface SwapSessionIdentity extends Schema.Schema.Type<typeof SwapSessionIdentitySchema> {}

/**
 * The complete exported view-model. The identity half is schema-decodable
 * (it crosses storage and process boundaries with SWAP-5's session store);
 * the projection half is the siblings' own computed types, held by
 * reference so a renderer cannot receive a stale re-encoding of them.
 */
export interface SwapSessionViewModel {
  readonly identity: SwapSessionIdentity;
  /** SWAP-0: the state both surfaces render. */
  readonly widgetState: SwapWidgetState;
  /** SWAP-0: the always-rendered action, already resolved to the locale. */
  readonly primaryAction: PrimaryActionModel;
  /** SWAP-3: why funding is or is not authorised, row-addressable. */
  readonly fundingGate: FundingGate | null;
  /** SWAP-6: per-signer lanes, gaps, forks, rungs, ladder, closes. */
  readonly progress: SwapProgressView | null;
}

export const decodeSwapSessionIdentity = Schema.decodeUnknownEffect(SwapSessionIdentitySchema);

/**
 * Assemble the exported view-model. Deliberately a plain function over
 * already-computed parts: every input is some package's proven output, and
 * this composition adds no verdict of its own.
 */
export const swapSessionViewModel = (parts: {
  readonly identity: SwapSessionIdentity;
  readonly widgetState: SwapWidgetState;
  readonly primaryAction: PrimaryActionModel;
  readonly fundingGate?: FundingGate | null;
  readonly progress?: SwapProgressView | null;
}): SwapSessionViewModel => ({
  identity: parts.identity,
  widgetState: parts.widgetState,
  primaryAction: parts.primaryAction,
  fundingGate: parts.fundingGate ?? null,
  progress: parts.progress ?? null,
});
