import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  type LandingSquaresHandle,
  mountLandingSquares,
} from './landingSquares'

// Foldkit binding for the standalone landing-page background: a few white
// squares drifting and pulsing subtly on black. Full-bleed and pointer-inert,
// it fills the viewport behind the (intentionally empty) landing surface.

export const landingSquaresTagName = 'oa-landing-squares'

const landingSquaresElement = defineCustomElement({
  events: {},
  properties: {},
  tag: landingSquaresTagName,
})

const makeLandingSquaresElement = (): CustomElementConstructor =>
  class LandingSquaresElement extends HTMLElement {
    #handle: LandingSquaresHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = `
        :host { position: absolute; inset: 0; display: block; pointer-events: none; background: #000; }
        .mount { width: 100%; height: 100%; }
      `
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      this.#handle = mountLandingSquares(mount)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerLandingSquaresElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(landingSquaresTagName) !== undefined) return
  customElements.define(landingSquaresTagName, makeLandingSquaresElement())
}

export const landingSquaresView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerLandingSquaresElement()
  const element = landingSquaresElement.withMessage<Message>()
  return element(attributes, [])
}
