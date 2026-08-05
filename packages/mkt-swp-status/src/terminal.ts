/**
 * Terminal outcomes and terminality (issue #9321 scopes 6-7).
 *
 * Terminality has ONE definition — `isWatchTerminal` — exported from this
 * session projection. Boltz ships two divergent definitions in one
 * repository (its app keeps refundable failures watched, its SDK helper
 * treats all eight failure statuses as terminal, so migrating code silently
 * stops watching refundable swaps). Everything in this repo must consult
 * this function and nothing else.
 *
 * Every terminal state has a stated user exit, and `unresolved` is
 * displayed as unresolved — never as failed, never as complete.
 */
import type {
  CloseOutcome,
  CloseRecord,
  LossAccounting,
  UserExitKind,
} from "./model.js";

export const CLOSE_OUTCOMES: readonly CloseOutcome[] = [
  "completed",
  "rejected",
  "cancelled",
  "expired",
  "failed",
  "refunded",
  "disputed",
  "unresolved",
];

export interface TerminalDescriptor {
  readonly outcome: CloseOutcome;
  readonly displayKey: `swap.status.terminal.${CloseOutcome}`;
  readonly exit: UserExitKind;
  /** Whether the session may stop being watched once this outcome is verified. */
  readonly watchTerminal: boolean;
}

const TERMINAL_TABLE: Readonly<Record<CloseOutcome, TerminalDescriptor>> = {
  completed: {
    outcome: "completed",
    displayKey: "swap.status.terminal.completed",
    exit: "none_needed",
    watchTerminal: true,
  },
  rejected: {
    outcome: "rejected",
    displayKey: "swap.status.terminal.rejected",
    exit: "none_needed",
    watchTerminal: true,
  },
  cancelled: {
    outcome: "cancelled",
    displayKey: "swap.status.terminal.cancelled",
    exit: "none_needed",
    watchTerminal: true,
  },
  expired: {
    outcome: "expired",
    displayKey: "swap.status.terminal.expired",
    exit: "none_needed",
    watchTerminal: true,
  },
  failed: {
    outcome: "failed",
    displayKey: "swap.status.terminal.failed",
    exit: "rescue",
    watchTerminal: false, // true only with fully accounted principal; see isWatchTerminal
  },
  refunded: {
    outcome: "refunded",
    displayKey: "swap.status.terminal.refunded",
    exit: "none_needed",
    watchTerminal: true,
  },
  disputed: {
    outcome: "disputed",
    displayKey: "swap.status.terminal.disputed",
    exit: "dispute",
    watchTerminal: false,
  },
  unresolved: {
    outcome: "unresolved",
    displayKey: "swap.status.terminal.unresolved",
    exit: "keep_watching",
    watchTerminal: false,
  },
};

export function terminalDescriptor(outcome: CloseOutcome): TerminalDescriptor {
  return TERMINAL_TABLE[outcome];
}

const isFullyAccounted = (close: CloseRecord): boolean => {
  if (close.lossAccounting === undefined) return false;
  const unknown = close.unknownFields ?? [];
  if (unknown.length > 0) return false;
  return close.lossAccounting.principal_unresolved === "0";
};

/**
 * THE definition of terminality for watching purposes. `failed` is watch-
 * terminal only when principal is fully accounted with nothing unresolved;
 * `disputed` and `unresolved` are never watch-terminal.
 */
export function isWatchTerminal(close: CloseRecord): boolean {
  const descriptor = TERMINAL_TABLE[close.outcome];
  if (close.outcome === "failed") return isFullyAccounted(close);
  return descriptor.watchTerminal;
}

export type LossField = keyof LossAccounting;

export const LOSS_AMOUNT_FIELDS: readonly LossField[] = [
  "input_committed",
  "input_recovered",
  "output_received",
  "provider_fee_paid",
  "miner_fee_paid",
  "lightning_routing_fee_paid",
  "guarantee_recovery_received",
  "principal_unresolved",
  "reservation_released",
];

/** Fee fields are rendered as their own rows — never collapsed into principal. */
export const LOSS_FEE_FIELDS: readonly LossField[] = [
  "provider_fee_paid",
  "miner_fee_paid",
  "lightning_routing_fee_paid",
];

export interface LossFieldView {
  readonly field: LossField;
  /** "unknown" for members of unknown_fields; NEVER "0" for an unknown value. */
  readonly value: string | "unknown";
  readonly isFee: boolean;
}

export interface LossAccountingView {
  readonly fields: readonly LossFieldView[];
  readonly unknownFields: readonly LossField[];
  readonly complete: boolean;
}

export function lossAccountingView(
  loss: LossAccounting,
  unknownFields: readonly LossField[] = [],
): LossAccountingView {
  const fields = LOSS_AMOUNT_FIELDS.map(
    (field): LossFieldView => ({
      field,
      value: unknownFields.includes(field) ? "unknown" : loss[field],
      isFee: LOSS_FEE_FIELDS.includes(field),
    }),
  );
  return { fields, unknownFields, complete: unknownFields.length === 0 };
}

export interface CloseView {
  readonly close: CloseRecord;
  readonly descriptor: TerminalDescriptor;
  readonly loss: LossAccountingView | null;
  readonly watchTerminal: boolean;
}

export interface ClosesView {
  /** Every Close, grouped nowhere — conflicting Closes both remain visible. */
  readonly closes: readonly CloseView[];
  /** True when the two parties' outcomes differ. */
  readonly conflict: boolean;
  /**
   * Watch-terminal only when every party's Close is watch-terminal and there
   * is no conflict. One party's `completed` does not force the other's
   * outcome, and it does not stop the watch on its own.
   */
  readonly watchTerminal: boolean;
}

export function closesView(closes: readonly CloseRecord[]): ClosesView {
  const views = closes
    .slice()
    .sort((left, right) => (left.id < right.id ? -1 : left.id > right.id ? 1 : 0))
    .map(
      (close): CloseView => ({
        close,
        descriptor: TERMINAL_TABLE[close.outcome],
        loss: close.lossAccounting
          ? lossAccountingView(close.lossAccounting, close.unknownFields ?? [])
          : null,
        watchTerminal: isWatchTerminal(close),
      }),
    );
  const outcomes = new Set(views.map((view) => view.close.outcome));
  const conflict = outcomes.size > 1;
  return {
    closes: views,
    conflict,
    watchTerminal: views.length > 0 && !conflict && views.every((view) => view.watchTerminal),
  };
}
