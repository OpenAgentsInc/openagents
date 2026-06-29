import { Schema as S } from "effect"
import { define as defineCustomElement } from "foldkit/customElement"
import type { Attribute, Html } from "foldkit/html"

import {
  mountPylonDiamonds,
  type PylonDiamondsHandle,
} from "../../../openagents.com/apps/web/src/scene/pylonDiamonds.js"

// #5049: desktop binding for the exact homepage pylon-diamond shader. The
// renderer source remains in the web app; desktop only provides a Foldkit
// custom element that forwards live activity into PylonDiamondsHandle.setActivity.

export const pylonDiamondsTagName = "oa-desktop-pylon-diamonds"

const pylonDiamondsElement = defineCustomElement({
  events: {},
  properties: {
    activity: S.Unknown,
  },
  tag: pylonDiamondsTagName,
})

const clampActivity = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.min(1, value))
    : 0

const makePylonDiamondsElement = (): CustomElementConstructor =>
  class AutopilotPylonDiamondsElement extends HTMLElement {
    #activity = 0
    #handle: PylonDiamondsHandle | null = null

    get activity(): number {
      return this.#activity
    }

    set activity(value: unknown) {
      const next = clampActivity(value)
      if (next === this.#activity) return
      this.#activity = next
      this.#handle?.setActivity(next)
    }

    connectedCallback(): void {
      if (this.#handle !== null) return

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: "open" })
      shadow.replaceChildren()

      const style = document.createElement("style")
      style.textContent = `
        :host {
          display: block;
          overflow: hidden;
          pointer-events: none;
        }
        .mount {
          width: 100%;
          height: 100%;
          min-height: inherit;
        }
      `

      const mount = document.createElement("div")
      mount.className = "mount"
      shadow.append(style, mount)

      this.#handle = mountPylonDiamonds(mount, {
        pixelRatio: 2,
        transparentBackground: true,
      })
      this.#handle.setActivity(this.#activity)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerPylonDiamondsElement = (): void => {
  if (typeof customElements === "undefined") return
  if (typeof HTMLElement === "undefined") return
  if (customElements.get(pylonDiamondsTagName) !== undefined) return
  customElements.define(pylonDiamondsTagName, makePylonDiamondsElement())
}

export const pylonDiamondsView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
  activity = 0,
): Html => {
  registerPylonDiamondsElement()
  const element = pylonDiamondsElement.withMessage<Message>()
  return element(
    [...attributes, element.Activity(clampActivity(activity))],
    [],
  )
}
