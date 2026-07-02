import type { Html } from 'foldkit/html'

import {
  orderRouter,
  proRouter,
  traceCompareRouter,
  traceRouter,
} from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { Model, ProAgentDashboardResponse } from '../model'
import { SAMPLE_COMPARE_PATH_IDS } from '../../trace-compare/sample'
import { SAMPLE_TRACE_UUID } from '../../trace/sample'

// ---------------------------------------------------------------------------
// /pro — operator / power-user console SHELL (issue 6179)
// ---------------------------------------------------------------------------
//
// This module ships the FRAME + a bounded operator dashboard model. The agent
// rows are loaded from the owner-scoped runner-neutral status spine; no local
// sample agents are rendered.
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

export const loadingView = (): Html =>
  Ui.proMainPane<Message>([Ui.proSkeletonRows<Message>()])

export const errorView = (detail: string): Html =>
  Ui.proMainPane<Message>([Ui.proErrorStrip<Message>(detail)])

const topStrip = (model: Model): Html =>
  Ui.proTopStrip<Message>({
    homeHref: proRouter(),
    breadcrumb: 'Operator console',
    creditsLabel: 'credits',
    creditsState: 'soon',
    creditsHint: 'Usage metering is coming to Pro — not active yet.',
    accountLabel: model.session.email,
  })

const dashboardView = (response: ProAgentDashboardResponse): Html =>
  Ui.proMainPane<Message>([
    Ui.proAgentDashboard<Message>(response as Ui.ProAgentDashboardSnapshot),
  ])

const mainPane = (model: Model): Html => {
  const state = model.proAgentDashboard

  if (
    state._tag === 'ProAgentDashboardIdle' ||
    state._tag === 'ProAgentDashboardLoading'
  ) {
    return loadingView()
  }

  if (state._tag === 'ProAgentDashboardFailed') {
    return errorView(state.error)
  }

  return dashboardView(state.response)
}

export const view = (model: Model): Html =>
  Ui.proConsoleShell<Message>({
    topStrip: topStrip(model),
    register: Ui.proRegister<Message>(SECTIONS),
    main: mainPane(model),
  })

// A safe forward path for any "go back" affordance the shell may need.
export const backHref = (): string => orderRouter()
