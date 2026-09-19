-- Expired legacy rows are intentionally not copied into active storage, but
-- they need a terminal ledger outcome so tail sweeps do not retry them.

ALTER TABLE nostr_effect_import_ledger
    DROP CONSTRAINT nostr_effect_import_outcome;

ALTER TABLE nostr_effect_import_ledger
    ADD CONSTRAINT nostr_effect_import_outcome CHECK (
        outcome IN ('stored', 'duplicate', 'ephemeral', 'expired', 'rejected')
    );
