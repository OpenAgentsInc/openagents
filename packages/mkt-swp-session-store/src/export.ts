/**
 * Export and import — both, in this issue (#9320 scope item 5).
 *
 * The teardown's sharpest finding (§4.4): Boltz's history export contains
 * exactly the dataset that would make a user coordinator-independent, and
 * has no import path — the one artifact that would free them is write-only.
 * If we emit an export, we ingest it.
 *
 * SENSITIVITY — read before wiring to UI. The export contains NO keys,
 * preimages, or other spend authority (the custody tripwire runs on every
 * session before it is emitted), so possession of the file does not spend
 * funds. It DOES contain the user's complete swap history: amounts,
 * invoices, addresses, counterparties, timestamps. That is private financial
 * data and the API names say so: `exportPrivateHistory` /
 * `importPrivateHistory`. Surfaces must present the
 * `swap.history.export.sensitivity` notice alongside the download. The
 * secret material a user must ALSO back up to sign fresh exits lives in the
 * SWAP-4 secret store (openagents#9319) and is exported through SWAP-4's
 * rescue ceremony, never through this document.
 *
 * Import is a validation gate, not a merge heuristic: the document is
 * decoded structurally, every session's content digest is re-verified, the
 * custody tripwire runs, versions newer than this build are refused, and a
 * session id that already exists locally with DIFFERENT content refuses the
 * whole document. All-or-nothing: a validation refusal means zero writes,
 * and a driver failure mid-apply rolls back this import's writes before
 * refusing (`storage_failure`).
 */
import { Effect } from "effect";

import type { MessageKey } from "@openagentsinc/swap-i18n";

import { contentDigestHex } from "./canonical.js";
import { HistoryImportError, type HistoryImportRefusalReason, type SecretMaterialError } from "./errors.js";
import {
  CURRENT_SCHEMA_VERSION,
  SESSION_STORE_MIGRATIONS,
  migrateEnvelopePayload,
  type SessionSchemaMigration,
} from "./migrate.js";
import { decodeStoredSwapSession, type StoredSwapSession } from "./model.js";
import { assertNoSecretMaterial } from "./secret-boundary.js";
import type { SessionStore } from "./store.js";

export const SWAP_HISTORY_EXPORT_FORMAT = "openagents.swap-history-export";

export interface ExportedSession {
  readonly schemaVersion: number;
  /** SHA-256 (canonical JSON) of `session`; verified before import applies. */
  readonly contentDigestHex: string;
  readonly session: StoredSwapSession;
}

/**
 * The private history document. Everything needed to resume or unilaterally
 * exit every session — signed records, exit packages, effect ledger, engine
 * snapshot — travels inside; secrets never do (see module doc).
 */
export interface SwapHistoryExport {
  readonly format: typeof SWAP_HISTORY_EXPORT_FORMAT;
  readonly schemaVersion: number;
  readonly exportedAt: number;
  readonly sessions: ReadonlyArray<ExportedSession>;
}

export const exportPrivateHistory = (
  store: SessionStore,
  now: () => number = () => Date.now(),
): Effect.Effect<SwapHistoryExport, SecretMaterialError> =>
  Effect.gen(function* () {
    const sessions = yield* store.list();
    // The tripwire runs ON THE EXPORT PATH, per session, not only
    // transitively at persist time: a document that would carry key
    // material is never emitted, whatever wrote the store.
    for (const session of sessions) {
      yield* assertNoSecretMaterial(session, `export(${session.sessionId})`);
    }
    return {
      format: SWAP_HISTORY_EXPORT_FORMAT,
      schemaVersion: CURRENT_SCHEMA_VERSION,
      exportedAt: now(),
      sessions: sessions.map((session) => ({
        schemaVersion: CURRENT_SCHEMA_VERSION,
        contentDigestHex: contentDigestHex(session),
        session,
      })),
    };
  });

export interface ImportOutcome {
  /** Session ids newly written or overwritten-with-identical-content. */
  readonly imported: ReadonlyArray<string>;
  /** Session ids skipped because identical content already exists. */
  readonly identical: ReadonlyArray<string>;
}

interface ValidatedImport {
  readonly session: StoredSwapSession;
  readonly identical: boolean;
}

/**
 * Ingest a history document produced by `exportPrivateHistory` — possibly by
 * an older build (sequential migrations run per session), possibly on
 * another device. Refusals are typed (`HistoryImportError`) and
 * all-or-nothing: nothing is written unless every session validates.
 */
