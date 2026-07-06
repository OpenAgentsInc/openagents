-- MM-F1 per-user model configuration (#8484, epic #8467 mobile-only MVP).
--
-- One row per OpenAgents user: the model id they last picked in the mobile
-- Settings model picker. Mutable (a preference is a current choice, not an
-- append-only ledger) — a user changing their mind overwrites the row rather
-- than accumulating history. No prompts, payment material, or provider
-- credentials; just a bounded model-id string and a timestamp.
CREATE TABLE IF NOT EXISTS user_model_preferences (
  user_id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
