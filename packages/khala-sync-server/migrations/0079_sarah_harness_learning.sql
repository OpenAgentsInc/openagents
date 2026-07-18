-- Sarah private terminal-history harness learning.
--
-- Raw owner conversation remains in the existing owner-private chat/runtime
-- tables. These tables retain only immutable content-addressed policies,
-- source refs/digests, bounded derived lessons, evaluation evidence, and the
-- exact bundle bound before inference. No table projects to public scopes.

CREATE TABLE IF NOT EXISTS sarah_harness_bundles (
  owner_user_id text NOT NULL,
  bundle_ref text NOT NULL,
  bundle_digest text NOT NULL,
  lifecycle text NOT NULL CHECK (lifecycle IN (
    'candidate', 'released', 'rejected', 'rolled_back'
  )),
  base_bundle_ref text,
  policy_json jsonb NOT NULL,
  lineage_json jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_by text NOT NULL,
  evaluated_by text,
  created_at text NOT NULL,
  evaluated_at text,
  released_at text,
  PRIMARY KEY (owner_user_id, bundle_ref),
  UNIQUE (owner_user_id, bundle_digest),
  CONSTRAINT sarah_harness_bundle_ref_shape CHECK (
    bundle_ref ~ '^harness\.bundle\.sarah\.[0-9a-f]{24}$'
  ),
  CONSTRAINT sarah_harness_bundle_digest_shape CHECK (
    bundle_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  CONSTRAINT sarah_harness_bundle_release_coherence CHECK (
    (lifecycle = 'released' AND evaluated_by IS NOT NULL
      AND evaluated_at IS NOT NULL AND released_at IS NOT NULL)
    OR (lifecycle <> 'released' AND released_at IS NULL)
  )
);

CREATE TABLE IF NOT EXISTS sarah_harness_active_bundles (
  owner_user_id text PRIMARY KEY,
  bundle_ref text NOT NULL,
  activation_receipt_ref text NOT NULL,
  activated_by text NOT NULL,
  activated_at text NOT NULL,
  FOREIGN KEY (owner_user_id, bundle_ref)
    REFERENCES sarah_harness_bundles(owner_user_id, bundle_ref),
  CONSTRAINT sarah_harness_activation_ref_shape CHECK (
    activation_receipt_ref ~ '^receipt\.sarah\.harness\.activation\.[0-9a-f]{24}$'
  )
);

CREATE TABLE IF NOT EXISTS sarah_harness_turn_bindings (
  owner_user_id text NOT NULL,
  thread_id text NOT NULL,
  turn_id text PRIMARY KEY,
  bundle_ref text NOT NULL,
  bundle_digest text NOT NULL,
  bound_at text NOT NULL,
  FOREIGN KEY (owner_user_id, bundle_ref)
    REFERENCES sarah_harness_bundles(owner_user_id, bundle_ref),
  CONSTRAINT sarah_harness_turn_bundle_digest_shape CHECK (
    bundle_digest ~ '^sha256:[0-9a-f]{64}$'
  )
);

CREATE INDEX IF NOT EXISTS sarah_harness_turn_bindings_owner_thread_idx
  ON sarah_harness_turn_bindings(owner_user_id, thread_id, bound_at DESC);

CREATE TABLE IF NOT EXISTS sarah_harness_experiences (
  owner_user_id text NOT NULL,
  experience_ref text NOT NULL,
  thread_id text NOT NULL,
  turn_id text NOT NULL,
  source_digest text NOT NULL,
  source_refs_json jsonb NOT NULL,
  lesson_json jsonb NOT NULL,
  visibility text NOT NULL CHECK (visibility = 'owner_private'),
  retrieval_eligible boolean NOT NULL DEFAULT true,
  training_eligible boolean NOT NULL DEFAULT false,
  terminal_at text NOT NULL,
  compiled_at text NOT NULL,
  deleted_at text,
  PRIMARY KEY (owner_user_id, experience_ref),
  UNIQUE (owner_user_id, turn_id),
  CONSTRAINT sarah_harness_experience_ref_shape CHECK (
    experience_ref ~ '^experience\.sarah\.[0-9a-f]{24}$'
  ),
  CONSTRAINT sarah_harness_experience_digest_shape CHECK (
    source_digest ~ '^sha256:[0-9a-f]{64}$'
  )
);

CREATE INDEX IF NOT EXISTS sarah_harness_experiences_owner_terminal_idx
  ON sarah_harness_experiences(owner_user_id, terminal_at DESC)
  WHERE deleted_at IS NULL AND retrieval_eligible = true;

CREATE TABLE IF NOT EXISTS sarah_harness_reviews (
  owner_user_id text NOT NULL,
  review_ref text NOT NULL,
  thread_id text NOT NULL,
  base_bundle_ref text NOT NULL,
  candidate_bundle_ref text NOT NULL,
  snapshot_digest text NOT NULL,
  training_experience_refs_json jsonb NOT NULL,
  held_out_experience_refs_json jsonb NOT NULL,
  optimizer_ref text NOT NULL,
  evaluator_ref text NOT NULL,
  release_gate_ref text NOT NULL,
  state text NOT NULL CHECK (state IN ('candidate', 'released', 'rejected')),
  evaluation_json jsonb NOT NULL,
  created_at text NOT NULL,
  evaluated_at text NOT NULL,
  released_at text,
  PRIMARY KEY (owner_user_id, review_ref),
  FOREIGN KEY (owner_user_id, base_bundle_ref)
    REFERENCES sarah_harness_bundles(owner_user_id, bundle_ref),
  FOREIGN KEY (owner_user_id, candidate_bundle_ref)
    REFERENCES sarah_harness_bundles(owner_user_id, bundle_ref),
  CONSTRAINT sarah_harness_review_ref_shape CHECK (
    review_ref ~ '^review\.sarah\.harness\.[0-9a-f]{24}$'
  ),
  CONSTRAINT sarah_harness_snapshot_digest_shape CHECK (
    snapshot_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  CONSTRAINT sarah_harness_review_release_coherence CHECK (
    (state = 'released' AND released_at IS NOT NULL)
    OR (state <> 'released' AND released_at IS NULL)
  )
);

CREATE INDEX IF NOT EXISTS sarah_harness_reviews_owner_created_idx
  ON sarah_harness_reviews(owner_user_id, created_at DESC);
