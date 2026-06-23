import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import {
  AutopilotRoute,
  AutopilotVerticalRoute,
} from '../../route'
import {
  ClickedAutopilotOnboardingCreditKickoff,
  FailedAutopilotOnboardingTurn,
  SubmittedAutopilotOnboardingTurn,
  SucceededAutopilotOnboardingTurn,
  UpdatedAutopilotOnboardingComposer,
} from '../loggedOut/message'
import { init } from '../loggedOut/model'
import { update } from '../loggedOut/update'
import {
  type FlowModel,
  type OnboardingTurnResponse,
  capturedSectionCount,
  currentSectionIndex,
  deriveComponentFrames,
  deriveIntakeRegister,
  initFlowModel,
  isQuoteReady,
} from './flow'
import {
  HUD_COMPONENT_ITEM_ATTR,
  HUD_COMPOSER_ATTR,
  HUD_LEGAL_OVERLAY_ATTR,
  HUD_LEGAL_STAT_STRIP_ATTR,
  HUD_LEGAL_VSL_ATTR,
  HUD_REGISTER_ATTR,
  HUD_ROOT_ATTR,
  HUD_THREAD_END_ATTR,
  LEGAL_VERIFIED_STATS,
  overlayView,
} from './page'
import {
  OpenedAutopilotOnboardingStream,
  ReceivedAutopilotOnboardingDelta,
} from '../loggedOut/message'
import { onboardingVerticalForSegment } from './vertical-overlay'

// A minimal Snabbdom-style vnode walker so the test can assert rendered markup
// without a DOM. Mirrors the existing scene/route tests.
type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    style?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const renderHtml = (node: Html | string | null): string => {
  if (node === null || typeof node === 'string') {
    return typeof node === 'string' ? node : ''
  }
  if (!isVNodeLike(node)) {
    return ''
  }

  const attrs = node.data?.attrs ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([name]) => name)
    .join(' ')
  const styleEntries = Object.entries(node.data?.style ?? {})
  const attrString = [
    ...Object.entries(attrs),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
    ...(styleEntries.length === 0
      ? []
      : [
          [
            'style',
            styleEntries.map(([k, v]) => `${k}:${String(v)}`).join(';'),
          ] as const,
        ]),
  ]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join('')

  const tag = node.sel ?? 'node'
  const children = (node.children ?? []).map(renderHtml).join('')
  return `<${tag}${attrString}>${node.text ?? ''}${children}</${tag}>`
}

// Stub action constructors for the page view; the markup tests don't dispatch.
const stubActions = {
  updatedComposer: (value: string) => ({ _tag: 'stub', value }),
  submittedTurn: () => ({ _tag: 'stub' }),
  clickedCreditKickoff: () => ({ _tag: 'stub' }),
}

// A stubbed program turn response (the Khala program is not invoked in the unit
// test; the success message carries this exact shape the real route returns).
const turnResponse = (
  overrides: Partial<OnboardingTurnResponse>,
): OnboardingTurnResponse => ({
  sessionId: 'ob_test_session',
  reply: 'Thanks — tell me more.',
  status: 'interviewing',
  turnCount: 1,
  outputSpec: {},
  ...overrides,
})

describe('autopilot onboarding — output spec progress', () => {
  test('counts captured (non-blank) sections and finds the current step', () => {
    const spec = { business: 'Acme', goal: 'Launch a site', quickWin: '  ' }
    expect(capturedSectionCount(spec)).toBe(2)
    // business (0) and goal (1) captured; chosenOfferings (2) is the first open
    // section, so the current step is index 2 (quickWin is blank, not captured).
    expect(currentSectionIndex(spec)).toBe(2)
  })

  test('quote-ready once business + goal + quick win are captured', () => {
    expect(isQuoteReady({})).toBe(false)
    expect(isQuoteReady({ business: 'Acme', goal: 'Ship' })).toBe(false)
    expect(
      isQuoteReady({ business: 'Acme', goal: 'Ship', quickWin: 'NDA pack' }),
    ).toBe(true)
  })
})

