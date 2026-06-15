import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

export const liveCopyInstructionsTagName = 'oa-live-copy-instructions'

const AGENT_INSTRUCTIONS_URL = '/AGENTS.md'

const hostCss = `
:host {
  position: absolute;
  inset: 0;
  pointer-events: none;
}
.copy-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.copy-stack {
  display: grid;
  gap: 0.75rem;
  justify-items: center;
}
.copy-button {
  pointer-events: auto;
  min-height: 2.75rem;
  border: 1px solid #d6f6ff;
  background: rgba(1, 1, 2, 0.86);
  color: #f1efe8;
  cursor: pointer;
  font-family: ui-monospace, 'Berkeley Mono', monospace;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  padding: 0.8rem 1rem;
  text-transform: uppercase;
  box-shadow: 0 0 28px rgba(41, 121, 255, 0.22);
}
.copy-button:hover {
  background: rgba(12, 15, 19, 0.94);
  border-color: #ffffff;
}
.copy-button:focus-visible {
  outline: 2px solid #2979ff;
  outline-offset: 3px;
}
.copy-button[aria-busy='true'] {
  cursor: wait;
}
.copy-status {
  min-height: 1rem;
  color: rgba(255, 255, 255, 0.58);
  font-family: ui-monospace, 'Berkeley Mono', monospace;
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  text-shadow: 0 2px 18px rgba(0, 0, 0, 0.6);
}
`

const liveCopyInstructionsElement = defineCustomElement({
  events: {},
  properties: {},
  tag: liveCopyInstructionsTagName,
})

const makeLiveCopyInstructionsElement = (): CustomElementConstructor =>
  class LiveCopyInstructionsElement extends HTMLElement {
    #resetTimer: ReturnType<typeof setTimeout> | null = null

    connectedCallback(): void {
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()

      const style = document.createElement('style')
      style.textContent = hostCss

      const overlay = document.createElement('div')
      overlay.className = 'copy-overlay'

      const stack = document.createElement('div')
      stack.className = 'copy-stack'

      const button = document.createElement('button')
      button.className = 'copy-button'
      button.type = 'button'
      button.textContent = 'Copy Agent Instructions'

      const status = document.createElement('div')
      status.className = 'copy-status'
      status.setAttribute('aria-live', 'polite')

      button.addEventListener('click', () => {
        void this.#copyInstructions(button, status)
      })

      stack.append(button, status)
      overlay.append(stack)
      shadow.append(style, overlay)
    }

    disconnectedCallback(): void {
      if (this.#resetTimer === null) return
      clearTimeout(this.#resetTimer)
      this.#resetTimer = null
    }

    async #copyInstructions(
      button: HTMLButtonElement,
      status: HTMLElement,
    ): Promise<void> {
      if (button.getAttribute('aria-busy') === 'true') return

      if (this.#resetTimer !== null) {
        clearTimeout(this.#resetTimer)
        this.#resetTimer = null
      }

      button.setAttribute('aria-busy', 'true')
      button.textContent = 'Copying...'
      status.textContent = ''

      try {
        const response = await fetch(AGENT_INSTRUCTIONS_URL, {
          cache: 'no-store',
          headers: { accept: 'text/markdown,text/plain,*/*' },
        })

        if (!response.ok) {
          throw new Error(`AGENTS.md returned HTTP ${response.status}`)
        }

        await navigator.clipboard.writeText(await response.text())
        button.textContent = 'Copied'
        status.textContent = 'Copied from openagents.com/AGENTS.md'
      } catch {
        button.textContent = 'Copy failed'
        status.textContent = 'Open /AGENTS.md'
      } finally {
        button.setAttribute('aria-busy', 'false')
        this.#resetTimer = setTimeout(() => {
          button.textContent = 'Copy Agent Instructions'
          status.textContent = ''
          this.#resetTimer = null
        }, 3000)
      }
    }
  }

export const registerLiveCopyInstructionsElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(liveCopyInstructionsTagName) !== undefined) return
  customElements.define(
    liveCopyInstructionsTagName,
    makeLiveCopyInstructionsElement(),
  )
}

export const liveCopyInstructionsView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerLiveCopyInstructionsElement()
  const element = liveCopyInstructionsElement.withMessage<Message>()
  return element(attributes, [])
}
