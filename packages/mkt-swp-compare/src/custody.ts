/**
 * Custody classification for the per-route custody strip
 * (openagents#9318 §5, MKT-SWP §6).
 *
 * One law dominates this file: the UI must never let a custodial route read
 * as noncustodial. A mint/federation route is custodial — MKT-MINT enforces
 * `custody_class` at the relay — and the classification here is fail-closed:
 * a route reads `noncustodial` only when every control entry across
 * funds control, execution control, and settlement authority is held by a
 * principal, the verified contract construction, chain consensus, or
 * Lightning HTLC rules. Any mint, federation, third party, or UNRECOGNISED
 * holder classifies the route custodial.
 *
 * The strip never collapses the six §6 dimensions into one score: the view
 * carries all six arrays plus BOTH custody-duration bounds (wall-clock
 * estimate and exact height bound) as separate fields.
 */
import type {
  CustodyDisclosure,
  CustodyEntry,
  CustodyHolder,
} from "./model.js";

export type CustodyStripClass = "custodial" | "noncustodial";

/**
 * Holders compatible with a noncustodial reading. Everything outside this
 * set — including `unknown` — is custodial. Extending this set is a policy
 * change; see the package tests and INVARIANTS discipline.
 */
const NONCUSTODIAL_HOLDERS: ReadonlySet<CustodyHolder> = new Set([
  "requester",
  "provider",
  "contract",
  "consensus",
  "lightning_htlc",
]);

const custodialEntries = (
  entries: readonly CustodyEntry[],
): readonly CustodyEntry[] =>
  entries.filter(entry => !NONCUSTODIAL_HOLDERS.has(entry.holder));

/** The classified strip: class plus the entries that forced it. */
export interface CustodyStrip {
  readonly stripClass: CustodyStripClass;
  /**
   * Every control entry that forced the custodial classification, with the
   * dimension it came from — the strip names WHO holds custody, it does not
   * just say "custodial".
   */
  readonly custodialControls: readonly {
    readonly dimension:
      | "funds_control"
      | "execution_control"
      | "settlement_authority";
    readonly entry: CustodyEntry;
  }[];
}

/**
 * Classify one Quote's custody disclosure, fail-closed. Only the three
 * control dimensions decide the class; `reversibility` and `recourse` are
 * disclosure rows, not control claims, and cannot make a route read
 * noncustodial (nothing can except the absence of custodial control).
 */
export const custodyStrip = (custody: CustodyDisclosure): CustodyStrip => {
  const custodialControls = [
    ...custodialEntries(custody.fundsControl).map(entry => ({
      dimension: "funds_control" as const,
      entry,
    })),
    ...custodialEntries(custody.executionControl).map(entry => ({
      dimension: "execution_control" as const,
      entry,
    })),
    ...custodialEntries(custody.settlementAuthority).map(entry => ({
      dimension: "settlement_authority" as const,
      entry,
    })),
  ];
  return {
    stripClass: custodialControls.length > 0 ? "custodial" : "noncustodial",
    custodialControls,
  };
};
