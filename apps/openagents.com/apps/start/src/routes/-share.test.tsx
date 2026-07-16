import type { ShareProjectionV1 } from '@openagentsinc/sync-schema'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ShareFailedView, ShareLoadedView, SharePage } from './-share-page'

const fixtureProjection: ShareProjectionV1 = {
  schemaVersion: 'openagents.share_projection.v1',
  id: 'share.public.fixture',
  url: 'https://openagents.com/share/share.public.fixture',
  audience: { _tag: 'Public' },
  audienceLabel: 'Public link',
  title: 'Fix the flaky retry test',
  subtitle: 'Autopilot session shared for review.',
  source: { kind: 'agent-run', id: 'run.fixture' },
  status: 'active',
  createdAt: '2026-07-01T12:34:56.000Z',
  updatedAt: '2026-07-01T12:34:56.000Z',
  messages: [
    {
      id: 'msg-1',
      author: 'user',
      label: 'chris',
      time: '2026-07-01T12:00:00.000Z',
      parts: [{ kind: 'text', body: ['Please fix the flaky retry test.'] }],
    },
    {
      id: 'msg-2',
      author: 'assistant',
      label: 'Adjutant',
      time: '2026-07-01T12:01:00.000Z',
      parts: [
        { kind: 'text', body: ['Looking into it now.'] },
        {
          kind: 'tool',
          title: 'Run tests',
          subtitle: 'shell: bun test',
          status: 'completed',
          detail: ['6 passed'],
        },
        {
          kind: 'diff',
          files: [
            { path: 'src/retry.ts', added: 4, removed: 1, status: 'modified' },
          ],
        },
        {
          kind: 'file',
          path: 'src/retry.ts',
          language: 'typescript',
          excerpt: ['export const retry = () => {}'],
        },
      ],
    },
  ],
  files: [{ label: 'src/retry.ts', meta: 'changed' }],
  artifacts: ['artifact.build.log'],
  approvals: [],
  receipts: [],
  metrics: { eventCount: 4, toolCallCount: 1, tokenTotal: 512 },
}

describe('Start /share/$shareId route', () => {
  test('server-renders the honest pre-fetch loading state', () => {
    const html = renderToStaticMarkup(
      <SharePage shareId="123e4567-e89b-42d3-a456-426614174000" />,
    )

    expect(html).toContain('data-route="share"')
    expect(html).toContain('Loading share')
    expect(html).toContain('Preparing the shared workroom.')
  })

  test('renders the full loaded shared timeline', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView projection={fixtureProjection} />,
    )

    expect(html).toContain('data-route="share"')
    expect(html).toContain('data-component="share-page"')
    expect(html).toContain('data-component="share-header"')
    expect(html).toContain('Public link')
    expect(html).toContain('Fix the flaky retry test')
    expect(html).toContain('Autopilot session shared for review.')
    expect(html).toContain('4 events')
    expect(html).toContain('1 tools')
    expect(html).toContain('512 tokens')
    expect(html).toContain('Copy link')
    expect(html).toContain('Open source run')
    expect(html).toContain('href="/t/run.fixture"')
  })

  test('renders every timeline part kind and rewrites the Adjutant codename', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView projection={fixtureProjection} />,
    )

    expect(html).toContain('Please fix the flaky retry test.')
    expect(html).toContain('Looking into it now.')
    expect(html).toContain('Run tests')
    expect(html).toContain('6 passed')
    expect(html).toContain('src/retry.ts')
    expect(html).toContain('aria-label="MOD src/retry.ts"')
    expect(html).toContain('<i>+4</i>')
    expect(html).toContain('<b>−1</b>')
    expect(html).toContain('export const retry = () =&gt; {}')
    expect(html).toContain('Autopilot')
    expect(html).not.toContain('Adjutant')
  })

  test('renders the review side panel and mobile review panel when files exist', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView projection={fixtureProjection} />,
    )

    expect(html).toContain('data-component="share-mobile-review"')
    expect(html).toContain('2 items')
    expect(html).toContain('artifact.build.log')
    expect(html).toContain('artifact')
  })

  test('renders an honest empty state for a share with no transcript messages', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView
        projection={{ ...fixtureProjection, messages: [] }}
      />,
    )

    expect(html).toContain('No messages')
    expect(html).toContain(
      'This share does not include transcript messages.',
    )
  })

  test('renders the sign-in gate for an unauthenticated 401 failure', () => {
    const html = renderToStaticMarkup(
      <ShareFailedView
        error="share_authentication_required"
        shareId="123e4567-e89b-42d3-a456-426614174000"
        status={401}
      />,
    )

    expect(html).toContain('data-route="share"')
    expect(html).toContain('Sign in to view this share')
    expect(html).toContain(
      'This share is restricted to specific OpenAgents members.',
    )
    expect(html).toContain(
      'href="/login/github?returnTo=%2Fshare%2F123e4567-e89b-42d3-a456-426614174000"',
    )
  })

  test('renders the forbidden state for a 403 failure', () => {
    const html = renderToStaticMarkup(
      <ShareFailedView error="share_forbidden" shareId="share.fixture" status={403} />,
    )

    expect(html).toContain('Share unavailable')
    expect(html).toContain('This share is not available to your account.')
    expect(html).toContain('href="/"')
  })

  test('distinguishes expired vs revoked for a 410 failure', () => {
    const expiredHtml = renderToStaticMarkup(
      <ShareFailedView
        error="share_expired"
        shareId="share.fixture"
        status={410}
      />,
    )
    const revokedHtml = renderToStaticMarkup(
      <ShareFailedView
        error="share_revoked"
        shareId="share.fixture"
        status={410}
      />,
    )

    expect(expiredHtml).toContain('Share expired')
    expect(revokedHtml).toContain('Share revoked')
  })

  test('renders the honest not-found fallback for any other failure', () => {
    const html = renderToStaticMarkup(
      <ShareFailedView error="unknown" shareId="share.fixture" status={404} />,
    )

    expect(html).toContain('Share not found')
    expect(html).toContain(
      'This share does not exist or is no longer available.',
    )
  })
})

