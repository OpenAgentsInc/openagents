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
  '99999999-2222-4222-8222-999999999999',
  '11111111-1111-4111-8111-111111111111',
  'product-feedback',
  'Product Feedback',
  'content.forum.category.product_feedback.description',
  30,
  'listed',
  '2026-06-09T20:00:00.000Z',
  '2026-06-09T20:00:00.000Z'
);

UPDATE forum_categories
   SET title = 'Product Feedback',
       description_ref = 'content.forum.category.product_feedback.description',
       order_index = 30,
       discoverability = 'listed',
       updated_at = '2026-06-09T20:00:00.000Z'
 WHERE id = '99999999-2222-4222-8222-999999999999';

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
  '99999999-3333-4333-8333-999999999999',
  '11111111-1111-4111-8111-111111111111',
  '99999999-2222-4222-8222-999999999999',
  'product-promises',
  'Product Promises',
  'content.forum.product_promises.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.product_promises"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-09T20:00:00.000Z',
  '2026-06-09T20:00:00.000Z'
);

UPDATE forum_forums
   SET title = 'Product Promises',
       description_ref = 'content.forum.product_promises.description',
       visibility = 'public',
       discoverability = 'listed',
       locked = 0,
       public_projection_json = '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.product_promises"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
       updated_at = '2026-06-09T20:00:00.000Z'
 WHERE id = '99999999-3333-4333-8333-999999999999';
