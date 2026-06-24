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
// This module ships the FRAME + the teaching Overview empty state only. The real
// operator features (run/session inspector, operator actions, the
// hosted-executor view, real usage metering, connect-a-coding-agent) are
// explicit follow-ups per docs/pro/2026-06-24-pro-operator-ui-revival-audit.md.
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
  { label: 'Overview', active: true, href: proRouter() },
  { label: 'Traces', href: tracesHref },
  { label: 'Compare', href: compareHref },
  { label: 'Sessions', disabled: true },
  { label: 'Settings', disabled: true },
]

const overviewEmptyState = (): Html =>
  Ui.proTeachingEmptyState<Message>({
    title: 'Pro is a power-user operator console',
    body: 'Run, inspect, and review machine work in one place. Sharing lives at /trace — every run renders as a public, shareable trace, and several runs compare side by side.',
    footnote:
      'Open a shared trace to see the full step timeline; open a comparison to see how config or MCP changes move verdict, latency, steps, and cost.',
    affordances: [
      Ui.proLinkAffordance<Message>({
        label: 'View a shared trace',
        href: tracesHref,
      }),
      Ui.proLinkAffordance<Message>({ label: 'Compare traces', href: compareHref }),
      Ui.proComingAffordance<Message>({
        label: 'Connect a coding agent',
        hint: 'Connecting a coding agent is coming — not active yet.',
      }),
    ],
  })

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
    main: Ui.proMainPane<Message>([overviewEmptyState()]),
  })

// A safe forward path for any "go back" affordance the shell may need.
export const backHref = (): string => orderRouter()
