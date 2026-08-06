/**
 * The verify-before-fund engine port and the funding gate
 * (openagents#9318 §6, MKT-SWP §7, mkt-swp-client.md).
 *
 * Verification truth is ENGINE truth. The MKT-SWP client engine (Immortal
 * client crate behind the SWAP-0 boundary) performs the §7.1–§7.5 checks —
 * signatures and causal references, recomputed external-effect IDs, scripts
 * and trees parsed from bytes, re-derived output key, payment hash and
 * timelocks, timeout-ladder inequalities, exit-package digest, and the
 * confirmation/RBF/reorg policy — and reaches `verification_passed` or not.
 * This file defines only the typed report the UI consumes and the gate that
 * keeps the fund action disabled.
 *
 * The UI NEVER computes a profile-level verdict itself. `fundingGate` is a
 * UX pre-check in the SWAP-2 sense: it can only keep funding disabled — on
 * a missing report, a stale epoch, an unresolved or failed row, or an
 * engine verdict that is not `verification_passed`. It has no branch that
 * turns anything other than a complete engine pass into an enabled action.
 *
 * Boltz redirects to a generic error page on re-derivation mismatch and the
 * user never learns which check failed; here every row keeps its identity
 * (§7 check id + §17 identifier) all the way to the rendered checklist.
 *
 * Behaviour contract:
 * `openagents_web.swap_compare.funding_disabled_until_checks_pass.v1`.
 */
import type { SwpErrorIdentifier } from "@openagentsinc/swap-i18n";
import { Effect, Schema } from "effect";

/**
 * The §7.1–§7.5 checklist rows, in profile order. Rail- and path-specific
 * rows are marked `pass` explicitly when they do not apply, so an incomplete
 * report can never read as complete.
 */
export const VERIFY_CHECK_IDS = [
  /** §7.1.1 signatures, causal references, profile tuple, expiry, terms. */
  "signatures_and_references",
  /** §7.1.2 recomputed external-effect IDs (§13). */
  "external_effect_ids",
  /** §7.1 / §7.5 exact funding transaction bytes and required signatures. */
  "funding_transaction",
  /** §7.1.3 scripts and Taproot trees parsed from bytes, never addresses. */
  "script_tree_parsed",
  /** §7.1.4 output key/address re-derived from keys, tree, network, tweak. */
  "output_key_rederived",
  /** §7.1.5 payment hash, claim/refund keys, timelocks, leaf, sighash, amount. */
  "terms_against_quote",
  /** §7.1.6 / §8 timeout-ladder inequalities against chain + invoice state. */
  "timeout_ladder",
  /** §7.1.7 / §12 exit package built, persisted, digest-checked. */
  "exit_package",
  /** §7.1.8 confirmation, RBF, replacement, reorg, relay-fee, dust policy. */
  "chain_policy",
  /** §7.5 local full node accepts the exact transaction under mempool policy. */
  "mempool_acceptance",
  /** §7.1.9 unknown versions / hidden composition / non-null evm_leg refused. */
  "unsupported_constructs_refused",
  /** §7.2 complete local invoice parse and coupling checks. */
  "lightning_invoice",
  /** §7.3 MuSig2 keys, ordering, tweak, message, nonces, partials. */
  "musig_transcript",
] as const;

export type VerifyCheckId = (typeof VERIFY_CHECK_IDS)[number];

export type VerifyCheckStatus = "unresolved" | "pass" | "fail";

/** One checklist row as reported by the engine. */
export type VerifyCheckRow =
  | {
      readonly id: VerifyCheckId;
      readonly status: "unresolved" | "pass";
    }
  | {
      readonly id: VerifyCheckId;
      readonly status: "fail";
      /** MKT-SWP §17 identifier — always typed, never prose. */
      readonly error: SwpErrorIdentifier;
    };

