/**
 * When a Full Auto control the phone offered is actually finished (omega#47).
 *
 * The contract already binds every control to an exact run generation and
 * idempotency reference, and already refuses to offer a control on a terminal
 * run. Neither of those answers the question this module exists for: the owner
 * pressed Pause, and the phone must now say whether the run is paused.
 *
 * The issue's exit is exact — "a Full Auto control completes only from an
 * Omega-owned terminal result". Three things are therefore NOT completion, and
 * this module refuses each of them by construction rather than by convention:
 *
 * - The phone drawing a paused row the moment the button was pressed. There is
 *   no optimistic state here to draw from.
 * - A relay accepting the signed intent. That is `requested`.
 * - The host accepting the command. That is `accepted` — the host has agreed to
 *   do the work, which is not the same as having done it.
 *
 * Only a host-signed owner projection that settles the command produces
 * `completed`, and the `completed` variant cannot be constructed at all without
 * the reference to that settlement. A caller cannot say "finished" without
 * naming what finished it.
 */
import { Schema as S } from "effect";

import { isIssue31PublicRef } from "@openagentsinc/sarah/issue31-workroom";

import type { Issue31OwnerCommandState } from "./issue31-owner-private-read-model.ts";
import type { Issue31FullAutoRunRow } from "./issue31-full-auto-read-model.ts";

export const ISSUE31_FULL_AUTO_CONTROL_SETTLEMENT_SCHEMA =
  "openagents.mobile.issue31.fullauto.control-settlement.v1" as const;

const PublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);

/** The control was never sent from this device. */
const Offered = S.Struct({
  state: S.Literal("offered"),
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
});

/**
 * A signed intent exists. The relay may well have taken it. That is not the
 * host having done anything.
 */
const Requested = S.Struct({
  state: S.Literal("requested"),
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  intentEventId: PublicRef,
});

/**
 * The host accepted the command and is handling it. Still not completion: the
 * owner is owed the outcome, not the acknowledgement.
 */
const Accepted = S.Struct({
  state: S.Literal("accepted"),
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  intentEventId: PublicRef,
  handlingRef: PublicRef,
});

/** The host declined, failed, or cannot say. Terminal, and not a success. */
const Rejected = S.Struct({
  state: S.Literals(["refused", "failed", "unavailable"]),
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  intentEventId: PublicRef,
  handlingRef: PublicRef,
  reasonRef: S.optional(PublicRef),
});

/**
 * The host settled the command with a signed owner projection.
 *
 * `hostSettlementRef` is required, not optional. That is the whole law: a
 * completed control that cannot name the Omega-owned result which completed it
 * is exactly the false claim omega#47 exists to prevent, and it cannot be
 * decoded, so it cannot be rendered.
 */
const Completed = S.Struct({
  state: S.Literal("completed"),
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  intentEventId: PublicRef,
  handlingRef: PublicRef,
  hostSettlementRef: PublicRef,
});

export const Issue31FullAutoControlSettlementSchema = S.Union([
  Offered,
  Requested,
  Accepted,
  Rejected,
  Completed,
]);
export type Issue31FullAutoControlSettlement = S.Schema.Type<
  typeof Issue31FullAutoControlSettlementSchema
>;

const decodeSettlement = S.decodeUnknownSync(Issue31FullAutoControlSettlementSchema);

/**
 * Decode a settlement, refusing anything the owner should not be shown as
 * finished. Use this at every boundary where a settlement arrives from outside
 * this module.
 */
export const decodeIssue31FullAutoControlSettlement = (
  value: unknown,
): Issue31FullAutoControlSettlement => {
  const settlement = decodeSettlement(value, { onExcessProperty: "error" });
  if (!isIssue31PublicRef(settlement.actionRef) || !isIssue31PublicRef(settlement.idempotencyRef)) {
    throw new Error("Issue 31 Full Auto control settlement carries an unsafe reference.");
  }
  return settlement;
};

/** True only for a settlement backed by an Omega-owned terminal result. */
export const issue31FullAutoControlIsComplete = (
  settlement: Issue31FullAutoControlSettlement,
): boolean => settlement.state === "completed";

/**
 * True when the phone should still show the control as in flight. Used to keep
 * a button disabled without ever implying the work is done.
 */
export const issue31FullAutoControlIsInFlight = (
  settlement: Issue31FullAutoControlSettlement,
): boolean => settlement.state === "requested" || settlement.state === "accepted";

/**
 * Settle one offered control against the reconciled host command ledger.
 *
 * The join is on `idempotencyRef`, which the contract already guarantees is the
 * exact reference the control was minted with. A ledger row that reuses that
 * reference for a different action is not this control's outcome — attributing
 * it here would let one command's success mark a different button finished — so
 * it fails closed to `unavailable` instead.
 */
export const settleIssue31FullAutoControl = (
  control: Issue31FullAutoRunRow["controls"][number],
  commands: ReadonlyArray<Issue31OwnerCommandState>,
): Issue31FullAutoControlSettlement => {
  const command = commands.find((row) => row.idempotencyRef === control.idempotencyRef);
  if (command === undefined) {
    return decodeIssue31FullAutoControlSettlement({
      state: "offered",
      actionRef: control.actionRef,
      idempotencyRef: control.idempotencyRef,
    });
  }

  const base = {
    actionRef: control.actionRef,
    idempotencyRef: control.idempotencyRef,
    intentEventId: command.intentEventId,
  };

  if (command.actionRef !== control.actionRef) {
    return decodeIssue31FullAutoControlSettlement({
      ...base,
      state: "unavailable",
      handlingRef: "handling.issue31.unbound",
      reasonRef: "reason.issue31.control_binding_mismatch",
    });
  }

  switch (command.state) {
    case "queued":
      return decodeIssue31FullAutoControlSettlement({ ...base, state: "requested" });
    case "accepted":
      return decodeIssue31FullAutoControlSettlement({
        ...base,
        state: "accepted",
        handlingRef: command.handlingRef,
      });
    case "refused":
    case "failed":
    case "unavailable":
      return decodeIssue31FullAutoControlSettlement({
        ...base,
        state: command.state,
        handlingRef: command.handlingRef,
        ...(command.reasonRef === null ? {} : { reasonRef: command.reasonRef }),
      });
    case "terminal":
      return decodeIssue31FullAutoControlSettlement({
        ...base,
        state: "completed",
        handlingRef: command.handlingRef,
        hostSettlementRef: command.sourceEventId,
      });
  }
};

/** Owner-facing copy for one settlement. Never claims more than the host said. */
export const issue31FullAutoControlSettlementCopy = (
  settlement: Issue31FullAutoControlSettlement,
): string => {
  switch (settlement.state) {
    case "offered":
      return "Not sent";
    case "requested":
      return "Sent · waiting for your Omega host";
    case "accepted":
      // Deliberately not "done". The host has the work, not the result.
      return "Accepted by your Omega host · not finished";
    case "refused":
      return `Refused · ${settlement.reasonRef ?? "reason.issue31.unknown"}`;
    case "failed":
      return `Failed · ${settlement.reasonRef ?? "reason.issue31.unknown"}`;
    case "unavailable":
      return `Outcome unavailable · ${settlement.reasonRef ?? "reason.issue31.unknown"}`;
    case "completed":
      return `Completed by your Omega host · ${settlement.hostSettlementRef}`;
  }
};
