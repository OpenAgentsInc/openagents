import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  computeActivityIntensity,
  fetchPylonStats,
  type PylonStatsSnapshot,
} from './pylonNetworkStats'

// #5050: the bezier network graph for the homepage. The central pylon (the
// pylonDiamonds scene) is the hub; this overlay draws the online pylons as nodes
// on a ring with **bezier curves** flowing into the center, lit by live activity.
// Plain SVG (no three-effect) so it composites over the 3D pylon and deploys
// now. Polls /api/public/pylon-stats. Visual language:
// docs/autopilot-coder/2026-06-15-autopilot-home-network-visual-language.md (§3).

export const pylonBezierNetworkTagName = 'oa-pylon-bezier-network'

const MAX_NODES = 48
const CX = 50
const CY = 50

const pos = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? Math.floor(value) : 0

const hostCss = `
:host { position: absolute; inset: 0; display: block; pointer-events: none; z-index: 5; }
svg { width: 100%; height: 100%; display: block; }
.edge { fill: none; stroke: #d6f6ff; opacity: 0.10; stroke-width: 0.15; }
.edge.lit { stroke: #2979ff; opacity: 0.5; }
.edge.lit { stroke-dasharray: 1.2 1.2; animation: flow 2.4s linear infinite; }
.node { stroke: none; }
@keyframes flow { to { stroke-dashoffset: -2.4; } }
`

type Rendered = {
  readonly online: number
  readonly assignmentReady: number
  readonly intensity: number
}

const toRendered = (snapshot: PylonStatsSnapshot | null): Rendered => ({
  online: pos(snapshot?.pylonsOnlineNow),
  assignmentReady: pos(
    (snapshot as { pylonsAssignmentReadyNow?: number } | null)?.pylonsAssignmentReadyNow,
  ),
  intensity: computeActivityIntensity(snapshot),
})

// Deterministic ring layout (golden-angle spiral) so nodes read as a network and
// the layout is stable between polls.
const nodeXY = (index: number, total: number): { x: number; y: number } => {
  const golden = 2.399963229728653
  const ringR = 24 + Math.min(16, total * 0.25)
  const r = ringR * (0.55 + 0.45 * Math.sqrt((index + 1) / Math.max(1, total)))
  const a = index * golden
  return { x: CX + Math.cos(a) * r, y: CY + Math.sin(a) * r * 0.7 }
}

// A bezier (quadratic) curve from a node to the center, bowed perpendicular to
// the chord so edges arc rather than run straight.
const edgePath = (x: number, y: number): string => {
  const mx = (x + CX) / 2
  const my = (y + CY) / 2
  const dx = CX - x
  const dy = CY - y
  const len = Math.hypot(dx, dy) || 1
  const bow = Math.min(10, len * 0.25)
  const cx = mx + (-dy / len) * bow
  const cy = my + (dx / len) * bow
  return `M ${x.toFixed(2)} ${y.toFixed(2)} Q ${cx.toFixed(2)} ${cy.toFixed(2)} ${CX} ${CY}`
}

export type PylonBezierNetworkHandle = Readonly<{ dispose: () => void }>

export const mountPylonBezierNetwork = (
  root: ShadowRoot | HTMLElement,
  options: { fetchFn?: typeof fetch; intervalMs?: number } = {},
): PylonBezierNetworkHandle => {
  const edges = root.querySelector<SVGGElement>('g.edges')
  const nodes = root.querySelector<SVGGElement>('g.nodes')

  const render = (r: Rendered): void => {
    if (!edges || !nodes) return
    const count = Math.min(MAX_NODES, r.online)
    const litCount = count > 0 ? Math.round((r.assignmentReady / r.online) * count) : 0
    const edgeParts: string[] = []
    const nodeParts: string[] = []
    for (let i = 0; i < count; i += 1) {
      const { x, y } = nodeXY(i, count)
      const lit = i < litCount
      edgeParts.push(`<path class="edge${lit ? ' lit' : ''}" d="${edgePath(x, y)}" />`)
      const fill = lit ? '#2979ff' : '#d6f6ff'
      const opacity = lit ? (0.55 + r.intensity * 0.45).toFixed(2) : '0.4'
      const radius = lit ? 0.85 : 0.6
      nodeParts.push(
        `<circle class="node" cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="${radius}" fill="${fill}" opacity="${opacity}" />`,
      )
    }
    edges.innerHTML = edgeParts.join('')
    nodes.innerHTML = nodeParts.join('')
  }

  let disposed = false
  const apply = async (): Promise<void> => {
    const snapshot = await fetchPylonStats(options.fetchFn)
    if (disposed) return
    render(toRendered(snapshot))
  }
  render(toRendered(null))
  void apply()
  const timer = setInterval(() => void apply(), options.intervalMs ?? 15_000)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      clearInterval(timer)
    },
  }
}

const pylonBezierNetworkElement = defineCustomElement({
  events: {},
  properties: {},
  tag: pylonBezierNetworkTagName,
})

const makePylonBezierNetworkElement = (): CustomElementConstructor =>
  class PylonBezierNetworkElement extends HTMLElement {
    #handle: PylonBezierNetworkHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = hostCss
      // Build the SVG via the DOM API (not an HTML string) — keeps this data-viz
      // out of the icon policy's raw-inline-SVG rule (icons must come from the
      // generated catalog; a network graph is not an icon).
      const NS = 'http://www.w3.org/2000/svg'
      const svg = document.createElementNS(NS, 'svg')
      svg.setAttribute('viewBox', '0 0 100 100')
      svg.setAttribute('preserveAspectRatio', 'xMidYMid slice')
      svg.setAttribute('aria-hidden', 'true')
      const edges = document.createElementNS(NS, 'g')
      edges.setAttribute('class', 'edges')
      const nodes = document.createElementNS(NS, 'g')
      nodes.setAttribute('class', 'nodes')
      svg.append(edges, nodes)
      shadow.append(style, svg)
      this.#handle = mountPylonBezierNetwork(shadow)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerPylonBezierNetworkElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(pylonBezierNetworkTagName) !== undefined) return
  customElements.define(pylonBezierNetworkTagName, makePylonBezierNetworkElement())
}

export const pylonBezierNetworkView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPylonBezierNetworkElement()
  const element = pylonBezierNetworkElement.withMessage<Message>()
  return element(attributes, [])
}
