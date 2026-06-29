ALTER TABLE forum_tip_recipient_wallets
  ADD COLUMN bolt12_offer TEXT;

CREATE INDEX IF NOT EXISTS idx_forum_tip_recipient_wallets_bolt12_ready
  ON forum_tip_recipient_wallets(actor_ref, state, updated_at DESC)
  WHERE archived_at IS NULL
    AND bolt12_offer IS NOT NULL;
