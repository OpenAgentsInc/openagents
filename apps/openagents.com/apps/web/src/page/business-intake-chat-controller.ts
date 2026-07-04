// Browser controller for the /business Khala intake console.
//
// Installed once at app boot (entry.ts), decoupled from the Foldkit loop the
// same way as the tokens-served count-up controller: it watches the document
// for the console root (`[data-business-intake-chat]`) and wires the composer,
// transcript, and the bounded server-side interview endpoint when the page
// renders it. The pure state machine lives in `business-intake-chat.ts`.
//
// Honesty + safety posture:
// - Server calls are strictly user-initiated (no completion is spent on page
//   load); the empty state is static copy.
// - When the interview completes, the drafted spec is written into the plain
//   signup form's `helpWith` textarea — the form remains the single submit
//   authority, so no-JS visitors and chat visitors converge on one receipt
//   path.
// - Reduced-motion-safe: scrolling snaps when the visitor prefers reduced
//   motion; nothing else animates.

import {
  BUSINESS_INTAKE_CHAT_ENDPOINT,
  type IntakeChatComponentFrame,
  type IntakeChatState,
  appendUserMessage,
  applyIntakeChatFailure,
  applyIntakeChatReply,
  canSendIntakeMessage,
  decodeIntakeChatReply,
  initialIntakeChatState,
  intakeChatStatusLine,
  recoverIntakeChat,
} from './business-intake-chat'

const ROOT_SELECTOR = '[data-business-intake-chat]'
const WIRED_FLAG = 'intakeChatWired'

type FetchLike = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, 'ok' | 'status' | 'json'>>

const prefersReducedMotion = (): boolean => {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return true
  }
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

const messageRow = (
  doc: Document,
  role: 'user' | 'assistant',
  content: string,
): HTMLElement => {
  const row = doc.createElement('div')
  row.setAttribute('data-intake-chat-row', role)
  row.className = 'grid grid-cols-[3.5rem_minmax(0,1fr)] gap-2'

  const prefix = doc.createElement('span')
  prefix.textContent = role === 'assistant' ? 'khala' : 'you'
  prefix.className =
    role === 'assistant'
      ? 'select-none text-[#ffb400]'
      : 'select-none text-white/35'

  const body = doc.createElement('p')
  body.textContent = content
  body.className =
    role === 'assistant'
      ? 'm-0 max-w-[62ch] whitespace-pre-wrap text-[#f1efe8]'
      : 'm-0 max-w-[62ch] whitespace-pre-wrap text-white/70'

  row.append(prefix, body)
  return row
}

const stringProp = (
  props: Readonly<Record<string, unknown>>,
  key: string,
): string | null => {
  const value = props[key]
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : null
}

const componentSummary = (frame: IntakeChatComponentFrame): string => {
  switch (frame.component) {
    case 'intake_progress': {
      const steps = Array.isArray(frame.props['steps'])
        ? frame.props['steps'].filter(
            (step): step is string => typeof step === 'string',
          )
        : []
      const current =
        typeof frame.props['current'] === 'number'
          ? frame.props['current']
          : 0
      const active = steps[current] ?? steps[0] ?? 'Intake'
      return `Progress: ${active}`
    }
    case 'quick_win_card':
      return `Quick win: ${stringProp(frame.props, 'title') ?? 'Scoped task'}`
    case 'consent_gate':
      return `Consent: ${stringProp(frame.props, 'scope') ?? 'Review required'}`
    case 'human_handoff':
      return `Handoff: ${stringProp(frame.props, 'reason') ?? 'Human review'}`
    case 'credit_kickoff':
      return stringProp(frame.props, 'label') ?? 'Credit kickoff'
    case 'dashboard_preview':
      return `Workspace: ${stringProp(frame.props, 'workspaceRef') ?? 'Preview'}`
    default:
      return frame.component
  }
}

const componentRow = (
  doc: Document,
  frame: IntakeChatComponentFrame,
): HTMLElement => {
  const row = doc.createElement('div')
  row.setAttribute('data-intake-chat-component', frame.component)
  row.className =
    'ml-[4.5rem] grid max-w-[62ch] gap-1 border border-[#333] bg-[#080808] px-3 py-2 font-mono text-xs'

  const title = doc.createElement('p')
  title.className = 'm-0 text-[#ffb400]'
  title.textContent = componentSummary(frame)

  const detail = doc.createElement('p')
  detail.className = 'm-0 text-white/45'
  detail.textContent = `component:${frame.component}`

  row.append(title, detail)
  return row
}

const sourceRefFromDocument = (root: HTMLElement): string => {
  const doc = root.ownerDocument
  const formValue = doc
    .querySelector<HTMLInputElement>('input[name="sourceRef"]')
    ?.value.trim()
  if (formValue !== undefined && formValue !== '') {
    return formValue
  }
  const rootValue = root.dataset['businessSourceRef']?.trim()
  return rootValue === undefined || rootValue === '' ? 'direct' : rootValue
}

