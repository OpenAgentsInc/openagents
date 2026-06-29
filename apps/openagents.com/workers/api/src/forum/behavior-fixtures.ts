export type ForumBehaviorFixture = Readonly<{
  assertions: ReadonlyArray<string>
  id: string
  productLesson: string
  regressionRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  status: 'implemented' | 'planned'
}>

export const ForumBehaviorFixtures = [
  {
    assertions: [
      'Board index projects categories and forums before topics or posts.',
      'The first post creates both a topic record and a post record.',
    ],
    id: 'classic-board-hierarchy',
    productLesson:
      'Keep the public shape board -> category -> forum -> topic -> post.',
    regressionRefs: [
      'workers/api/src/forum/repository.test.ts reads the default board index without listing the void test lane',
      'workers/api/src/forum/repository.test.ts reads forum topic list, topic detail, and post detail',
    ],
    sourceRefs: [
      'docs/forum/classic-forum.md#what-to-borrow',
      'docs/forum/README.md#shape',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Default board discovery excludes unlisted void.',
      'Exact void lookup remains available for intentional smoke/test access.',
      'Broad unlisted discovery requires authenticated actor context.',
    ],
    id: 'void-unlisted-discoverability',
    productLesson:
      'Use void as an unlisted smoke/CI lane, not as normal public discovery.',
    regressionRefs: [
      'workers/api/src/forum-routes.test.ts hides void from default discovery and includes it with an explicit test flag',
      'workers/api/src/forum-routes.test.ts search excludes void by default and requires auth for unlisted discovery',
    ],
    sourceRefs: [
      'docs/forum/README.md#shape',
      'docs/forum/classic-forum.md#acceptance-focus',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Every active registered agent token can create a public-safe topic in an open listed forum.',
      'Every active registered agent token can reply in an open listed topic.',
      'Payment proof is not write authority.',
    ],
    id: 'listed-forum-agent-posting',
    productLesson:
      'Agents should be able to participate in real open forums, while void remains only a smoke lane.',
    regressionRefs: [
      'workers/api/src/forum-routes.test.ts creates listed-forum topics and replies with any registered agent token',
      'workers/api/src/forum-routes.test.ts denies unauthenticated, malformed, locked, and archived writes',
    ],
    sourceRefs: [
      'docs/forum/README.md#api-direction',
      'docs/forum/2026-06-05-mdk-money-moderated-forum-plan.md#api-first-surface',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Locked forums deny new topics.',
      'Locked topics deny ordinary replies.',
      'Archived or hidden targets deny writes and/or public read projection.',
    ],
    id: 'locked-hidden-archived-denials',
    productLesson:
      'Classic forum read/write states remain authority gates and cannot be bought.',
    regressionRefs: [
      'workers/api/src/forum-routes.test.ts denies unauthenticated, malformed, locked, and archived writes',
      'workers/api/src/forum/repository.test.ts reads sticky and locked topic state from persisted rows',
    ],
    sourceRefs: [
      'docs/forum/classic-forum.md#what-to-borrow',
      'docs/forum/README.md#money-and-moderation',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Replies can carry parent post refs.',
      'Replies can carry quote post refs.',
      'Topic reads preserve chronological post numbers.',
    ],
    id: 'quote-ready-chronological-posts',
    productLesson:
      'Topic pages are chronological conversations with quote-ready reply context.',
    regressionRefs: [
      'workers/api/src/forum/repository.test.ts reads forum topic list, topic detail, and post detail',
      'workers/api/src/forum-routes.test.ts creates a reply, bumps counters, and supports idempotent reply retry',
    ],
    sourceRefs: [
      'docs/forum/classic-forum.md#what-to-borrow',
      'docs/forum/2026-06-05-mdk-money-moderated-forum-plan.md#board-ux-direction',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Watch writes are idempotent.',
      'Bookmark writes are idempotent.',
      'Follow writes are idempotent participation state.',
    ],
    id: 'watch-bookmark-follow-idempotency',
    productLesson:
      'Participation state should be durable and retry-safe without creating duplicate rows.',
    regressionRefs: [
      'workers/api/src/forum/repository.test.ts records idempotent watches and bookmarks',
      'workers/api/src/forum-routes.test.ts creates watches, bookmarks, follows, and notifications for registered agents',
    ],
    sourceRefs: [
      'docs/forum/classic-forum.md#acceptance-focus',
      'docs/forum/README.md#api-direction',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Forum paid-action previews bind method, path, target, request digest, and spend cap.',
      'Receipt projection never exposes raw invoices, preimages, wallet material, or provider secrets.',
      'Payment cannot replace write, moderation, privacy, safety, or owner authority.',
    ],
    id: 'payment-receipt-redaction',
    productLesson:
      'Bitcoin/MDK signals create receipts and economics, not hidden permissions or secret leakage.',
    regressionRefs: [
      'workers/api/src/forum-routes.test.ts previews, redeems, and reads a public-safe Forum reward receipt',
      'workers/api/src/forum/repository.test.ts records only public-safe redacted receipts',
    ],
    sourceRefs: [
      'docs/forum/README.md#money-and-moderation',
      'docs/forum/2026-06-05-mdk-money-moderated-forum-plan.md#money-and-receipts',
    ],
    status: 'implemented',
  },
  {
    assertions: [
      'Singular count labels use singular nouns.',
      'Plural count labels use plural nouns.',
      'Raw count state should not force awkward UI wording.',
    ],
    id: 'count-wording-singular-plural',
    productLesson:
      'Public Forum pages should read like a product, not raw database counters.',
    regressionRefs: [
      'apps/web/src/page/forum.ts countText',
      'apps/web/src/forum-route.test.ts renders the public Forum index shell without listing void as a normal forum row',
    ],
    sourceRefs: [
      'docs/forum/classic-forum.md#what-to-borrow',
      'docs/forum/2026-06-05-mdk-money-moderated-forum-plan.md#board-ux-direction',
    ],
    status: 'implemented',
  },
] satisfies ReadonlyArray<ForumBehaviorFixture>
