/**
 * Typed failure vocabulary for the local swap session store (openagents#9320,
 * SWAP-5). Every refusal is a tagged error the surface can match on; nothing
 * here is prose from a counterparty, and nothing here ever carries secret
 * material (see `secret-boundary.ts`).
 */
import { Schema } from "effect";

/** The backing key-value driver failed an operation. */
export class StorageDriverError extends Schema.TaggedErrorClass<StorageDriverError>()(
  "StorageDriverError",
  {
    operation: Schema.Literals(["get", "set", "delete", "keys"]),
    key: Schema.String,
    detail: Schema.String,
  },
) {}

/**
 * A persisted record failed its integrity digest or could not be parsed —
 * a torn (partially written) record. A torn record is never surfaced as a
 * loadable session; the journal either rolls forward to a complete committed
 * version or fails with this error. This is the crash-mid-write refusal the
 * issue requires.
 */
export class TornSessionRecordError extends Schema.TaggedErrorClass<TornSessionRecordError>()(
  "TornSessionRecordError",
  {
    key: Schema.String,
    reason: Schema.Literals(["unparseable", "digest_mismatch", "envelope_invalid"]),
  },
) {}

/**
 * A persisted record (or an import document) declares a schema version newer
 * than this build understands. The store refuses to open/ingest rather than
 * silently reinterpreting a future format. Downgrade paths do not exist by
 * design; the user upgrades the app or exports from the newer build.
 */
export class UnsupportedSchemaVersionError extends Schema.TaggedErrorClass<UnsupportedSchemaVersionError>()(
  "UnsupportedSchemaVersionError",
  {
    key: Schema.String,
    found: Schema.Number,
    supported: Schema.Number,
  },
) {}

/** A sequential migration step is missing between two schema versions. */
export class MigrationStepMissingError extends Schema.TaggedErrorClass<MigrationStepMissingError>()(
  "MigrationStepMissingError",
  {
    from: Schema.Number,
    to: Schema.Number,
  },
) {}

/** A migrated or written payload failed the session schema. */
export class SessionRecordInvalidError extends Schema.TaggedErrorClass<SessionRecordInvalidError>()(
  "SessionRecordInvalidError",
  {
    key: Schema.String,
    detail: Schema.String,
  },
) {}

export class SessionNotFoundError extends Schema.TaggedErrorClass<SessionNotFoundError>()(
  "SessionNotFoundError",
  {
    sessionId: Schema.String,
  },
) {}

export class SessionAlreadyExistsError extends Schema.TaggedErrorClass<SessionAlreadyExistsError>()(
  "SessionAlreadyExistsError",
  {
    sessionId: Schema.String,
  },
) {}

/**
 * The custody tripwire (MKT-SWP §12 / §17 `swp_secret_material_forbidden`):
 * a payload offered for persistence or export appears to carry key material,
 * a preimage, or another secret. The store persists public records and
 * non-secret handles only; secrets belong to the SWAP-4 secret store
 * (openagents#9319). The offending JSON path is reported; the value never is.
 */
export class SecretMaterialError extends Schema.TaggedErrorClass<SecretMaterialError>()(
  "SecretMaterialError",
  {
    /** JSON path of the offending member, e.g. "sessions[0].engineSnapshot.mnemonic". */
    path: Schema.String,
    /** Stable MKT-SWP identifier for surfaces: always `swp_secret_material_forbidden`. */
    identifier: Schema.Literal("swp_secret_material_forbidden"),
  },
) {}

/**
 * Idempotency fail-closed (MKT §"Idempotency", client doc "External wallet,
 * payment, and broadcast operations use deterministic effect IDs"): binding
 * one effect ID to a different request or result digest is refused.
 */
export class EffectBindingConflictError extends Schema.TaggedErrorClass<EffectBindingConflictError>()(
  "EffectBindingConflictError",
  {
    sessionId: Schema.String,
    effectId: Schema.String,
    reason: Schema.Literals(["request_digest_mismatch", "result_digest_mismatch", "result_without_request"]),
  },
) {}

/** A signed record changed bytes at an already-persisted event id. */
export class SignedRecordConflictError extends Schema.TaggedErrorClass<SignedRecordConflictError>()(
  "SignedRecordConflictError",
  {
    sessionId: Schema.String,
    eventId: Schema.String,
  },
) {}

export const HISTORY_IMPORT_REFUSAL_REASONS = [
  /** Not shaped like a swap-history export at all (wrong format marker). */
  "not_an_export",
  /** Export was produced by a newer schema than this build understands. */
  "unsupported_version",
  /** A session inside the export failed structural validation. */
  "session_invalid",
  /** A session's content digest does not match its bytes (corrupt/foreign). */
  "digest_mismatch",
  /** The export carries secret-looking material; see SecretMaterialError. */
  "secret_material",
  /** A session id already exists locally with different content. */
  "conflicting_session",
] as const;

export type HistoryImportRefusalReason = (typeof HISTORY_IMPORT_REFUSAL_REASONS)[number];

/**
 * Import is all-or-nothing: any refusal reason means zero sessions were
 * written. `detail` names the offending session id or member for the surface;
 * it never echoes foreign document content verbatim.
 */
export class HistoryImportError extends Schema.TaggedErrorClass<HistoryImportError>()(
  "HistoryImportError",
  {
    reason: Schema.Literals([...HISTORY_IMPORT_REFUSAL_REASONS]),
    detail: Schema.String,
  },
) {}
