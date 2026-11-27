-- Phase 4 (#14128): Allow compute = 'sandbox' alongside 'cloud' and 'local' on chat.turns_v2.
-- Preserves the strict invariant that compute = 'local' always carries cost_msat = 0.
ALTER TABLE chat.turns_v2 DROP CONSTRAINT IF EXISTS turns_v2_compute_check;
ALTER TABLE chat.turns_v2 ADD CONSTRAINT turns_v2_compute_check CHECK (compute IN ('cloud', 'local', 'sandbox'));
