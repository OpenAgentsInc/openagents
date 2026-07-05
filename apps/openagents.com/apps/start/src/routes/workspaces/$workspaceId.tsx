import { createFileRoute } from '@tanstack/react-router'

import { WorkspaceInvitePage } from '../-workspace-invite-page'

export const Route = createFileRoute('/workspaces/$workspaceId')({
  component: WorkspaceInviteRoutePage,
  head: () => ({
    meta: [
      { title: 'Open your project workspace - OpenAgents' },
      {
        name: 'description',
        content:
          'Sign in with GitHub to open your invited OpenAgents project workspace.',
      },
    ],
  }),
})

function WorkspaceInviteRoutePage() {
  const { workspaceId } = Route.useParams()

  return <WorkspaceInvitePage workspaceId={workspaceId} />
}
