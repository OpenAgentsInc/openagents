# Forum parentPostId validation on create and repair on PATCH

Date: 2026-06-12
Issue: OpenAgentsInc/openagents#4856

## Repro

- `POST /api/forum/topics/{topicId}/posts` accepted any `parentPostId`
  string and stored it verbatim. A truncated ID (`95993529` instead of the
  full UUID) was persisted as a dangling thread ref.
- `PATCH /api/forum/posts/{postId}` succeeded but silently ignored a
  supplied `parentPostId`, so an author could not repair the broken ref.

Live evidence: post `87648d2c-cb75-413e-84b5-dbd44d5f8c72` in topic
`8cec9ec1-422e-4462-96dd-a74fbfd71995` carries the dangling parent ref
`95993529`.

## Fix

- Create path (`createReplyResponse` in `workers/api/src/forum-routes.ts`):
  a supplied `parentPostId` must reference an existing, visible
  (`visible`/`edited`) post in the same topic. Missing, cross-topic, and
  non-visible parents reject with typed `bad_request` 400 reasons. The
  lookup is a single D1 read (`readForumPostThreadRef` in
  `workers/api/src/forum/repository.ts`). An absent/null `parentPostId`
  keeps the existing default of `topic.latestPostId`.
- PATCH path (`editPostResponse`): `parentPostId` is now honored, not
  ignored. No immutability invariant exists for post threading in the forum
  code or `INVARIANTS.md`, so author-repairable threading was chosen over
  explicit rejection. The same validation applies, plus:
  - self-reference rejects (`parentPostId must not reference the edited
    post`), and
  - a bounded cycle guard rejects re-parenting a post under its own
    descendant (`parentPostId must not create a reply cycle`). The guard is
    one recursive-CTE D1 query (`forumPostThreadHasAncestor`, depth-capped
    at 128).
  - an explicit `parentPostId: null` re-parents the post to top level,
    which is also a valid repair for a dangling ref.
  - omitting `parentPostId` leaves threading unchanged, preserving existing
    PATCH behavior and idempotency replays.

## Data hygiene

No mass migration of existing dangling refs. The known live dangling ref on
post `87648d2c-cb75-413e-84b5-dbd44d5f8c72` is left as-is; its author can
now repair it through PATCH. The public topic projection lists posts flat by
post number and does not build a tree from `parentPostId`, so the dangling
ref renders as an ordinary post rather than breaking the projection.

## Tests

`workers/api/src/forum-routes.test.ts`:

- valid same-topic parent accepted on create and stored,
- truncated/missing, cross-topic, and tombstoned parents reject with their
  typed reasons,
- PATCH repairs a seeded dangling ref to a full valid ID, rejects
  self/missing/cycle parents, supports `null` re-parenting to top level, and
  keeps Idempotency-Key replays intact.
