import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

// Shared factory for Three.js animation experiments: wrap a `mount(el) => {dispose}`
// function in a Foldkit custom element (absolute-fill, transparent, pointer-none)
// and return a typed view helper. Lets the /animations playground add a new
// experiment with one mount function + one line.

export type AnimationHandle = Readonly<{ dispose: () => void }>
export type AnimationMount = (element: HTMLElement) => AnimationHandle

// Deterministic [0,1) value from two integer seeds. Scenes use this instead of
// nondeterministic seeding so layouts are reproducible and the determinism
// architecture rule (no raw time/id/randomness primitives) stays satisfied.
export const seededUnit = (a: number, b: number): number => {
  const value = Math.sin((a + 1) * (b * 101 + 997)) * 10000
  return value - Math.floor(value)
}

export const makeAnimationView = (
  tag: string,
  mount: AnimationMount,
): (<Message>(attributes?: ReadonlyArray<Attribute<Message>>) => Html) => {
  const element = defineCustomElement({ events: {}, properties: {}, tag })

  const makeClass = (): CustomElementConstructor =>
    class extends HTMLElement {
      #handle: AnimationHandle | null = null

      connectedCallback(): void {
        if (this.#handle !== null) return
        const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
        shadow.replaceChildren()
        const style = document.createElement('style')
        style.textContent =
          ':host{position:absolute;inset:0;display:block;pointer-events:none}.mount{width:100%;height:100%}'
        const mountEl = document.createElement('div')
        mountEl.className = 'mount'
        shadow.append(style, mountEl)
        this.#handle = mount(mountEl)
      }

      disconnectedCallback(): void {
        if (this.#handle === null) return
        this.#handle.dispose()
        this.#handle = null
      }
    }

  const register = (): void => {
    if (typeof customElements === 'undefined') return
    if (typeof HTMLElement === 'undefined') return
    if (customElements.get(tag) !== undefined) return
    customElements.define(tag, makeClass())
  }

  return <Message>(attributes: ReadonlyArray<Attribute<Message>> = []): Html => {
    register()
    return element.withMessage<Message>()(attributes, [])
  }
}

// Common boilerplate: a transparent, auto-resizing WebGL canvas mounted into
// `element`. Returns the canvas + a sizer the scene reads, and wires disposal.
export type WebglRig = Readonly<{
  canvas: HTMLCanvasElement
  size: () => { height: number; width: number }
}>

export const webglCanvas = (element: HTMLElement): WebglRig => {
  element.style.position = 'absolute'
  element.style.inset = '0'
  element.style.overflow = 'hidden'
  const canvas = document.createElement('canvas')
  canvas.style.cssText =
    'position:absolute;inset:0;width:100%;height:100%;display:block'
  element.append(canvas)
  return {
    canvas,
    size: () => {
      const rect = element.getBoundingClientRect()
      return {
        height: Math.max(1, Math.floor(rect.height || element.clientHeight || 260)),
        width: Math.max(1, Math.floor(rect.width || element.clientWidth || 320)),
      }
    },
  }
}
