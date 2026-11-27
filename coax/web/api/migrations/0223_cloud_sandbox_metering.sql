-- Phase 5 (#14129): Metered sandbox compute duration and Auto placement provenance on chat.turns_v2.
-- Pure DDL (zero SQL policy functions; billing math and placement decisions live in #authority Coax).
ALTER TABLE chat.turns_v2
  ADD COLUMN sandbox_seconds bigint NOT NULL DEFAULT 0 CHECK (sandbox_seconds >= 0 AND (compute = 'sandbox' OR sandbox_seconds = 0)),
  ADD COLUMN placement_reason text NOT NULL DEFAULT '';
