import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  mountPylonCountdown,
  type PylonCountdownHandle,
} from './pylonCountdown'

// Centered 12-hour countdown overlay for the /pylon page. Lives in its own
// shadow root so the slot-text roll CSS is scoped here and does not leak into
// the rest of the app.

export const pylonCountdownTagName = 'oa-pylon-countdown'

// Vanilla slot-text structural CSS (slot-text@0.3.1 style.css), inlined so the
// roll animation works inside this element's shadow root.
const slotTextCss = `
.slot-text {
  display: inline-flex;
  white-space: pre;
}
.char-slot {
  position: relative;
  display: inline-flex;
  flex: none;
  justify-content: center;
  overflow: hidden;
  overflow-x: visible;
  overflow-y: clip;
  line-height: 1.3;
  vertical-align: bottom;
}
.char-slot.is-resizing {
  overflow-x: clip;
}
.char-sizer {
  visibility: hidden;
  white-space: pre;
}
.char-face {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  white-space: pre;
  will-change: transform;
}
`

const hostCss = `
:host {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
.countdown {
  font-family: 'Inter', ui-sans-serif, system-ui, sans-serif;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  font-size: clamp(2.5rem, 12vw, 7rem);
  letter-spacing: 0.04em;
  color: #ffffff;
  text-shadow: 0 2px 24px rgba(0, 0, 0, 0.55);
}
`

const pylonCountdownElement = defineCustomElement({
  events: {},
  properties: {},
  tag: pylonCountdownTagName,
})

const makePylonCountdownElement = (): CustomElementConstructor =>
  class PylonCountdownElement extends HTMLElement {
    #handle: PylonCountdownHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()

      const style = document.createElement('style')
      style.textContent = `${hostCss}\n${slotTextCss}`

      const target = document.createElement('div')
      target.className = 'countdown'

      shadow.append(style, target)

      this.#handle = mountPylonCountdown(target)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerPylonCountdownElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(pylonCountdownTagName) !== undefined) return
  customElements.define(pylonCountdownTagName, makePylonCountdownElement())
}

export const pylonCountdownView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPylonCountdownElement()
  const element = pylonCountdownElement.withMessage<Message>()
  return element(attributes, [])
}
