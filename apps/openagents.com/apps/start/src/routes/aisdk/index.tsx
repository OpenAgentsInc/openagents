import { createFileRoute } from '@tanstack/react-router'

import { AisdkPage } from '../-aisdk-page'

/** Public AI SDK page (owner-directed addition, 2026-07-21). */
export const Route = createFileRoute('/aisdk/')({
  component: AisdkPage,
  head: () => ({
    meta: [
      { title: 'AI SDK - OpenAgents' },
      {
        name: 'description',
        content:
          'The OpenAgents AI SDK: an Effect-native toolkit for building agent applications with durable, cursor-exact streams, coding-agent harnesses, redaction as a schema field, and recall instead of compaction.',
      },
    ],
  }),
})