describe('autopilot onboarding — derived component frames', () => {
  test('surfaces nothing inline on the first render (intake is the sidebar register)', () => {
    // intake_progress moved to the sidebar register (problem #3); the inline
    // component column starts empty until facts are captured.
    const frames = deriveComponentFrames(initFlowModel(Option.none()))
    expect(frames).toHaveLength(0)
  })

  test('legal vertical adds a consent_gate', () => {
    const frames = deriveComponentFrames(initFlowModel(Option.some('legal')))
    const components = frames.map(frame =>
      frame._tag === 'CatalogComponentFrame' ? frame.frame.component : 'unknown',
    )
    expect(components).toEqual(['consent_gate'])
  })

  test('quote-ready state surfaces quick_win, dashboard_preview, credit_kickoff', () => {
    const model: FlowModel = {
      ...initFlowModel(Option.none()),
      outputSpec: {
        business: 'Acme Co',
        goal: 'Launch a marketing site',
        quickWin: 'A one-page launch site',
        scope: 'Single page, copy + deploy',
      },
    }
    const components = deriveComponentFrames(model).map(frame =>
      frame._tag === 'CatalogComponentFrame' ? frame.frame.component : 'unknown',
    )
    expect(components).toEqual([
      'quick_win_card',
      'dashboard_preview',
      'credit_kickoff',
    ])
  })
})

describe('autopilot onboarding — intake register (sidebar)', () => {
  test('lights up captured sections and marks the current step active', () => {
    const model: FlowModel = {
      ...initFlowModel(Option.none()),
      outputSpec: { business: 'Acme', goal: 'Launch a site' },
    }
    const register = deriveIntakeRegister(model)
    expect(register).toHaveLength(10)
    expect(register[0]).toMatchObject({ id: 'business', status: 'done' })
    expect(register[1]).toMatchObject({ id: 'goal', status: 'done' })
    // chosenOfferings is the first open section -> active.
    expect(register[2]).toMatchObject({ id: 'chosenOfferings', status: 'active' })
    expect(register[3]).toMatchObject({ status: 'queued' })
  })
})

describe('autopilot onboarding — page overlay rendering', () => {
  test('renders the HUD shell, composer, scrollable thread, and sidebar register on first paint', () => {
    const markup = renderHtml(
      overlayView(initFlowModel(Option.none()), stubActions),
    )
    expect(markup).toContain(`data-${HUD_ROOT_ATTR}`)
    expect(markup).toContain(`data-${HUD_COMPOSER_ATTR}`)
    // The intake progress is now a compact sidebar register (problem #3), not a
    // giant "Onboarding progress" box in the main column.
    expect(markup).toContain(`data-${HUD_REGISTER_ATTR}`)
    expect(markup).toContain('Intake')
    expect(markup).toContain('0/10')
    expect(markup).not.toContain('Onboarding progress')
    // The thread is an internal scroll region with a bottom sentinel (problem #2).
    expect(markup).toContain('oa-thread-scroll')
    expect(markup).toContain(`data-${HUD_THREAD_END_ATTR}="true"`)
    // No credit kickoff before quote-ready.
    expect(markup).not.toContain('Kick off the work')
  })

  test('renders a clickable credit_kickoff once quote-ready', () => {
    const model: FlowModel = {
      ...initFlowModel(Option.none()),
      outputSpec: {
        business: 'Acme Co',
        goal: 'Launch a site',
        quickWin: 'A one-page launch site',
      },
    }
    const markup = renderHtml(overlayView(model, stubActions))
    expect(markup).toContain('Kick off the work')
    // The kickoff confirmation exposes the credit amount ($5.00).
    expect(markup).toContain('$5')
  })

  test('component surfaces flutter in with a deterministic per-index delay', () => {
    const model: FlowModel = {
      ...initFlowModel(Option.none()),
      outputSpec: {
        business: 'Acme Co',
        goal: 'Launch a site',
        quickWin: 'A one-page launch site',
      },
    }
    const markup = renderHtml(overlayView(model, stubActions))
    // Each component item carries the flutter-in class and an inline,
    // order-derived animation delay (no Math.random / time-of-day).
    expect(markup).toContain('oa-flutter-in')
    expect(markup).toContain(`data-${HUD_COMPONENT_ITEM_ATTR}="0"`)
    expect(markup).toContain('animationDelay:0ms')
    expect(markup).toContain('animationDelay:70ms')
  })

  test('reduced-motion: the surface ships fully visible (no class-gated blank)', () => {
    // The flutter-in animation is enhancement-only — the CSS reduced-motion
    // fallback disables the animation and the surface is already present in the
    // markup with no opacity:0 / hidden gate. Asserting the content is rendered
    // unconditionally is the headless/reduced-motion contract.
    const markup = renderHtml(
      overlayView(initFlowModel(Option.none()), stubActions),
    )
    // No inline opacity gate and no `hidden`/`display:none` style — the content
    // is present unconditionally (the flutter-in is enhancement-only).
    expect(markup).not.toContain('opacity:0')
    expect(markup).not.toContain('display:none')
    expect(markup).not.toContain('style="hidden"')
    expect(markup).toContain('Put an AI workforce to work')
  })
})

