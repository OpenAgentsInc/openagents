import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import { type LightBeamsHandle, mountLightBeams } from './lightBeams'

// Foldkit binding for the standalone light-beams background (the white diagonal
// beams from the homepage hero, beams-only + transparent). Drops behind any
// relatively-positioned surface — e.g. the login screen.

export const lightBeamsTagName = 'oa-light-beams'

const lightBeamsElement = defineCustomElement({
  events: {},
  properties: {},
  tag: lightBeamsTagName,
})

const makeLightBeamsElement = (): CustomElementConstructor =>
  class LightBeamsElement extends HTMLElement {
    #handle: LightBeamsHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = `
        :host { position: absolute; inset: 0; display: block; pointer-events: none; }
        .mount { width: 100%; height: 100%; }
      `
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      this.#handle = mountLightBeams(mount)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerLightBeamsElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(lightBeamsTagName) !== undefined) return
  customElements.define(lightBeamsTagName, makeLightBeamsElement())
}

export const lightBeamsView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerLightBeamsElement()
  const element = lightBeamsElement.withMessage<Message>()
  return element(attributes, [])
}
