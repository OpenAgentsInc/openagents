import { createFileRoute } from '@tanstack/react-router'

import { ForumTopicPage } from '../-forum-page'

const ForumTopicRouteComponent = () => {
  const { topicId } = Route.useParams()
  return <ForumTopicPage topicId={topicId} />
}

export const Route = createFileRoute('/forum/t/$topicId')({
  component: ForumTopicRouteComponent,
  head: ({ params }) => ({
    meta: [{ title: `${params.topicId.slice(0, 8)} Topic - OpenAgents` }],
  }),
})
