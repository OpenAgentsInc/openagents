import type { SarahNostrMigrationStage } from "./types.ts";

/**
 * Allowed forward and rollback transitions (SARAH-NR-08).
 *
 * Forward: shadow → cutover → retirement
 * Rollback (within the window): cutover → shadow, retirement → cutover
 *
 * Same-stage transitions are idempotent no-ops (always allowed).
 */
const ALLOWED_TRANSITIONS: Readonly<
  Record<SarahNostrMigrationStage, ReadonlyArray<SarahNostrMigrationStage>>
> = {
  shadow: ["cutover"],
  cutover: ["retirement", "shadow"],
  retirement: ["cutover"],
};

export const SARAH_NOSTR_MIGRATION_STAGES: ReadonlyArray<SarahNostrMigrationStage> =
  ["shadow", "cutover", "retirement"];

export const isSarahNostrMigrationStage = (
  value: unknown,
): value is SarahNostrMigrationStage =>
  value === "shadow" || value === "cutover" || value === "retirement";

export const canTransitionSarahNostrMigrationStage = (
  from: SarahNostrMigrationStage,
  to: SarahNostrMigrationStage,
): boolean => {
  if (from === to) return true;
  return ALLOWED_TRANSITIONS[from].includes(to);
};

export class SarahNostrMigrationStageError extends Error {
  readonly from: SarahNostrMigrationStage;
  readonly to: SarahNostrMigrationStage;
  constructor(from: SarahNostrMigrationStage, to: SarahNostrMigrationStage) {
    super(
      `sarah_nostr_migration: illegal stage transition ${from} → ${to}`,
    );
    this.name = "SarahNostrMigrationStageError";
    this.from = from;
    this.to = to;
  }
}

/**
 * Pure stage machine. Holds the current stage and applies only legal
 * transitions. Idempotent for same-stage apply.
 */
export class SarahNostrMigrationStageMachine {
  private stage: SarahNostrMigrationStage;
  private readonly history: SarahNostrMigrationStage[];

  constructor(initial: SarahNostrMigrationStage = "shadow") {
    if (!isSarahNostrMigrationStage(initial)) {
      throw new Error(`sarah_nostr_migration: invalid initial stage: ${String(initial)}`);
    }
    this.stage = initial;
    this.history = [initial];
  }

  getStage(): SarahNostrMigrationStage {
    return this.stage;
  }

  getHistory(): ReadonlyArray<SarahNostrMigrationStage> {
    return [...this.history];
  }

  /** Whether Khala Sync is still the writable record authority. */
  khalaIsRecordAuthority(): boolean {
    return this.stage === "shadow";
  }

  /** Whether the relay is the writable record authority. */
  nostrIsRecordAuthority(): boolean {
    return this.stage === "cutover" || this.stage === "retirement";
  }

  /** Whether dual-publish (shadow) or Nostr-primary write should run. */
  shouldPublishToNostr(): boolean {
    return true; // all three stages publish to Nostr
  }

  /** Whether the Khala Sync write path for the Sarah lane stays open. */
  shouldWriteKhala(): boolean {
    return this.stage === "shadow" || this.stage === "cutover";
  }

  /**
   * Apply a transition. Same-stage is a no-op (idempotent). Illegal transitions
   * throw SarahNostrMigrationStageError.
   */
  transition(to: SarahNostrMigrationStage): SarahNostrMigrationStage {
    if (!isSarahNostrMigrationStage(to)) {
      throw new Error(`sarah_nostr_migration: invalid target stage: ${String(to)}`);
    }
    if (this.stage === to) {
      return this.stage;
    }
    if (!canTransitionSarahNostrMigrationStage(this.stage, to)) {
      throw new SarahNostrMigrationStageError(this.stage, to);
    }
    this.stage = to;
    this.history.push(to);
    return this.stage;
  }

  /** Convenience: shadow → cutover. */
  cutover(): SarahNostrMigrationStage {
    return this.transition("cutover");
  }

  /** Convenience: cutover → retirement. */
  retire(): SarahNostrMigrationStage {
    return this.transition("retirement");
  }

  /**
   * Roll back one step when legal:
   * retirement → cutover, cutover → shadow. No-op at shadow.
   */
  rollback(): SarahNostrMigrationStage {
    if (this.stage === "retirement") {
      return this.transition("cutover");
    }
    if (this.stage === "cutover") {
      return this.transition("shadow");
    }
    return this.stage;
  }
}

/**
 * Map a record mode flag onto the stage that owns authority for that mode.
 * Mode `khala` is pre-migration (not a stage). Mode `shadow` is stage shadow.
 * Mode `nostr` defaults to stage cutover (relay is record; Khala still writable
 * as projection until retirement).
 */
export const defaultStageForRecordMode = (
  mode: "khala" | "shadow" | "nostr",
): SarahNostrMigrationStage | null => {
  if (mode === "khala") return null;
  if (mode === "shadow") return "shadow";
  return "cutover";
};
