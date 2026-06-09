ALTER TABLE omni_workrooms
  ADD COLUMN data_classification TEXT NOT NULL DEFAULT 'customer';

ALTER TABLE omni_workrooms
  ADD COLUMN trust_tier TEXT NOT NULL DEFAULT 'unverified';

ALTER TABLE omni_workrooms
  ADD COLUMN classification_caveat_ref TEXT NOT NULL DEFAULT 'classification_caveat_unreviewed';

CREATE INDEX IF NOT EXISTS idx_omni_workrooms_classification
  ON omni_workrooms(data_classification, trust_tier, archived_at);
