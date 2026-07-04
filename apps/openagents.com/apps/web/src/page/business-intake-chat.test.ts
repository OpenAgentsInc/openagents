import { describe, expect, test, vi } from 'vitest'

import {
  INTAKE_CHAT_MAX_MESSAGES,
  INTAKE_CHAT_MAX_MESSAGE_CHARS,
  appendUserMessage,
  applyIntakeChatFailure,
  applyIntakeChatReply,
  canSendIntakeMessage,
  decodeIntakeChatReply,
  initialIntakeChatState,
  intakeChatStatusLine,
  recoverIntakeChat,
} from './business-intake-chat'
import { installBusinessIntakeChatController } from './business-intake-chat-controller'

describe('business intake chat state core', () => {
  test('sends only trimmed, bounded messages while ready', () => {
    expect(canSendIntakeMessage(initialIntakeChatState, '  ')).toBe(false)
    expect(canSendIntakeMessage(initialIntakeChatState, 'fix our dashboard')).toBe(
      true,
    )
    expect(
      canSendIntakeMessage(
        initialIntakeChatState,
        'x'.repeat(INTAKE_CHAT_MAX_MESSAGE_CHARS + 1),
      ),
    ).toBe(false)
    const waiting = appendUserMessage(initialIntakeChatState, 'hello')
    expect(canSendIntakeMessage(waiting, 'again')).toBe(false)
  })

  test('caps the transcript at the shared message bound', () => {
    let state = initialIntakeChatState
    while (state.messages.length + 2 <= INTAKE_CHAT_MAX_MESSAGES) {
      state = applyIntakeChatReply(appendUserMessage(state, 'q'), {
        component: null,
        reply: 'a',
        done: false,
        spec: null,
        specObject: null,
      })
    }
    expect(state.messages.length).toBe(INTAKE_CHAT_MAX_MESSAGES)
    expect(canSendIntakeMessage(state, 'one more')).toBe(false)
  })

  test('decodes replies strictly and treats done-without-spec as not done', () => {
    expect(decodeIntakeChatReply(null)).toBeNull()
    expect(decodeIntakeChatReply({ ok: false, reply: 'x' })).toBeNull()
    expect(
      decodeIntakeChatReply({ ok: true, reply: 'hi', done: false, spec: null }),
    ).toEqual({
      component: null,
      reply: 'hi',
      done: false,
      spec: null,
      specObject: null,
    })
    expect(
      decodeIntakeChatReply({ ok: true, reply: 'hi', done: true, spec: null }),
    ).toEqual({
      component: null,
      reply: 'hi',
      done: false,
      spec: null,
      specObject: null,
    })
    expect(
      decodeIntakeChatReply({
        ok: true,
        reply: 'hi',
        done: true,
        spec: '# Spec',
        specObject: {
          schemaVersion: 'business_intake_spec.v1',
          vertical: 'legal',
          goals: ['launch intake'],
          pains: ['manual calls'],
          systemsOfRecord: ['CRM'],
        },
      }),
    ).toEqual({
      component: null,
      reply: 'hi',
      done: true,
      spec: '# Spec',
      specObject: {
        schemaVersion: 'business_intake_spec.v1',
        vertical: 'legal',
        goals: ['launch intake'],
        pains: ['manual calls'],
        systemsOfRecord: ['CRM'],
      },
    })
  })

  test('completes into the done phase carrying the drafted spec', () => {
    const waiting = appendUserMessage(initialIntakeChatState, 'build my site')
    const done = applyIntakeChatReply(waiting, {
      component: {
        component: 'quick_win_card',
        id: 'cmp_1',
        props: { etaDays: 3, scope: 'billing page', title: 'Billing page' },
        v: 1,
      },
      reply: 'Here is your spec.',
      done: true,
      spec: '# OpenAgents Business — Customer Intake Spec',
      specObject: {
        schemaVersion: 'business_intake_spec.v1',
        vertical: 'software',
        goals: ['rebuild billing'],
        pains: ['old page'],
        systemsOfRecord: ['repo'],
      },
    })
    expect(done.phase).toBe('done')
    expect(done.spec).toContain('Customer Intake Spec')
    expect(done.specObject?.systemsOfRecord).toEqual(['repo'])
    expect(done.components).toHaveLength(1)
    expect(intakeChatStatusLine(done)).toContain('review and submit')
    // done is terminal — typing does not recover it
    expect(recoverIntakeChat(done)).toBe(done)
  })

  test('maps failures honestly and recovers retryable ones', () => {
    const waiting = appendUserMessage(initialIntakeChatState, 'hello')
    expect(applyIntakeChatFailure(waiting, 429).phase).toBe('rate_limited')
    expect(applyIntakeChatFailure(waiting, 503).phase).toBe('unavailable')
    expect(applyIntakeChatFailure(waiting, 'network').phase).toBe('error')
    const recovered = recoverIntakeChat(applyIntakeChatFailure(waiting, 429))
    expect(recovered.phase).toBe('ready')
  })
})

