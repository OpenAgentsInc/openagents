import { createFileRoute, notFound } from '@tanstack/react-router'

import { BlogPostPage } from '../-funnel-components'
import { findBlogPost, type BlogPost } from '../-funnel-data'

export const Route = createFileRoute('/blog/$slug')({
  component: BlogPostRoute,
  head: ({ params }) => {
    const post = findBlogPost(params.slug)
    return {
      meta: [
        { title: `${post?.title ?? 'Blog'} - OpenAgents` },
        {
          name: 'description',
          content: post?.excerpt ?? 'OpenAgents blog.',
        },
      ],
    }
  },
  loader: ({ params }) => {
    const post = findBlogPost(params.slug)
    if (post === undefined) {
      throw notFound()
    }
    return { post }
  },
})

function BlogPostRoute() {
  const { post } = Route.useLoaderData() as { post: BlogPost }
  return <BlogPostPage post={post} />
}