/**
 * The engine's verify-before-fund report for one session, tagged with the
 * comparison epoch it was computed for. `verdict` is the engine's own
 * profile-level result (`verification_passed` is the §7 state name); the
 * rows are its per-check evidence. The UI re-derives neither.
 */
export interface VerifyBeforeFundReport {
  readonly quoteEventId: string;
  readonly orderEventId: string | null;
  readonly epoch: number;
  readonly rows: readonly VerifyCheckRow[];
  readonly verdict: "verification_passed" | "verification_blocked";
}

export class FundVerifierUnavailable extends Schema.TaggedErrorClass<FundVerifierUnavailable>()(
  "FundVerifierUnavailable",
  { reason: Schema.Literals(["engine_loading", "engine_failed"]) },
) {}

export interface VerifySessionRequest {
  readonly quoteEventId: string;
  readonly orderEventId: string | null;
  readonly epoch: number;
}

/**
 * Implemented by the SWAP-0 engine binding. Unavailability is the only
 * failure channel, so a missing engine can never masquerade as a pass.
 */
export interface FundVerifier {
  readonly verifySession: (
    request: VerifySessionRequest,
  ) => Effect.Effect<VerifyBeforeFundReport, FundVerifierUnavailable>;
}

/** Why funding is disabled, row-addressable for the checklist surface. */
export interface FundingDisabled {
  readonly enabled: false;
  /** §17: verify-before-fund authority is absent. */
  readonly error: "swp_funding_not_authorized";
  readonly reason: "no_report" | "stale_report" | "engine_verdict_blocked" | "rows_incomplete";
  /** Ids of rows still unresolved (empty when reason is no/stale report). */
  readonly unresolved: readonly VerifyCheckId[];
  /** Failed rows with their typed identifiers, individually identifiable. */
  readonly failed: readonly Extract<VerifyCheckRow, { status: "fail" }>[];
}

export interface FundingEnabled {
  readonly enabled: true;
  readonly reportEpoch: number;
}

export type FundingGate = FundingDisabled | FundingEnabled;

const disabled = (
  reason: FundingDisabled["reason"],
  unresolved: readonly VerifyCheckId[] = [],
  failed: readonly Extract<VerifyCheckRow, { status: "fail" }>[] = [],
): FundingDisabled => ({
  enabled: false,
  error: "swp_funding_not_authorized",
  reason,
  unresolved,
  failed,
});

/**
 * The one gate in front of the fund action. Fail-closed in every direction:
 *
 * - no report for the current epoch → disabled (`swp_funding_not_authorized`);
 * - report from a superseded epoch → disabled;
 * - any expected row missing, unresolved, or failed → disabled;
 * - engine verdict not `verification_passed` → disabled, even if every
 *   visible row reads pass (the engine's verdict is authoritative; rows are
 *   its evidence, and a disagreement is treated as a blocked verdict).
 *
 * Enabling requires the engine verdict AND a complete all-pass row set for
 * the current epoch. There is no path from UI-side state to `enabled`.
 */
export const fundingGate = (
  report: VerifyBeforeFundReport | null,
  currentEpoch: number,
): FundingGate => {
  if (report === null) return disabled("no_report");
  if (report.epoch !== currentEpoch) return disabled("stale_report");

  const byId = new Map(report.rows.map((row) => [row.id, row]));
  const unresolved: VerifyCheckId[] = [];
  const failed: Extract<VerifyCheckRow, { status: "fail" }>[] = [];
  for (const id of VERIFY_CHECK_IDS) {
    const row = byId.get(id);
    if (row === undefined || row.status === "unresolved") unresolved.push(id);
    else if (row.status === "fail") failed.push(row);
  }
  if (unresolved.length > 0 || failed.length > 0) {
    return disabled("rows_incomplete", unresolved, failed);
  }
  if (report.verdict !== "verification_passed") {
    return disabled("engine_verdict_blocked");
  }
  return { enabled: true, reportEpoch: report.epoch };
};
