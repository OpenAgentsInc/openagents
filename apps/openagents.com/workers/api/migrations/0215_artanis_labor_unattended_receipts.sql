-- Durable tick-ledger table for consolidated, public-safe Artanis unattended
-- labor request receipts (promise artanis.labor_requester.v1, blocker
-- artanis_labor_unattended_request_receipts_missing). The receipt module builds,
-- serializes, content-addresses, parses, verifies, and seals one receipt; the
-- in-memory store proved the persistence contract; the public feed route reads a
-- store. What was still missing is a DURABLE backing behind that same store
-- interface so a receipt survives an isolate restart and the public feed can
-- serve real, persisted receipts.
--
-- One row per content-addressed receipt ref (the canonical-bytes digest), so
-- persistence is idempotent by construction: re-storing the same lifecycle is a
-- no-op (INSERT OR IGNORE on the primary key). The canonical serialized bytes
-- are the source of truth; the store re-verifies them against the ref on every
-- read (tamper-evident). The denormalized terminal_state column is for
-- query/audit only.
--
-- Public-safe by construction: the serialized receipt projects only refs that
-- already exist on the requester outcomes and carries no payment, identity, or
-- settlement authority (assertArtanisLaborPublicSafe holds the line on every
-- write and read).

CREATE TABLE IF NOT EXISTS artanis_labor_unattended_receipts (
  receipt_ref TEXT PRIMARY KEY,
  serialized_json TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_labor_unattended_receipts_created
  ON artanis_labor_unattended_receipts (created_at, receipt_ref);