const consoleDom = (): HTMLElement => {
  const root = document.createElement('section')
  root.setAttribute('data-business-intake-chat', '')
  root.innerHTML = [
    '<span data-intake-chat-status="idle"></span>',
    '<div data-intake-chat-transcript><p data-intake-chat-empty>empty</p></div>',
    '<textarea data-intake-chat-input></textarea>',
    '<button data-intake-chat-send type="button">Send</button>',
  ].join('')
  document.body.appendChild(root)
  return root
}

const flush = async (): Promise<void> => {
  await Promise.resolve()
  await Promise.resolve()
  await new Promise(resolve => setTimeout(resolve, 0))
}

describe('business intake chat controller (DOM)', () => {
  test('wires the console, sends a message, renders the reply', async () => {
    document.body.innerHTML = ''
    const root = consoleDom()
    const sourceRef = document.createElement('input')
    sourceRef.name = 'sourceRef'
    sourceRef.value = 'apollo_agent_readiness_a'
    document.body.appendChild(sourceRef)
    const fetchLike = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        ok: true,
        reply: 'What does your business do?',
        done: false,
        spec: null,
      }),
    })
    const teardown = installBusinessIntakeChatController(document, fetchLike)

    const input = root.querySelector<HTMLTextAreaElement>(
      '[data-intake-chat-input]',
    )
    const send = root.querySelector<HTMLButtonElement>('[data-intake-chat-send]')
    input!.value = 'we need our billing page rebuilt'
    send!.click()
    await flush()

    expect(fetchLike).toHaveBeenCalledTimes(1)
    const [, init] = fetchLike.mock.calls[0] as [string, RequestInit]
    expect(String(init.body)).toContain('billing page rebuilt')
    expect(String(init.body)).toContain('apollo_agent_readiness_a')
    expect(root.querySelectorAll('[data-intake-chat-row]').length).toBe(2)
    expect(root.textContent).toContain('What does your business do?')
    expect(root.querySelector('[data-intake-chat-empty]')).toBeNull()
    teardown()
  })

  test('hands the drafted spec off to the signup form on completion', async () => {
    document.body.innerHTML = ''
    const root = consoleDom()
    const form = document.createElement('form')
    form.id = 'business-signup'
    const helpWith = document.createElement('textarea')
    helpWith.name = 'helpWith'
    const intakeSpecObject = document.createElement('input')
    intakeSpecObject.name = 'intakeSpecObject'
    const sourceRef = document.createElement('input')
    sourceRef.name = 'sourceRef'
    sourceRef.value = 'apollo_agent_readiness_a'
    form.append(helpWith, intakeSpecObject, sourceRef)
    document.body.appendChild(form)

    const fetchLike = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        ok: true,
        reply: 'Done — your spec is drafted.',
        done: true,
        spec: '# OpenAgents Business — Customer Intake Spec\n- Quick win: rebuild billing page',
        specObject: {
          schemaVersion: 'business_intake_spec.v1',
          vertical: 'software',
          goals: ['rebuild billing page'],
          pains: ['checkout is slow'],
          systemsOfRecord: ['repo', 'Stripe'],
        },
        component: {
          v: 1,
          id: 'business_intake_cmp_1',
          component: 'quick_win_card',
          props: {
            title: 'Billing page',
            scope: 'Rebuild the page with review',
            etaDays: 3,
          },
        },
      }),
    })
    installBusinessIntakeChatController(document, fetchLike)

    const input = root.querySelector<HTMLTextAreaElement>(
      '[data-intake-chat-input]',
    )
    input!.value = 'yes, draft it'
    root.querySelector<HTMLButtonElement>('[data-intake-chat-send]')!.click()
    await flush()

    expect(helpWith.value).toContain('Customer Intake Spec')
    expect(intakeSpecObject.value).toContain('business_intake_spec.v1')
    expect(sourceRef.value).toBe('apollo_agent_readiness_a')
    expect(root.textContent).toContain('Quick win: Billing page')
    expect(root.querySelector('[data-intake-chat-component="quick_win_card"]')).not.toBeNull()
    expect(input!.disabled).toBe(true)
    const status = root.querySelector('[data-intake-chat-status]')
    expect(status?.textContent).toContain('review and submit')
  })

  test('renders honest failure states for rate limits and outages', async () => {
    document.body.innerHTML = ''
    const root = consoleDom()
    const fetchLike = vi
      .fn()
      .mockResolvedValue({ ok: false, status: 503, json: async () => ({}) })
    installBusinessIntakeChatController(document, fetchLike)

    const input = root.querySelector<HTMLTextAreaElement>(
      '[data-intake-chat-input]',
    )
    input!.value = 'hello'
    root.querySelector<HTMLButtonElement>('[data-intake-chat-send]')!.click()
    await flush()

    const status = root.querySelector('[data-intake-chat-status]')
    expect(status?.getAttribute('data-intake-chat-status')).toBe('unavailable')
    expect(status?.textContent).toContain('form below works')
  })
})
