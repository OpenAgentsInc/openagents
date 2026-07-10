import { createFileRoute } from '@tanstack/react-router'

import { ForumForumPage } from '../-forum-page'

const ForumForumRouteComponent = () => {
  const { forumRef } = Route.useParams()
  return <ForumForumPage forumRef={forumRef} />
}

export const Route = createFileRoute('/forum/f/$forumRef')({
  component: ForumForumRouteComponent,
  head: ({ params }) => ({
    meta: [{ title: `${params.forumRef} Forum - OpenAgents` }],
  }),
})
