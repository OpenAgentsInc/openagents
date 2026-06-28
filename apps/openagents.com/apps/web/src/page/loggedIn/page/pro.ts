import type { Html } from 'foldkit/html'

import type { Session } from '../../../domain/session'
import {
  orderRouter,
  proRouter,
  traceCompareRouter,
  traceRouter,
} from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import { SAMPLE_COMPARE_PATH_IDS } from '../../trace-compare/sample'
import { SAMPLE_TRACE_UUID } from '../../trace/sample'

// ---------------------------------------------------------------------------
// /pro — operator / power-user console SHELL (issue 6179)
// ---------------------------------------------------------------------------
//
// This module ships the FRAME + a bounded operator dashboard model. The live
// ingest/send endpoints remain explicit follow-ups; this page only renders
// public-safe sample state that matches the production contract shape.
//
// All class-bearing markup lives in the shared `@openagentsinc/ui` Pro console
// primitives (proConsoleShell / proTopStrip / proRegister / ...). This page only
// wires data and composes those registry components, per DESIGN.md and the
// Foldkit-UI-composition guard.
//
// The left register lists the live + future sections honestly: Overview is the
// live operator landing; Traces / Compare link out to the public, shareable
// `/trace/{uuid}` + `/trace/compare/{ids}` surfaces (the former `/pro/runs` +
// `/pro/evals` fixture subpages were retired in #6215). Sessions / Settings stay
// honest disabled placeholders, never fake links.

const tracesHref = traceRouter({ uuid: SAMPLE_TRACE_UUID })
const compareHref = traceCompareRouter({ ids: SAMPLE_COMPARE_PATH_IDS })

const SECTIONS: ReadonlyArray<Ui.ProConsoleSection> = [
  { label: 'Agents', active: true, href: proRouter() },
  { label: 'Traces', href: tracesHref },
  { label: 'Compare', href: compareHref },
  { label: 'Sessions', disabled: true },
  { label: 'Settings', disabled: true },
]

const agentDashboardSnapshot: Ui.ProAgentDashboardSnapshot = {
  generatedAt: '2026-06-27T18:44:00Z',
  liveEntries: [
    {
      id: 'pane.codex-1',
      agentLabel: 'Codex lane 1',
      worktreeLabel: 'issue-6406-agent-dashboard',
      state: 'working',
      prompt: 'Build the operator dashboard status model and review queue.',
      updatedAt: '18:43:58Z',
      stateStartedAt: '18:36:14Z',
      acknowledgedAt: '18:31:09Z',
      unread: true,
      toolName: 'bun test',
      lastAssistantMessage:
        'Status model is shaped; wiring the Pro surface and scene coverage now.',
      stateHistory: [
        {
          state: 'waiting',
          label: 'Workspace materialized',
          at: '18:31Z',
        },
        {
          state: 'working',
          label: 'Dashboard implementation started',
          at: '18:36Z',
        },
      ],
    },
    {
      id: 'pane.claude-1',
      agentLabel: 'Claude lane 1',
      worktreeLabel: 'status-ingest-runbook',
      state: 'blocked',
      prompt: 'Verify owner-scoped status ingest prerequisites.',
      updatedAt: '18:42:10Z',
      stateStartedAt: '18:39:02Z',
      acknowledgedAt: '18:39:02Z',
      unread: false,
      toolName: 'owner gate',
      lastAssistantMessage:
        'NEEDS-OWNER: confirm the live relay endpoint before enabling send.',
      stateHistory: [
        {
          state: 'working',
          label: 'Read relay docs',
          at: '18:33Z',
        },
        {
          state: 'blocked',
          label: 'Owner endpoint confirmation needed',
          at: '18:39Z',
        },
      ],
    },
    {
      id: 'pane.opencode-1',
      agentLabel: 'OpenCode lane',
      worktreeLabel: 'future-runner-fixture',
      state: 'waiting',
      prompt: 'Hold for runner registry integration.',
      updatedAt: '18:40:25Z',
      stateStartedAt: '18:40:25Z',
      acknowledgedAt: '18:40:25Z',
      unread: false,
      toolName: 'queue',
      lastAssistantMessage:
        'Waiting for a registry-backed runner before assignment.',
      stateHistory: [
        {
          state: 'waiting',
          label: 'Queued behind available Codex capacity',
          at: '18:40Z',
        },
      ],
    },
  ],
  retainedEntries: [
    {
      id: 'pane.codex-0',
      agentLabel: 'Codex lane 0',
      worktreeLabel: 'trace-compare-polish',
      state: 'done',
      prompt: 'Retain the completed run until the operator dismisses it.',
      updatedAt: '18:22:31Z',
      stateStartedAt: '18:22:31Z',
      acknowledgedAt: '18:25:00Z',
      unread: false,
      toolName: 'verify',
      lastAssistantMessage:
        'Proof-ready with public trace refs; retained for review.',
      stateHistory: [
        {
          state: 'working',
          label: 'Compared trace variants',
          at: '18:07Z',
        },
        {
          state: 'done',
          label: 'Verification completed',
          at: '18:22Z',
        },
      ],
    },
  ],
  diffComments: [
    {
      id: 'diff-comment-1',
      filePath: 'apps/openagents.com/apps/web/src/page/loggedIn/page/pro.ts',
      lineLabel: 'agent dashboard',
      body: 'Keep stateStartedAt distinct from updatedAt so tool pings do not clear unread state.',
      selectedText: 'stateStartedAt drives unread; updatedAt only shows freshness.',
      targetAgentLabel: 'Codex lane 1',
      sentAt: 'staged',
    },
    {
      id: 'diff-comment-2',
      filePath: 'packages/ui/src/pro.ts',
      lineLabel: 'diff queue',
      body: 'Group queued review comments by target agent before enabling the send endpoint.',
      selectedText: 'diffComments are retained as operator review intent.',
      targetAgentLabel: 'Claude lane 1',
      sentAt: 'staged',
    },
  ],
}

// LOADING STATE: skeleton rows (not a center spinner). Exported so the shell can
// render it while the future run/session index is fetching.
export const loadingView = (): Html =>
  Ui.proMainPane<Message>([Ui.proSkeletonRows<Message>()])

// ERROR STATE: a compact inline error strip (not a full-page error). Exported so
// the shell can surface a failed future fetch without losing the console frame.
export const errorView = (detail: string): Html =>
  Ui.proMainPane<Message>([Ui.proErrorStrip<Message>(detail)])

const topStrip = (session: Session): Html =>
  Ui.proTopStrip<Message>({
    homeHref: proRouter(),
    breadcrumb: 'Operator console',
    creditsLabel: 'credits',
    creditsState: 'soon',
    creditsHint: 'Usage metering is coming to Pro — not active yet.',
    accountLabel: session.email,
  })

// The Pro console shell. Rendered as a top-level page (see view.ts) so it owns
// the full top-strip + left-register + main-pane layout, independent of the
// workroom sidebar shell.
export const view = (session: Session): Html =>
  Ui.proConsoleShell<Message>({
    topStrip: topStrip(session),
    register: Ui.proRegister<Message>(SECTIONS),
    main: Ui.proMainPane<Message>([
      Ui.proAgentDashboard<Message>(agentDashboardSnapshot),
    ]),
  })

// A safe forward path for any "go back" affordance the shell may need.
export const backHref = (): string => orderRouter()
