-- Seed the Research category with dedicated Tassadar and Psionic forums.
-- Tassadar: the executor-compiler / internal-computation research lane.
-- Psionic: the ML substrate, CS336 reference lanes, and training programs.
-- Copy stays bounded: research-lane discussion surfaces, no capability claims.

INSERT OR IGNORE INTO forum_categories (
  id,
  board_id,
  slug,
  title,
  description_ref,
  order_index,
  discoverability,
  created_at,
  updated_at
)
VALUES (
  '99999999-4444-4444-8444-999999999999',
  '11111111-1111-4111-8111-111111111111',
  'research',
  'Research',
  'content.forum.category.research.description',
  40,
  'listed',
  '2026-06-11T00:00:00.000Z',
  '2026-06-11T00:00:00.000Z'
);

INSERT OR IGNORE INTO forum_forums (
  id,
  board_id,
  category_id,
  slug,
  title,
  description_ref,
  visibility,
  discoverability,
  locked,
  topic_count,
  post_count,
  public_projection_json,
  created_at,
  updated_at
)
VALUES (
  '99999999-5555-4555-8555-999999999999',
  '11111111-1111-4111-8111-111111111111',
  '99999999-4444-4444-8444-999999999999',
  'tassadar',
  'Tassadar',
  'content.forum.tassadar.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.tassadar"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-11T00:00:00.000Z',
  '2026-06-11T00:00:00.000Z'
);

INSERT OR IGNORE INTO forum_forums (
  id,
  board_id,
  category_id,
  slug,
  title,
  description_ref,
  visibility,
  discoverability,
  locked,
  topic_count,
  post_count,
  public_projection_json,
  created_at,
  updated_at
)
VALUES (
  '99999999-6666-4666-8666-999999999999',
  '11111111-1111-4111-8111-111111111111',
  '99999999-4444-4444-8444-999999999999',
  'psionic',
  'Psionic',
  'content.forum.psionic.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.psionic"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-11T00:00:00.000Z',
  '2026-06-11T00:00:00.000Z'
);
