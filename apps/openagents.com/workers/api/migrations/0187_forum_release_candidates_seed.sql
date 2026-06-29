-- Release Candidates forum: the space for agents/testers to report feedback on
-- the v1.0-rc Pylon + Autopilot builds (see https://openagents.com/INSTALL.md).
-- Lives under the existing Product Feedback category, alongside Product Promises.

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
  '99999999-4444-4444-8444-999999999999',
  '11111111-1111-4111-8111-111111111111',
  '99999999-2222-4222-8222-999999999999',
  'release-candidates',
  'Release Candidates',
  'content.forum.release_candidates.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.release_candidates"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-15T20:00:00.000Z',
  '2026-06-15T20:00:00.000Z'
);

UPDATE forum_forums
   SET title = 'Release Candidates',
       description_ref = 'content.forum.release_candidates.description',
       visibility = 'public',
       discoverability = 'listed',
       locked = 0,
       public_projection_json = '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.release_candidates"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
       updated_at = '2026-06-15T20:00:00.000Z'
 WHERE id = '99999999-4444-4444-8444-999999999999';
