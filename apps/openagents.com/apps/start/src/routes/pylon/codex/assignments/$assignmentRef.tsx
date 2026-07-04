import { createFileRoute } from '@tanstack/react-router'

import { PylonCodexAssignmentStatusPage } from '../../../-pylon-codex-assignment-status-page'

export const Route = createFileRoute(
  '/pylon/codex/assignments/$assignmentRef',
)({
  component: PylonCodexAssignmentStatusRoute,
  head: ({ params }) => ({
    meta: [
      {
        title: `Pylon Codex assignment ${params.assignmentRef} - OpenAgents`,
      },
    ],
  }),
})

function PylonCodexAssignmentStatusRoute() {
  const { assignmentRef } = Route.useParams()

  return <PylonCodexAssignmentStatusPage assignmentRef={assignmentRef} />
}
