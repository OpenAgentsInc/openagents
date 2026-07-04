import { createFileRoute } from '@tanstack/react-router'

import { SiteCheckoutDemoPage } from '../../-site-checkout-demo-page'

export const Route = createFileRoute('/sites/demo-checkout/$returnAction')({
  component: DemoCheckoutReturnRoute,
  head: ({ params }) => ({
    meta: [{ title: `Demo checkout ${params.returnAction} - OpenAgents` }],
  }),
})

function DemoCheckoutReturnRoute() {
  const { returnAction } = Route.useParams()

  return <SiteCheckoutDemoPage returnAction={returnAction} />
}