describe('autopilot onboarding — turn loop (via the loggedOut update)', () => {
  const flowOf = (model: { autopilotOnboarding: FlowModel }): FlowModel =>
    model.autopilotOnboarding

  test('submitting a turn appends the user turn, clears the draft, sets a pending turn, and scrolls', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    expect(flowOf(typed).composerDraft).toBe('I run a bakery')

    const [submitted, commands] = update(
      typed,
      SubmittedAutopilotOnboardingTurn(),
    )
    expect(flowOf(submitted).status).toBe('submitting')
    expect(flowOf(submitted).composerDraft).toBe('')
    expect(flowOf(submitted).transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
    ])
    // The turn is now driven by the streaming subscription: submit sets a
    // pendingTurn (the subscription opens the SSE stream) and scrolls to the
    // just-sent message. No fetch command fires on the pure path.
    const pending = flowOf(submitted).pendingTurn
    expect(pending).not.toBeNull()
    expect(pending?.userText).toBe('I run a bakery')
    expect(commands.map(command => command.name)).toEqual([
      'ScrollAutopilotOnboardingThreadToEnd',
    ])
  })

  test('the streaming lifecycle: open -> deltas accumulate -> done commits and resets', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())
    const turnId = flowOf(submitted).pendingTurn?.id ?? ''
    expect(turnId).not.toBe('')

    const [opened] = update(
      submitted,
      OpenedAutopilotOnboardingStream({ turnId }),
    )
    expect(flowOf(opened).status).toBe('streaming')
    expect(flowOf(opened).streamingReply).toBe('')

    const [d1] = update(
      opened,
      ReceivedAutopilotOnboardingDelta({ turnId, text: 'Great' }),
    )
    const [d2] = update(
      d1,
      ReceivedAutopilotOnboardingDelta({ turnId, text: ' — what next?' }),
    )
    expect(flowOf(d2).streamingReply).toBe('Great — what next?')
    expect(flowOf(d2).status).toBe('streaming')

    const [done] = update(
      d2,
      SucceededAutopilotOnboardingTurn({
        response: turnResponse({
          reply: 'Great — what next?',
          outputSpec: { business: 'A neighborhood bakery' },
          turnCount: 1,
        }),
      }),
    )
    const flow = flowOf(done)
    expect(flow.status).toBe('idle')
    expect(flow.streamingReply).toBeNull()
    expect(flow.pendingTurn).toBeNull()
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
      { role: 'assistant', content: 'Great — what next?' },
    ])
  })

  test('a stale delta for a resolved turn is ignored', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'hi' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())
    const [done] = update(
      submitted,
      SucceededAutopilotOnboardingTurn({
        response: turnResponse({ reply: 'ok', turnCount: 1 }),
      }),
    )
    // A late delta for the now-cleared turn must not mutate the committed state.
    const [after] = update(
      done,
      ReceivedAutopilotOnboardingDelta({ turnId: 'new:1', text: 'late' }),
    )
    expect(flowOf(after).streamingReply).toBeNull()
    expect(flowOf(after).transcript).toEqual([
      { role: 'user', content: 'hi' },
      { role: 'assistant', content: 'ok' },
    ])
  })

  test('an empty or in-flight composer never fires a turn', () => {
    const base = init(AutopilotRoute())
    const [noop, commands] = update(base, SubmittedAutopilotOnboardingTurn())
    expect(commands).toEqual([])
    expect(flowOf(noop).transcript).toEqual([])
  })

  test('a succeeded turn appends the assistant reply and accumulates the spec', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())

    const [resolved] = update(
      submitted,
      SucceededAutopilotOnboardingTurn({
        response: turnResponse({
          reply: 'Great — what is the first thing you want done?',
          outputSpec: { business: 'A neighborhood bakery' },
          turnCount: 1,
        }),
      }),
    )

    const flow = flowOf(resolved)
    expect(flow.status).toBe('idle')
    expect(flow.sessionId).toBe('ob_test_session')
    expect(flow.turnCount).toBe(1)
    expect(flow.outputSpec.business).toBe('A neighborhood bakery')
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
      {
        role: 'assistant',
        content: 'Great — what is the first thing you want done?',
      },
    ])

    // The accumulated spec now lights up an additional intake section and a
    // dashboard preview (the business is known).
    const components = deriveComponentFrames(flow).map(frame =>
      frame._tag === 'CatalogComponentFrame' ? frame.frame.component : 'unknown',
    )
    expect(components).toContain('dashboard_preview')
  })

  test('reaching a quote-ready spec surfaces a credit_kickoff that dispatches the kickoff command', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'Launch my bakery site' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())
    const [resolved] = update(
      submitted,
      SucceededAutopilotOnboardingTurn({
        response: turnResponse({
          outputSpec: {
            business: 'A neighborhood bakery',
            goal: 'Launch a one-page site',
            quickWin: 'A one-page launch site',
          },
          status: 'complete',
          turnCount: 2,
        }),
      }),
    )

    const markup = renderHtml(
      overlayView(flowOf(resolved), stubActions),
    )
    expect(markup).toContain('Kick off the work')

    // The kickoff click routes into the funded GitHub login path.
    const [, commands] = update(
      resolved,
      ClickedAutopilotOnboardingCreditKickoff(),
    )
    expect(commands.map(command => command.name)).toEqual([
      'OpenAutopilotCreditKickoff',
    ])
  })

  test('a failed turn sets an error reason and leaves the transcript intact', () => {
    const base = init(AutopilotVerticalRoute({ vertical: 'legal' }))
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'Help with an NDA' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())
    // The bounded vertical is threaded to the pending turn on the first turn;
    // server-owned prompt guidance stays server-side.
    expect(flowOf(submitted).vertical).toBe('legal')
    expect(flowOf(submitted).pendingTurn?.vertical).toBe('legal')

    // Simulate the stream's failure terminal.
    const [failed] = update(
      submitted,
      FailedAutopilotOnboardingTurn({
        reason: 'Autopilot could not respond just now. Try sending that again.',
      }),
    )
    const flow = flowOf(failed)
    expect(flow.status).toBe('error')
    expect(flow.errorReason).toContain('could not respond')
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'Help with an NDA' },
    ])
  })
})