export const importPrivateHistory = (
  store: SessionStore,
  document: unknown,
  // The default MUST be the shipped chain, exactly as store open defaults
  // to it: a default-argument import of an older build's export is the
  // promised case, not an error.
  migrations: ReadonlyArray<SessionSchemaMigration> = SESSION_STORE_MIGRATIONS,
  currentVersion: number = CURRENT_SCHEMA_VERSION,
): Effect.Effect<ImportOutcome, HistoryImportError> =>
  Effect.gen(function* () {
    if (typeof document !== "object" || document === null || Array.isArray(document)) {
      return yield* new HistoryImportError({ reason: "not_an_export", detail: "not an object" });
    }
    const candidate = document as Record<string, unknown>;
    if (candidate.format !== SWAP_HISTORY_EXPORT_FORMAT) {
      return yield* new HistoryImportError({
        reason: "not_an_export",
        detail: "format marker missing or unrecognised",
      });
    }
    if (typeof candidate.schemaVersion !== "number" || !Array.isArray(candidate.sessions)) {
      return yield* new HistoryImportError({
        reason: "not_an_export",
        detail: "schemaVersion or sessions member malformed",
      });
    }
    if (candidate.schemaVersion > currentVersion) {
      return yield* new HistoryImportError({
        reason: "unsupported_version",
        detail: `document version ${candidate.schemaVersion} > supported ${currentVersion}`,
      });
    }

    const existing = new Map(
      (yield* store.list()).map((session) => [session.sessionId, contentDigestHex(session)]),
    );

    // Phase 1 — validate EVERYTHING before writing anything.
    const validated: ValidatedImport[] = [];
    for (let index = 0; index < candidate.sessions.length; index += 1) {
      const entry: unknown = candidate.sessions[index];
      if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
        return yield* new HistoryImportError({
          reason: "session_invalid",
          detail: `sessions[${index}] is not an object`,
        });
      }
      const { schemaVersion, contentDigestHex: declaredDigest, session } = entry as Record<string, unknown>;
      if (typeof schemaVersion !== "number" || typeof declaredDigest !== "string") {
        return yield* new HistoryImportError({
          reason: "session_invalid",
          detail: `sessions[${index}] envelope malformed`,
        });
      }
      const actualDigest = yield* Effect.try({
        try: () => contentDigestHex(session),
        catch: () =>
          new HistoryImportError({
            reason: "session_invalid",
            detail: `sessions[${index}] is not canonicalisable JSON`,
          }),
      });
      if (actualDigest !== declaredDigest) {
        return yield* new HistoryImportError({
          reason: "digest_mismatch",
          detail: `sessions[${index}] content does not match its digest`,
        });
      }
      const migrated = yield* migrateEnvelopePayload(
        `import[${index}]`,
        { schemaVersion, writeSeq: 0, payload: session, digestHex: actualDigest },
        migrations,
        currentVersion,
      ).pipe(
        Effect.mapError(
          (error) =>
            new HistoryImportError({
              reason: error._tag === "UnsupportedSchemaVersionError" ? "unsupported_version" : "session_invalid",
              detail: `sessions[${index}]: ${error._tag}`,
            }),
        ),
      );
      const decoded = yield* Effect.try({
        try: () => decodeStoredSwapSession(migrated.payload),
        catch: (error) =>
          new HistoryImportError({
            reason: "session_invalid",
            detail: `sessions[${index}] failed the session schema: ${
              error instanceof Error ? error.message : String(error)
            }`,
          }),
      });
      yield* assertNoSecretMaterial(decoded, `sessions[${index}]`).pipe(
        Effect.mapError(
          (error) => new HistoryImportError({ reason: "secret_material", detail: error.path }),
        ),
      );
      const existingDigest = existing.get(decoded.sessionId);
      if (existingDigest !== undefined && existingDigest !== contentDigestHex(decoded)) {
        return yield* new HistoryImportError({
          reason: "conflicting_session",
          detail: `session ${decoded.sessionId} already exists with different content`,
        });
      }
      validated.push({ session: decoded, identical: existingDigest !== undefined });
    }

    // Phase 2 — apply. Every applied session is strictly NEW (identical
    // ones are skipped, conflicting ones were refused), so a mid-apply
    // driver failure (quota exhaustion is the realistic case) rolls back by
    // deleting exactly the sessions this import wrote, restoring the
    // profile as it was, and refuses typed (`storage_failure`) — the
    // all-or-nothing clause holds for the apply phase, not only for
    // validation refusals.
    const imported: string[] = [];
    const identical: string[] = [];
    for (const entry of validated) {
      if (entry.identical) {
        identical.push(entry.session.sessionId);
        continue;
      }
      const write = yield* store.putValidated(entry.session).pipe(Effect.exit);
      if (write._tag === "Failure") {
        const rollbackFailures: string[] = [];
        for (const sessionId of imported) {
          const rollback = yield* store.delete(sessionId).pipe(Effect.exit);
          if (rollback._tag === "Failure") rollbackFailures.push(sessionId);
        }
        return yield* new HistoryImportError({
          reason: "storage_failure",
          detail:
            rollbackFailures.length === 0
              ? `storage failed writing session ${entry.session.sessionId}; ${imported.length} prior write(s) rolled back`
              : `storage failed writing session ${entry.session.sessionId}; rollback ALSO failed for: ${rollbackFailures.join(", ")}`,
        });
      }
      imported.push(entry.session.sessionId);
    }
    return { imported, identical };
  });

/**
 * The `@openagentsinc/swap-i18n` notice surfaces MUST present alongside the
 * export download (see the module doc's SENSITIVITY paragraph).
 */
export const EXPORT_SENSITIVITY_KEY: MessageKey = "swap.history.export.sensitivity";

const IMPORT_REFUSAL_KEYS: Readonly<Record<HistoryImportRefusalReason, MessageKey>> = {
  not_an_export: "swap.history.import.refused.not_an_export",
  unsupported_version: "swap.history.import.refused.unsupported_version",
  session_invalid: "swap.history.import.refused.session_invalid",
  digest_mismatch: "swap.history.import.refused.digest_mismatch",
  secret_material: "swap.history.import.refused.secret_material",
  conflicting_session: "swap.history.import.refused.conflicting_session",
  storage_failure: "swap.history.import.refused.storage_failure",
};

/**
 * The typed message key for one import refusal — the producer that binds
 * every `HistoryImportError.reason` to its catalog copy, so a removed or
 * renamed refusal message fails typecheck here instead of rendering blank.
 */
export const importRefusalKeyOf = (error: HistoryImportError): MessageKey =>
  IMPORT_REFUSAL_KEYS[error.reason];
