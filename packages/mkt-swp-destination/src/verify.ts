/**
 * The engine verification port (issue #9317 §4–§5).
 *
 * Local address and invoice verification is engine work: the MKT-SWP
 * engine (Immortal client crate behind the SWAP-0 boundary) parses scripts
 * and Taproot trees from bytes, re-derives the output key/address from
 * internal keys, tree, network, and tweak (§7.1 steps 3–5), and parses the
 * complete invoice (§7.2). The host must not re-implement those checks —
 * this file defines only the typed verdict the entry state consumes, and
 * the entry state keeps funding ineligible unless the verdict is a pass.
 *
 * Provider-reported success is never a substitute for the local view; a
 * verdict here is only ever the local engine's.
 */
import { Effect, Schema } from "effect";

import type { SwpDestinationErrorId } from "./model.js";

export const SwpDestinationErrorIdSchema = Schema.Literals([
  "swp_invoice_invalid",
  "swp_script_invalid",
  "swp_script_commitment_mismatch",
  "swp_invalid_asset_id",
]);

/** A verification verdict, tagged with the entry epoch it was computed for. */
export type DestinationVerdict =
  | { readonly verdict: "pass"; readonly epoch: number }
  | {
      readonly verdict: "fail";
      readonly epoch: number;
      /** MKT-SWP §17 identifier — always the typed identifier, never prose. */
      readonly error: SwpDestinationErrorId;
      /** Which §7.1/§7.2 check failed, for the checklist surface (SWAP-3). */
      readonly failedCheck: string | null;
    };

export class DestinationVerifierUnavailable extends Schema.TaggedErrorClass<DestinationVerifierUnavailable>()(
  "DestinationVerifierUnavailable",
  { reason: Schema.Literals(["engine_loading", "engine_failed"]) },
) {}

export interface VerifyAddressRequest {
  /** The counterparty-supplied address, as parsed text. */
  readonly address: string;
  /** Serialized script/tree bytes from the Swap Contract, hex. */
  readonly scriptBytesHex: string;
  readonly epoch: number;
}

export interface VerifyInvoiceRequest {
  readonly invoice: string;
  /**
   * Expected payment hash from the session, hex. Produced by the entry
   * reducer's `request_verification` intent, which threads it from
   * `EntryConfig.expectedPaymentHashHex` (the shell supplies the session's
   * value). Expiry, minimum final CLTV delta, and route-hint policy have
   * no comparand here on purpose: the engine checks them against the
   * quote inside `verify_before_fund` (MKT-SWP §7.2).
   */
  readonly expectedPaymentHashHex: string | null;
  readonly epoch: number;
}

/**
 * Implemented by the SWAP-0 engine binding. Both operations resolve to a
 * verdict; transport/engine unavailability is the only failure channel so
 * a missing engine can never masquerade as a pass.
 */
export interface DestinationVerifier {
  readonly verifyAddress: (
    request: VerifyAddressRequest,
  ) => Effect.Effect<DestinationVerdict, DestinationVerifierUnavailable>;
  readonly verifyInvoice: (
    request: VerifyInvoiceRequest,
  ) => Effect.Effect<DestinationVerdict, DestinationVerifierUnavailable>;
}