describe('autopilot onboarding — legal vertical selection (#6148)', () => {
  test('maps route segments to bounded server-owned vertical selectors', () => {
    expect(onboardingVerticalForSegment('legal')).toBe('legal')
    expect(onboardingVerticalForSegment(null)).toBe('general')
    expect(onboardingVerticalForSegment('health')).toBe('general')
    expect(onboardingVerticalForSegment('LEGAL')).toBe('general')
  })

  test('legal vertical renders the legal overlay: VSL slot, intro, and verified stat strip', () => {
    const markup = renderHtml(
      overlayView(initFlowModel(Option.some('legal')), stubActions),
    )
    expect(markup).toContain(`data-${HUD_LEGAL_OVERLAY_ATTR}`)
    expect(markup).toContain(`data-${HUD_LEGAL_VSL_ATTR}`)
    expect(markup).toContain(`data-${HUD_LEGAL_STAT_STRIP_ATTR}`)
    // Legal intro framing: control, review gate, no AI-lawyer/case-law claim.
    expect(markup).toContain('Stay in expert review mode')
    expect(markup).toContain('Not an AI lawyer, not case-law research.')
    // Every stat figure carries its primary-source citation label (the openable
    // link href is a DOM prop; the lightweight walker serializes the citation
    // text, which is what proves the figure traces to a primary source).
    expect(markup).toContain('69%')
    expect(markup).toContain('8am 2026 Legal Industry Report (n=1,395)')
    expect(markup).toContain('ABA Formal Opinion 512 (July 29, 2024)')
    // No scarcity, projection, or unproven outcome numbers on this page.
    expect(markup).not.toMatch(/N of \d|\b3×\b|\$\d+k|80% faster/i)
  })

  test('every legal stat carries a non-empty figure, label, and a primary-source citation with a real URL', () => {
    expect(LEGAL_VERIFIED_STATS.length).toBeGreaterThan(0)
    LEGAL_VERIFIED_STATS.forEach(stat => {
      expect(stat.value.trim()).not.toBe('')
      expect(stat.label.trim()).not.toBe('')
      expect(stat.source.trim()).not.toBe('')
      // The citation must be an openable primary source (8am report or ABA PDF).
      expect(stat.sourceUrl).toMatch(/^https:\/\//)
      expect(stat.sourceUrl).toMatch(/8am\.com|americanbar\.org/)
    })
  })

  test('the generic /autopilot path shows NO legal-only content', () => {
    const markup = renderHtml(
      overlayView(initFlowModel(Option.none()), stubActions),
    )
    expect(markup).not.toContain(`data-${HUD_LEGAL_OVERLAY_ATTR}`)
    expect(markup).not.toContain(`data-${HUD_LEGAL_VSL_ATTR}`)
    expect(markup).not.toContain(`data-${HUD_LEGAL_STAT_STRIP_ATTR}`)
    expect(markup).not.toContain('ABA Formal Opinion 512')
    expect(markup).not.toContain('Stay in expert review mode')
  })
})
