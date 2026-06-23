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
  initFlowModel,
  isQuoteReady,
} from './flow'
import {
  HUD_COMPONENT_ITEM_ATTR,
  HUD_COMPOSER_ATTR,
  HUD_ROOT_ATTR,
  overlayView,
} from './page'

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
  test('surfaces intake_progress from the first render, nothing else yet', () => {
    const frames = deriveComponentFrames(initFlowModel(Option.none()))
    expect(frames).toHaveLength(1)
    expect(frames[0]).toMatchObject({
      _tag: 'CatalogComponentFrame',
      frame: { component: 'intake_progress' },
    })
  })

  test('legal vertical adds a consent_gate', () => {
    const frames = deriveComponentFrames(initFlowModel(Option.some('legal')))
    const components = frames.map(frame =>
      frame._tag === 'CatalogComponentFrame' ? frame.frame.component : 'unknown',
    )
    expect(components).toEqual(['intake_progress', 'consent_gate'])
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
      'intake_progress',
      'quick_win_card',
      'dashboard_preview',
      'credit_kickoff',
    ])
  })
})

describe('autopilot onboarding — page overlay rendering', () => {
  test('renders the HUD shell, composer, and intake register on first paint', () => {
    const markup = renderHtml(
      overlayView(initFlowModel(Option.none()), stubActions),
    )
    expect(markup).toContain(`data-${HUD_ROOT_ATTR}`)
    expect(markup).toContain(`data-${HUD_COMPOSER_ATTR}`)
    expect(markup).toContain('Onboarding progress')
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

  test('submitting a turn appends the user turn, clears the draft, and fires the program command', () => {
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
    expect(commands.map(command => command.name)).toEqual([
      'SubmitAutopilotOnboardingTurn',
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
    // The verticalOverlay is threaded to the program command on the first turn.
    expect(flowOf(submitted).vertical).toBe('legal')

    // Simulate the command's failure terminal.
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
