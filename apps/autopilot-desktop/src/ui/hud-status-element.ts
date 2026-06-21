import { Schema as S } from "effect"
import { define as defineCustomElement } from "foldkit/customElement"
import type { Attribute, Html } from "foldkit/html"

import {
  hudStatusProjection,
  type HudStatusInput,
  type HudStatusProjection,
} from "../shared/hud-status-projection.js"
import { mountHudStatusScene, type HudStatusSceneHandle } from "./hud-status-scene.js"

// HUD H7 (#5504): the Foldkit custom-element binding for the live status/meters
// HUD overlay. Same pattern as `pylon-diamonds-element.ts` — the (heavy, DOM-
// owning) three-effect scene lives in `hud-status-scene.ts`; this element is the
// thin Foldkit bridge that mounts it once and forwards the live projection in
// via `setProjection`. The webview view passes a public-safe `HudStatusInput`
// (node launch status + node-state projection) as the element property; the
// element derives the projection with the SAME pure `hudStatusProjection` the
// tests exercise, so render and tests never diverge.

export const statusHudTagName = "oa-desktop-status-hud"

const statusHudElement = defineCustomElement({
  events: {},
  properties: {
    input: S.Unknown,
  },
  tag: statusHudTagName,
})

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

// Narrow the unknown property to a HudStatusInput, degrading missing/garbage
// payloads to the honest empty input (null status + null node → "connecting…" +
// unknown meters) rather than throwing.
const inputFromUnknown = (value: unknown): HudStatusInput => {
  if (!isRecord(value)) return { nodeLaunchStatus: null, node: null }
  const status = value.nodeLaunchStatus
  return {
    nodeLaunchStatus: typeof status === "string" ? status : null,
    node: (value.node ?? null) as HudStatusInput["node"],
  }
}

const signatureOf = (projection: HudStatusProjection): string => {
  try {
    return JSON.stringify(projection)
  } catch {
    return `${Date.now()}`
  }
}

const makeStatusHudElement = (): CustomElementConstructor =>
  class AutopilotStatusHudElement extends HTMLElement {
    #handle: HudStatusSceneHandle | null = null
    #mount: HTMLDivElement | null = null
    #projection: HudStatusProjection = hudStatusProjection({
      nodeLaunchStatus: null,
      node: null,
    })
    #signature = signatureOf(this.#projection)

    get input(): HudStatusInput {
      return { nodeLaunchStatus: null, node: null }
    }

    set input(value: unknown) {
      const next = hudStatusProjection(inputFromUnknown(value))
      const signature = signatureOf(next)
      if (signature === this.#signature) return
      this.#projection = next
      this.#signature = signature
      this.#handle?.setProjection(next)
    }

    connectedCallback(): void {
      if (this.#mount !== null) return

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: "open" })
      shadow.replaceChildren()

      const style = document.createElement("style")
      style.textContent = `
        :host {
          display: block;
          width: 220px;
          height: 280px;
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
      this.#mount = mount
      shadow.append(style, mount)

      this.#handle = mountHudStatusScene(mount, this.#projection)
    }

    disconnectedCallback(): void {
      this.#handle?.dispose()
      this.#handle = null
      this.#mount = null
    }
  }

export const registerStatusHudElement = (): void => {
  if (typeof customElements === "undefined") return
  if (typeof HTMLElement === "undefined") return
  if (customElements.get(statusHudTagName) !== undefined) return
  customElements.define(statusHudTagName, makeStatusHudElement())
}

export const statusHudView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
  input: HudStatusInput = { nodeLaunchStatus: null, node: null },
): Html => {
  registerStatusHudElement()
  const element = statusHudElement.withMessage<Message>()
  return element([...attributes, element.Input(input)], [])
}