const wireConsole = (root: HTMLElement, fetchLike: FetchLike): void => {
  const doc = root.ownerDocument
  const transcript = root.querySelector<HTMLElement>(
    '[data-intake-chat-transcript]',
  )
  const input = root.querySelector<HTMLTextAreaElement>(
    '[data-intake-chat-input]',
  )
  const send = root.querySelector<HTMLButtonElement>('[data-intake-chat-send]')
  const status = root.querySelector<HTMLElement>('[data-intake-chat-status]')
  if (transcript === null || input === null || send === null) {
    return
  }

  let state: IntakeChatState = initialIntakeChatState

  const render = (): void => {
    if (status !== null) {
      status.textContent = intakeChatStatusLine(state)
      status.setAttribute('data-intake-chat-status', state.phase)
    }
    const busy = state.phase === 'waiting' || state.phase === 'done'
    send.disabled = busy
    input.disabled = state.phase === 'done'
  }

  const scrollToLatest = (): void => {
    transcript.scrollTo({
      top: transcript.scrollHeight,
      behavior: prefersReducedMotion() ? 'auto' : 'smooth',
    })
  }

  const appendRow = (role: 'user' | 'assistant', content: string): void => {
    const empty = transcript.querySelector('[data-intake-chat-empty]')
    if (empty !== null) {
      empty.remove()
    }
    transcript.append(messageRow(doc, role, content))
    scrollToLatest()
  }

  const appendComponent = (frame: IntakeChatComponentFrame): void => {
    const empty = transcript.querySelector('[data-intake-chat-empty]')
    if (empty !== null) {
      empty.remove()
    }
    transcript.append(componentRow(doc, frame))
    scrollToLatest()
  }

  // Terminal handoff: write the drafted spec into the plain form and walk the
  // visitor to it. The form stays the single submit authority.
  const handOffSpec = (spec: string): void => {
    const helpWith = doc.querySelector<HTMLTextAreaElement>(
      'textarea[name="helpWith"]',
    )
    if (helpWith !== null) {
      helpWith.value = spec
      helpWith.dispatchEvent(new Event('input', { bubbles: true }))
    }
    const specObject = doc.querySelector<HTMLInputElement>(
      'input[name="intakeSpecObject"]',
    )
    if (specObject !== null && state.specObject !== null) {
      specObject.value = JSON.stringify(state.specObject)
      specObject.dispatchEvent(new Event('input', { bubbles: true }))
    }
    const sourceRef = doc.querySelector<HTMLInputElement>(
      'input[name="sourceRef"]',
    )
    if (sourceRef !== null) {
      sourceRef.value = sourceRefFromDocument(root)
      sourceRef.dispatchEvent(new Event('input', { bubbles: true }))
    }
    const form = doc.getElementById('business-signup')
    if (form !== null && typeof form.scrollIntoView === 'function') {
      form.scrollIntoView({
        behavior: prefersReducedMotion() ? 'auto' : 'smooth',
        block: 'start',
      })
    }
  }

  const sendMessage = async (): Promise<void> => {
    const text = input.value
    if (!canSendIntakeMessage(state, text)) {
      return
    }
    state = appendUserMessage(state, text)
    appendRow('user', text.trim())
    input.value = ''
    render()

    try {
      const response = await fetchLike(BUSINESS_INTAKE_CHAT_ENDPOINT, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          messages: state.messages,
          sourceRef: sourceRefFromDocument(root),
        }),
      })
      if (!response.ok) {
        state = applyIntakeChatFailure(state, response.status)
        render()
        return
      }
      const payload: unknown = await response.json().catch(() => null)
      const reply = decodeIntakeChatReply(payload)
      if (reply === null) {
        state = applyIntakeChatFailure(state, 'network')
        render()
        return
      }
      state = applyIntakeChatReply(state, reply)
      appendRow('assistant', reply.reply)
      if (reply.component !== null) {
        appendComponent(reply.component)
      }
      render()
      if (state.phase === 'done' && state.spec !== null) {
        handOffSpec(state.spec)
      }
    } catch {
      state = applyIntakeChatFailure(state, 'network')
      render()
    }
  }

  send.addEventListener('click', () => {
    void sendMessage()
  })
  input.addEventListener('keydown', event => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      void sendMessage()
    }
  })
  input.addEventListener('input', () => {
    // A retryable failure recovers as soon as the visitor starts typing again.
    const recovered = recoverIntakeChat(state)
    if (recovered !== state) {
      state = recovered
      render()
    }
  })

  render()
}

// Install once. Watches for the console root across client-side navigations;
// a no-op outside a real browser document.
export const installBusinessIntakeChatController = (
  doc: Document | undefined = typeof document === 'undefined'
    ? undefined
    : document,
  fetchLike: FetchLike | undefined = typeof fetch === 'undefined'
    ? undefined
    : (input, init) => fetch(input, init),
): (() => void) => {
  if (doc === undefined || fetchLike === undefined) {
    return () => {}
  }

  const wireAll = (): void => {
    for (const root of doc.querySelectorAll<HTMLElement>(ROOT_SELECTOR)) {
      if (root.dataset[WIRED_FLAG] === 'true') {
        continue
      }
      root.dataset[WIRED_FLAG] = 'true'
      wireConsole(root, fetchLike)
    }
  }

  wireAll()
  const observer = new MutationObserver(wireAll)
  observer.observe(doc.documentElement, { childList: true, subtree: true })
  return () => {
    observer.disconnect()
  }
}