// ---------------------------------------------------------------------------
// T14 (#8871): the widened `WorkroomTimelinePart` kinds render through the
// same shared @openagentsinc/ui/desktop-workbench components desktop uses.
// ---------------------------------------------------------------------------

const widenedProjection: ShareProjectionV1 = {
  ...fixtureProjection,
  id: 'share.public.widened',
  messages: [
    {
      id: 'msg-widened',
      author: 'assistant',
      label: 'Adjutant',
      time: '2026-07-01T12:01:00.000Z',
      parts: [
        { kind: 'reasoning', summary: 'Traced the flaky retry to a race.' },
        {
          kind: 'command',
          command: 'pnpm run test:retry',
          cwd: '/work/openagents',
          status: 'completed',
          exitCode: 0,
          outputTail: 'retry.test.ts passed',
        },
        {
          kind: 'fileChange',
          status: 'completed',
          changes: [
            { path: 'src/retry.ts', kind: 'update', adds: 3, dels: 1, diff: '+await sleep(10)' },
          ],
        },
        {
          kind: 'toolCall',
          callKind: 'mcp',
          tool: 'search_issues',
          server: 'github',
          args: [{ key: 'query', value: 'is:open retry' }],
          resultSnippet: '2 related issues',
          status: 'completed',
        },
        {
          kind: 'plan',
          entries: [
            { step: 'Reproduce the flake', status: 'completed' },
            { step: 'Add a deterministic wait', status: 'in_progress' },
          ],
        },
        { kind: 'approval', decision: 'approved', detail: 'Command looked safe.' },
        {
          kind: 'agent',
          tool: 'spawnAgent',
          prompt: 'Verify the fix under load.',
          status: 'running' as const,
          children: [{ threadRef: 'child_1', status: 'running' }],
        },
        { kind: 'notice', severity: 'warning', text: 'Model rerouted to a fallback.' },
        { kind: 'compaction' },
        { kind: 'meter', inputTokens: 120, outputTokens: 48, totalTokens: 168 },
        {
          kind: 'tool',
          title: 'Autopilot run',
          subtitle: 'openagents/autopilot-omega',
          status: 'completed',
          detail: ['completed · 4 events · 512 tokens'],
          actionHref: '/t/run.fixture',
          actionLabel: 'Open run',
        },
      ],
    },
  ],
}

describe('Start /share/$shareId widened timeline parts (T14 #8871)', () => {
  test('renders every widened kind through the shared workbench components', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView projection={widenedProjection} />,
    )

    expect(html).toContain('Traced the flaky retry to a race.')
    expect(html).toContain('pnpm run test:retry')
    expect(html).toContain('retry.test.ts passed')
    expect(html).toContain('src/retry.ts')
    expect(html).toContain('+await sleep(10)')
    expect(html).toContain('github')
    expect(html).toContain('search_issues')
    expect(html).toContain('2 related issues')
    expect(html).toContain('Reproduce the flake')
    expect(html).toContain('Add a deterministic wait')
    expect(html).toContain('Command looked safe.')
    expect(html).toContain('Verify the fix under load.')
    expect(html).toContain('Model rerouted to a fallback.')
    expect(html).toContain('CONTEXT COMPACTED')
  })

  test('preserves the legacy tool part actionHref/actionLabel as a link beside the shared card', () => {
    const html = renderToStaticMarkup(
      <ShareLoadedView projection={widenedProjection} />,
    )

    expect(html).toContain('oa-share-tool-wrapper')
    expect(html).toContain('href="/t/run.fixture"')
    expect(html).toContain('Open run')
  })
})
