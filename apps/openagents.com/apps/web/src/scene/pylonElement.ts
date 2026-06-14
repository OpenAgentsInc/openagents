import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import { mountPylonDiamonds, type PylonDiamondsHandle } from './pylonDiamonds'

// Foldkit binding for the isolated Pylon diamond scene. Mirrors the pattern
// the shared three-effect package uses for its scene elements, but the scene
// itself lives entirely in this app (see ./pylonDiamonds).

export const pylonTagName = 'oa-pylon'

const pylonElement = defineCustomElement({
  events: {},
  properties: {},
  tag: pylonTagName,
})

const makePylonElement = (): CustomElementConstructor =>
  class PylonElement extends HTMLElement {
    #handle: PylonDiamondsHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()

      const style = document.createElement('style')
      style.textContent = `
        :host {
          display: block;
          min-height: 100dvh;
          overflow: hidden;
          background: #0c0f13;
        }
        .mount {
          width: 100%;
          height: 100%;
          min-height: inherit;
        }
      `

      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)

      this.#handle = mountPylonDiamonds(mount)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerPylonElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(pylonTagName) !== undefined) return
  customElements.define(pylonTagName, makePylonElement())
}

export const pylonView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPylonElement()
  const element = pylonElement.withMessage<Message>()
  return element(attributes, [])
}
