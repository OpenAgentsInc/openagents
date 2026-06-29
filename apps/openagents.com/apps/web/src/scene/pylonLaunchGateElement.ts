import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import { currentUnixMs } from '../time-format'
import {
  liveCopyInstructionsTagName,
  registerLiveCopyInstructionsElement,
} from './liveCopyInstructionsElement'
import {
  type PylonCountdownHandle,
  isPylonLaunchDeadlinePassed,
  mountPylonCountdown,
  pylonLaunchDeadlineMs,
} from './pylonCountdown'

export const pylonLaunchGateTagName = 'oa-pylon-launch-gate'

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
  position: absolute;
  inset: 0;
  pointer-events: none;
}
.launch-layer {
  position: absolute;
  inset: 0;
  transition:
    opacity 900ms ease,
    transform 900ms ease;
}
.countdown-layer {
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 1;
  transform: translateY(0);
}
.copy-layer {
  opacity: 0;
  pointer-events: none;
  transform: translateY(0.25rem);
}
:host([data-state='launched']) .countdown-layer {
  opacity: 0;
  transform: translateY(-0.25rem);
}
:host([data-state='launched']) .copy-layer {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}
.countdown {
  color: #ffffff;
  font-family: 'Inter', ui-sans-serif, system-ui, sans-serif;
  font-size: clamp(2.5rem, 12vw, 7rem);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-shadow: 0 2px 24px rgba(0, 0, 0, 0.55);
}
`

const pylonLaunchGateElement = defineCustomElement({
  events: {},
  properties: {},
  tag: pylonLaunchGateTagName,
})

const makePylonLaunchGateElement = (): CustomElementConstructor =>
  class PylonLaunchGateElement extends HTMLElement {
    #handle: PylonCountdownHandle | null = null
    #mounted = false

    connectedCallback(): void {
      if (this.#mounted) return
      this.#mounted = true

      registerLiveCopyInstructionsElement()

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = `${hostCss}\n${slotTextCss}`

      const countdownLayer = document.createElement('div')
      countdownLayer.className = 'launch-layer countdown-layer'

      const countdownTarget = document.createElement('div')
      countdownTarget.className = 'countdown'
      countdownLayer.append(countdownTarget)

      const copyLayer = document.createElement('div')
      copyLayer.className = 'launch-layer copy-layer'
      copyLayer.setAttribute('aria-hidden', 'true')
      copyLayer.append(document.createElement(liveCopyInstructionsTagName))

      shadow.append(style, countdownLayer, copyLayer)

      const showCopyInstructions = (): void => {
        // Post-launch root visits should bypass the timer and show the
        // instructions control only. Remove this handoff branch after the
        // June 15, 2026 launch page is replaced with the permanent live state.
        this.setAttribute('data-state', 'launched')
        countdownLayer.setAttribute('aria-hidden', 'true')
        copyLayer.removeAttribute('aria-hidden')
      }

      if (isPylonLaunchDeadlinePassed(currentUnixMs())) {
        showCopyInstructions()
        return
      }

      this.#handle = mountPylonCountdown(countdownTarget, {
        deadlineMs: pylonLaunchDeadlineMs(),
        onComplete: showCopyInstructions,
      })
    }

    disconnectedCallback(): void {
      this.#mounted = false
      if (this.#handle !== null) {
        this.#handle.dispose()
        this.#handle = null
      }
    }
  }

export const registerPylonLaunchGateElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(pylonLaunchGateTagName) !== undefined) return
  customElements.define(pylonLaunchGateTagName, makePylonLaunchGateElement())
}

export const pylonLaunchGateView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPylonLaunchGateElement()
  const element = pylonLaunchGateElement.withMessage<Message>()
  return element(attributes, [])
}
