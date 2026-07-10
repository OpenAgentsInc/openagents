import { createFileRoute } from '@tanstack/react-router'

import { ForumReceiptPage } from '../-forum-page'

const ForumReceiptRouteComponent = () => {
  const { receiptRef } = Route.useParams()
  return <ForumReceiptPage receiptRef={receiptRef} />
}

export const Route = createFileRoute('/forum/receipts/$receiptRef')({
  component: ForumReceiptRouteComponent,
  head: () => ({
    meta: [{ title: 'Forum Receipt - OpenAgents' }],
  }),
})
