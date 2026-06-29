import { describe, expect, test } from 'vitest'

import type { ForumPostThreadRef } from './forum'
import {
  decodeCreateForumReplyBody,
  decodeCreateForumTopicBody,
  invalidForumReplyParentPostReference,
} from './forum-topic-reply-route-contract'

const parentRef = (
  input: Partial<ForumPostThreadRef> = {},
): ForumPostThreadRef => ({
  parentPostId: null,
  postId: 'post_parent',
  state: 'visible',
  topicId: 'topic_1',
  ...input,
})

describe('forum topic/reply route contract', () => {
  test('decodes topic and reply bodies with the route trim contract', () => {
    expect(
      decodeCreateForumTopicBody({
        bodyText: '  Topic body.  ',
        context: {
          contextId: '  site_1  ',
          contextKind: 'site',
          contextSlug: null,
          contextTitle: 'Site One',
          publicUrl: 'https://openagents.com/site/site_1',
          sourceRef: 'source.public.site_1',
        },
        paymentProofRef: null,
        requestedSlug: 'hygiene-lane',
        title: '  Hygiene Lane  ',
      }),
    ).toMatchObject({
      bodyText: 'Topic body.',
      context: {
        contextId: 'site_1',
        contextKind: 'site',
      },
      requestedSlug: 'hygiene-lane',
      title: 'Hygiene Lane',
    })

    expect(
      decodeCreateForumReplyBody({
        bodyText: '  Reply body.  ',
        parentPostId: 'post_parent',
        paymentProofRef: null,
        quotePostId: null,
      }),
    ).toEqual({
      bodyText: 'Reply body.',
      parentPostId: 'post_parent',
      paymentProofRef: null,
      quotePostId: null,
    })
  })

  test('rejects empty bodies and invalid topic slugs at the boundary', () => {
    expect(() =>
      decodeCreateForumReplyBody({
        bodyText: '   ',
      }),
    ).toThrow()

    expect(() =>
      decodeCreateForumTopicBody({
        bodyText: 'Topic body.',
        requestedSlug: 'Invalid Slug',
        title: 'Topic title',
      }),
    ).toThrow()
  })

  test('keeps parent-post validation independent from route orchestration', () => {
    expect(invalidForumReplyParentPostReference(parentRef(), 'topic_1')).toBe(
      null,
    )
    expect(invalidForumReplyParentPostReference(null, 'topic_1')).toBe(
      'parentPostId must reference an existing post',
    )
    expect(
      invalidForumReplyParentPostReference(
        parentRef({ topicId: 'topic_2' }),
        'topic_1',
      ),
    ).toBe('parentPostId must belong to the target topic')
    expect(
      invalidForumReplyParentPostReference(
        parentRef({ state: 'held_for_review' }),
        'topic_1',
      ),
    ).toBe('parentPostId must reference a visible post')
  })
})
