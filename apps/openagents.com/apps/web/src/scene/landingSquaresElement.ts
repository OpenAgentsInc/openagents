import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  type LandingPose,
  type LandingSquaresHandle,
  mountLandingSquares,
} from './landingSquares'

// Foldkit binding for the standalone landing-page background: a 3D pylon
// constellation (HDR-emissive cores + energy lines + sparks through a bloom
// chain) on near-black. Full-bleed and pointer-inert, it fills the viewport
// behind the centred landing wordmark.
//
// The element is mounted ONCE and persists across the / <-> /khala route
// change (same keyed node). The active route is passed as `data-pose`; on change
// the element eases the camera to that pose, so navigation is a continuous flight
// through the same scene rather than a page cut.

export const landingSquaresTagName = 'oa-landing-squares'

const POSE_ATTRIBUTE = 'data-pose'

const parsePose = (value: string | null): LandingPose =>
  value === 'khala'
    ? 'khala'
    : value === 'tassadar'
      ? 'tassadar'
      : value === 'autopilot'
        ? 'autopilot'
        : value === 'login'
          ? 'login'
          : 'landing'

const landingSquaresElement = defineCustomElement({
  events: {},
  properties: {},
  tag: landingSquaresTagName,
})

const makeLandingSquaresElement = (): CustomElementConstructor =>
  class LandingSquaresElement extends HTMLElement {
    #handle: LandingSquaresHandle | null = null

    static get observedAttributes(): ReadonlyArray<string> {
      return [POSE_ATTRIBUTE]
    }

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
      this.#handle = mountLandingSquares(mount, {
        pose: parsePose(this.getAttribute(POSE_ATTRIBUTE)),
      })
    }

    attributeChangedCallback(
      name: string,
      _previous: string | null,
      next: string | null,
    ): void {
      if (name !== POSE_ATTRIBUTE) return
      this.#handle?.setPose(parsePose(next))
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
