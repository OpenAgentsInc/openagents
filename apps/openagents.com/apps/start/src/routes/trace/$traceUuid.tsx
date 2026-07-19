import { createFileRoute } from '@tanstack/react-router'

import { TracePage } from '../-trace-page'

/** Public or owner-authorized evidence viewer for one stored ATIF trace. */
export const Route = createFileRoute('/trace/$traceUuid')({
  component: TraceRoutePage,
  head: () => ({
    meta: [{ title: 'Trace - OpenAgents' }],
  }),
})

function TraceRoutePage() {
  const { traceUuid } = Route.useParams()

  return <TracePage traceUuid={traceUuid} />
}
