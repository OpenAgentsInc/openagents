import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

// Client behavior for the `@openagentsinc/ui` code-block copy button.
//
// The library `codeBlock` renders a static `[data-oa-code-copy]` button and a
// hidden, byte-faithful `[data-oa-code-source]` element per `[data-oa-code-block]`.
// Wrapping that markup in `oa-code-copy-scope` activates the buttons: a single
// delegated click handler on the host copies the pristine source text of the
// clicked block and flips the button to a "Copied" state for 2s. Keeping the
// behavior here (not in the library) keeps the component pure, SSR-safe, and
// reusable across surfaces. Precedent: `scene/liveCopyInstructionsElement.ts`.

export const codeCopyScopeTagName = 'oa-code-copy-scope'

const COPY_RESET_MS = 2000

const codeCopyScopeElement = defineCustomElement({
  events: {},
  properties: {},
  tag: codeCopyScopeTagName,
})

const setLabel = (button: HTMLElement, text: string): void => {
  const label = button.querySelector<HTMLElement>('[data-oa-code-copy-label]')
  if (label !== null) {
    label.textContent = text
  }
}

// Swap the copy/check glyphs purely in JS so we never depend on a Tailwind
// arbitrary `data-[]`/named-group variant compiling.
const showCheck = (button: HTMLElement, on: boolean): void => {
  const copyIcon = button.querySelector<HTMLElement>(
    '[data-oa-code-copy-icon="copy"]',
  )
  const checkIcon = button.querySelector<HTMLElement>(
    '[data-oa-code-copy-icon="check"]',
  )
  copyIcon?.classList.toggle('hidden', on)
  checkIcon?.classList.toggle('hidden', !on)
}

const makeCodeCopyScopeElement = (): CustomElementConstructor =>
  class CodeCopyScopeElement extends HTMLElement {
    #timers = new WeakMap<HTMLElement, ReturnType<typeof setTimeout>>()

    #onClick = (event: Event): void => {
      const target = event.target
      if (!(target instanceof Element)) return

      const button = target.closest<HTMLElement>('[data-oa-code-copy]')
      if (button === null) return

      const block = button.closest('[data-oa-code-block]')
      const source = block?.querySelector('[data-oa-code-source]')
      const text = source?.textContent ?? ''
      void this.#copy(button, text)
    }

    connectedCallback(): void {
      this.addEventListener('click', this.#onClick)
    }

    disconnectedCallback(): void {
      this.removeEventListener('click', this.#onClick)
    }

    async #copy(button: HTMLElement, text: string): Promise<void> {
      const existing = this.#timers.get(button)
      if (existing !== undefined) {
        clearTimeout(existing)
      }

      let copied = false
      try {
        if (
          typeof navigator !== 'undefined' &&
          navigator.clipboard?.writeText !== undefined
        ) {
          await navigator.clipboard.writeText(text)
          copied = true
        }
      } catch {
        copied = false
      }

      if (copied) {
        button.dataset.copied = 'true'
        showCheck(button, true)
        setLabel(button, 'Copied')
      } else {
        delete button.dataset.copied
        showCheck(button, false)
        setLabel(button, 'Copy failed')
      }

      this.#timers.set(
        button,
        setTimeout(() => {
          delete button.dataset.copied
          showCheck(button, false)
          setLabel(button, 'Copy')
          this.#timers.delete(button)
        }, COPY_RESET_MS),
      )
    }
  }

export const registerCodeCopyScopeElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(codeCopyScopeTagName) !== undefined) return
  customElements.define(codeCopyScopeTagName, makeCodeCopyScopeElement())
}

export const codeCopyScopeView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
  children: ReadonlyArray<Html> = [],
): Html => {
  registerCodeCopyScopeElement()
  const element = codeCopyScopeElement.withMessage<Message>()
  return element(attributes, children)
}
