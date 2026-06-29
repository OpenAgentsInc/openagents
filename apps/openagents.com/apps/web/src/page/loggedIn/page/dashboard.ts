import type { Html } from 'foldkit/html'

import { Session } from '../../../domain/session'
import { chatRouter } from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'

export const view = (session: Session): Html =>
  Ui.container<Message>([
    Ui.applicationHomeScreen<Message>({
      eyebrow: 'OpenAgents',
      title: `Welcome back, ${session.name}`,
      body: 'Autopilot sessions, connected credentials, and team workrooms live in the command surface.',
      stats: [
        { label: 'Sessions', value: '42', tone: 'accent' },
        { label: 'Projects', value: '7', tone: 'info' },
        { label: 'Completed', value: '128', tone: 'positive' },
      ],
      steps: [
        {
          label: 'Identity',
          detail: session.email,
          tone: 'positive',
          active: true,
        },
        {
          label: 'Provider account',
          detail: 'ChatGPT account connected in the workroom.',
          tone: 'positive',
        },
        {
          label: 'Repo push',
          detail: 'Scoped GitHub write access for branch delivery.',
          tone: 'positive',
        },
        {
          label: 'Runtime',
          detail: 'Primary computer, backup computer.',
          tone: 'accent',
        },
      ],
      action: Ui.linkButton<Message>({
        href: chatRouter(),
        label: 'Open chat',
        size: 'sm',
      }),
      aside: Ui.calendarMonth<Message>({
        title: 'Mission window',
        days: Array.from({ length: 35 }, (_, index) => ({
          label: String(index + 1),
          ...(index === 1
            ? { meta: 'Computer', tone: 'accent' as const, active: true }
            : {}),
          ...(index === 8 ? { meta: 'Review', tone: 'info' as const } : {}),
          ...(index === 15
            ? { meta: 'Receipt', tone: 'positive' as const }
            : {}),
        })),
      }),
    }),
    Ui.actionPanel<Message>({
      eyebrow: 'Runtime lanes',
      title: 'Primary computer with backup computer',
      body: 'Autopilot assignments launch through the primary computer first. A backup computer remains available for recovery.',
      tone: 'accent',
      action: Ui.linkButton<Message>({
        href: chatRouter(),
        label: 'Open workroom',
        size: 'sm',
        variant: 'secondary',
      }),
    }),
    Ui.commerceCategoryPage<Message>({
      title: 'Launch lanes',
      body: 'Ecommerce category and product patterns adapted into operational launch inventory.',
      filters: [
        {
          label: 'Runtime',
          options: [
            { label: 'Primary computer', count: '1' },
            { label: 'Backup computer', count: '1' },
          ],
        },
        {
          label: 'Delivery',
          options: [
            { label: 'Branches', count: 'ready' },
            { label: 'Issue comments', count: 'ready' },
          ],
        },
      ],
      products: [
        {
          title: 'Primary computer',
          detail: 'Fast lane for Autopilot assignments.',
          price: 'online',
          rating: 5,
          reviewCount: 8,
          swatches: ['accent', 'positive'],
        },
        {
          title: 'Backup computer',
          detail:
            'Standby recovery lane when the primary computer is unavailable.',
          price: 'standby',
          rating: 4,
          reviewCount: 2,
          swatches: ['neutral', 'info'],
        },
        {
          title: 'Repo delivery',
          detail: 'Branches, commits, issue comments, and pull requests.',
          price: 'ready',
          rating: 5,
          reviewCount: 7,
          swatches: ['positive', 'info'],
        },
      ],
    }),
    Ui.commerceOrderDetailPage<Message>({
      title: 'Mission prerequisites',
      lines: [
        {
          title: 'ChatGPT account',
          detail: 'Connected provider credential.',
          quantity: '1',
          status: 'healthy',
          tone: 'positive',
        },
        {
          title: 'Repo push grant',
          detail: 'Scoped GitHub write access.',
          quantity: '1',
          status: 'healthy',
          tone: 'positive',
        },
        {
          title: 'Computer runtime',
          detail: 'Primary execution lane.',
          quantity: '1',
          status: 'primary',
          tone: 'accent',
        },
      ],
      summary: [
        { label: 'Identity', value: 'ready', tone: 'positive' },
        { label: 'Provider', value: 'ready', tone: 'positive' },
        { label: 'Runtime', value: 'computer', tone: 'accent' },
        { label: 'Launch state', value: 'available', strong: true },
      ],
    }),
    Ui.incentiveGrid<Message>([
      {
        title: 'Replayable receipts',
        body: 'Every run keeps commits, logs, comments, and deployment evidence together.',
        tone: 'info',
      },
      {
        title: 'Scoped credentials',
        body: 'Provider and repo grants are resolved just in time for each assignment.',
        tone: 'positive',
      },
      {
        title: 'Fallback lane',
        body: 'A backup computer remains available when the primary lane is blocked.',
        tone: 'accent',
      },
    ]),
  ])
