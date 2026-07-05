import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { WorkspaceInvitePage } from './-workspace-invite-page'

describe('Start workspace invite route', () => {
  test('server-renders the sign-in gate for a specific workspace', () => {
    const html = renderToStaticMarkup(
      <WorkspaceInvitePage workspaceId="workspace.public.invite_example" />,
    )

    expect(html).toContain('data-route="workspace-invite"')
    expect(html).toContain('Workspace invite')
    expect(html).toContain('Open your project workspace')
    expect(html).toContain(
      'Your project setup is waiting. Sign in to review the seeded notes and starter workflows.',
    )
    expect(html).toContain('workspace.public.invite_example')
    expect(html).toContain('href="/login/github"')
    expect(html).toContain('Log in with GitHub')
  })

  test('keeps the invite gate free of any pre-authenticated session content', () => {
    const html = renderToStaticMarkup(
      <WorkspaceInvitePage workspaceId="workspace.public.invite_example" />,
    )

    expect(html).not.toContain('seeded notes and starter workflows.</p><ul')
    expect(html).not.toMatch(/session|logout|dashboard/i)
  })
})
