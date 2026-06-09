ALTER TABLE forum_categories
  ADD COLUMN discoverability TEXT NOT NULL DEFAULT 'listed' CHECK (
    discoverability IN ('listed', 'unlisted', 'hidden')
  );

ALTER TABLE forum_forums
  ADD COLUMN discoverability TEXT NOT NULL DEFAULT 'listed' CHECK (
    discoverability IN ('listed', 'unlisted', 'hidden')
  );

INSERT OR IGNORE INTO forum_boards (
  id,
  slug,
  title,
  description_ref,
  visibility,
  public_projection_json,
  created_at,
  updated_at
)
VALUES (
  '11111111-1111-4111-8111-111111111111',
  'openagents',
  'OpenAgents',
  'content.forum.board.openagents.description',
  'public',
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.board.openagents"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
);

UPDATE forum_boards
   SET title = 'OpenAgents',
       description_ref = 'content.forum.board.openagents.description',
       visibility = 'public',
       public_projection_json = '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.board.openagents"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
       updated_at = '2026-06-05T00:00:00.000Z'
 WHERE id = '11111111-1111-4111-8111-111111111111';

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
  '22222222-2222-4222-8222-222222222222',
  '11111111-1111-4111-8111-111111111111',
  'sites',
  'Sites',
  'content.forum.category.sites.description',
  10,
  'listed',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
);

UPDATE forum_categories
   SET title = 'Sites',
       description_ref = 'content.forum.category.sites.description',
       order_index = 10,
       discoverability = 'listed',
       updated_at = '2026-06-05T00:00:00.000Z'
 WHERE id = '22222222-2222-4222-8222-222222222222';

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
  '33333333-3333-4333-8333-333333333333',
  '11111111-1111-4111-8111-111111111111',
  '22222222-2222-4222-8222-222222222222',
  'site-builder-help',
  'Site Builder Help',
  'content.forum.site_builder_help.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.site_builder_help"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
);

UPDATE forum_forums
   SET title = 'Site Builder Help',
       description_ref = 'content.forum.site_builder_help.description',
       visibility = 'public',
       discoverability = 'listed',
       public_projection_json = '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.site_builder_help"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
       updated_at = '2026-06-05T00:00:00.000Z'
 WHERE id = '33333333-3333-4333-8333-333333333333';

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
  '44444444-1111-4111-8111-444444444444',
  '11111111-1111-4111-8111-111111111111',
  'void',
  'Void',
  'content.forum.category.void.description',
  900,
  'unlisted',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
);

UPDATE forum_categories
   SET title = 'Void',
       description_ref = 'content.forum.category.void.description',
       order_index = 900,
       discoverability = 'unlisted',
       updated_at = '2026-06-05T00:00:00.000Z'
 WHERE id = '44444444-1111-4111-8111-444444444444';

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
  '55555555-1111-4111-8111-555555555555',
  '11111111-1111-4111-8111-111111111111',
  '44444444-1111-4111-8111-444444444444',
  'void',
  'Void',
  'content.forum.void.description',
  'public',
  'unlisted',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.void"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
);

UPDATE forum_forums
   SET title = 'Void',
       description_ref = 'content.forum.void.description',
       visibility = 'public',
       discoverability = 'unlisted',
       public_projection_json = '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.void"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
       updated_at = '2026-06-05T00:00:00.000Z'
 WHERE id = '55555555-1111-4111-8111-555555555555';
