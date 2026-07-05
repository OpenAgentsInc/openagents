import { useEffect, useState } from 'react'

import {
  type PylonBezierGraph,
  type PylonStatsSnapshot,
  fetchPylonStats,
  PYLON_STAT_LOADING,
  PYLON_STATS,
  pylonBezierGraph,
  pylonStatValues,
} from './-pylon-network'

// `openagents.com/pylons` — the Pylon network hero page. Ported from
// `apps/web/src/page/loggedOut/page/pylon.ts`.
//
// Scope note (bundle budget, not a fidelity shortcut): the legacy Foldkit page
// composes four layers — an install CTA, the literal WebGL diamond-refraction
// scene (`scene/pylonDiamonds.ts`, backed by `three` + `@openagentsinc/
// three-effect` + `GLTFLoader`), an SVG bezier network graph, and a live stats
// overlay. This route ports every layer that doesn't require `three`: the CTA,
// the bezier graph (pure SVG + math, no 3D dependency), the live stats
// counters, and the "Copy Agent Instructions" control. It intentionally does
// NOT pull `three`/`@openagentsinc/three-effect`/`GLTFLoader` into this app.
// Those add ~350-600 KiB of minified JS even with tree-shaking (three.module.
// min.js alone is ~365 KiB), and the Start funnel enforces a 760 KiB total
// client-JS budget across every route (`-funnel-budget.ts`) plus a 120 KiB
// per-route-chunk cap — already at ~709 KiB before this route. Importing the
// literal 3D scene here would blow both budgets by a wide margin, and that
// budget is a deliberate, existing performance gate for this bundle, not
// something to quietly relax mid-migration. The literal WebGL diamond mesh
// stays on the legacy Foldkit `/pylons` page (`apps/web`, still served by the
// production Worker) until an explicit follow-up decision — either a
// dedicated lazy sub-bundle exempted from this budget, or a raised budget for
// scene-bearing routes. In its place this route renders a design-consistent
// ambient glow backdrop (the same blue-glow visual language already used by
// `SceneLayer` on `/code` and `/khala`) rather than fabricating a fake
// diamond shape.
//
// Live-fetch note: unlike every prior TS-6 route (which stayed static/SSR-only
// rather than being first to wire a client fetch), this route polls the real
// public `GET /api/public/pylon-stats` endpoint — the same one the legacy page
// uses, no auth, no spend, no mutation. That's a deliberate exception: this
// page's entire purpose is showing live network state, so freezing it at the
// permanent "before first poll" placeholder (the honest-idle-state approach
// used for `/mirrorcode`, `/promises`, `/stats`) would not be a faithful port
// of what this specific page is for. Fail-soft is preserved: any fetch error
// renders the same dormant/zero state the legacy page shows.
//
// The countdown-to-launch branch of the legacy `pylonLaunchGateElement` is not
// ported: its fixed deadline (June 15, 2026, 1 PM America/Chicago) has already
// passed and can only ever be in the past from here forward, so a real visitor
// today only ever reaches the post-launch "Copy Agent Instructions" state —
// same scoping posture as the `/onboarding` funding-step branch in Slice 17.

const PYLON_INSTALL_COMMAND = 'npx @openagentsinc/pylon'
const AGENT_INSTRUCTIONS_URL = '/AGENTS.md'

function PylonAmbientBackdrop() {
  return (
    <div
      aria-hidden="true"
      className="absolute inset-0 z-0"
      data-pylon-scene="ambient-placeholder"
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_45%,rgba(41,121,255,0.22),transparent_32%),linear-gradient(180deg,rgba(0,0,0,0.35),#000_82%)]" />
      <div className="absolute top-1/2 left-1/2 aspect-square w-[min(60vw,26rem)] -translate-1/2 rotate-45 border border-khala-energy-cyan/25 bg-khala-surface/30 khala-glow" />
    </div>
  )
}

function PylonInstallCta() {
  return (
    <div className="pointer-events-none absolute inset-x-0 top-4 z-20 flex justify-center px-3">
      <div
        className="pointer-events-auto inline-flex max-w-md flex-col items-center gap-1.5 border border-khala-energy-cyan bg-khala-void/86 px-4 py-3 text-center font-mono text-khala-text shadow-[0_0_28px_rgba(41,121,255,0.22)]"
        data-cta="install-pylon"
      >
        <span className="text-[0.75rem] font-bold uppercase leading-none tracking-[0.08em]">
          Run a Pylon node
        </span>
        <span className="text-[0.6rem] uppercase leading-none tracking-[0.08em] text-white/55">
          Paste this to your coding agent
        </span>
        <pre
          className="mt-0.5 w-full select-all overflow-x-auto border border-white/15 bg-[rgba(12,15,19,0.94)] px-3 py-2 text-left text-[0.8rem] leading-none text-khala-energy-cyan"
          data-cta="install-pylon-command"
        >
          <code>{PYLON_INSTALL_COMMAND}</code>
        </pre>
        {/* AO-5 (#5446): a single discoverable link to the Mac app download
            page. The homepage stays Pylon-CLI-first; this is a link only, not
            a marketing-copy rewrite. */}
        <a
          className="text-[0.6rem] uppercase leading-none tracking-[0.08em] text-white/55 underline underline-offset-2 hover:text-white"
          data-cta="download-autopilot-link"
          href="/download"
        >
          Or download the Mac app
        </a>
      </div>
    </div>
  )
}

function PylonBezierNetwork({ graph }: Readonly<{ graph: PylonBezierGraph }>) {
  return (
    <svg
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 z-[5] block h-full w-full"
      data-pylon-scene="bezier-network"
      preserveAspectRatio="xMidYMid slice"
      viewBox="0 0 100 100"
    >
      <g>
        {graph.edges.map((edge, index) => (
          <path
            className={edge.lit ? 'opacity-50' : 'opacity-10'}
            d={edge.d}
            fill="none"
            key={index}
            stroke={edge.lit ? '#2979ff' : '#d6f6ff'}
            strokeDasharray={edge.lit ? '1.2 1.2' : undefined}
            strokeWidth={0.15}
          />
        ))}
      </g>
      <g>
        {graph.nodes.map((node, index) => (
          <circle
            cx={node.cx}
            cy={node.cy}
            fill={node.lit ? '#2979ff' : '#d6f6ff'}
            key={index}
            opacity={node.opacity}
            r={node.radius}
            stroke="none"
          />
        ))}
      </g>
    </svg>
  )
}

function PylonStatsOverlay({
  snapshot,
  values,
}: Readonly<{
  snapshot: PylonStatsSnapshot | null
  values: Record<string, string>
}>) {
  return (
    <div
      className="pointer-events-none absolute inset-0 z-[6] flex items-end justify-center px-3 pb-[clamp(1.5rem,5vw,4rem)]"
      data-pylon-scene="stats-overlay"
    >
      <div className="flex flex-wrap items-end justify-center gap-[clamp(1rem,4vw,3rem)] opacity-92">
        {PYLON_STATS.map(stat => (
          <div
            className="flex flex-col items-center gap-0.5 text-khala-text-muted"
            key={stat.key}
          >
            <span
              className="text-[clamp(1.1rem,2.4vw,1.9rem)] font-semibold tabular-nums text-khala-energy-cyan"
              data-stat-value={stat.key}
            >
              {snapshot === null ? PYLON_STAT_LOADING : values[stat.key]}
            </span>
            <span className="text-[0.65rem] uppercase tracking-[0.14em] text-khala-text-faint">
              {stat.label}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

type CopyState = 'copied' | 'copying' | 'failed' | 'idle'

function PylonCopyInstructions() {
  const [state, setState] = useState<CopyState>('idle')

  useEffect(() => {
    if (state !== 'copied' && state !== 'failed') return
    const timer = setTimeout(() => setState('idle'), 3000)
    return () => clearTimeout(timer)
  }, [state])

  const handleClick = async (): Promise<void> => {
    if (state === 'copying') return
    setState('copying')
    try {
      const response = await fetch(AGENT_INSTRUCTIONS_URL, {
        cache: 'no-store',
        headers: { accept: 'text/markdown,text/plain,*/*' },
      })
      if (!response.ok) throw new Error(`AGENTS.md returned HTTP ${response.status}`)
      await navigator.clipboard.writeText(await response.text())
      setState('copied')
    } catch {
      setState('failed')
    }
  }

  const label =
    state === 'copying'
      ? 'Copying...'
      : state === 'copied'
        ? 'Copied'
        : state === 'failed'
          ? 'Copy failed'
          : 'Copy Agent Instructions'
  const status =
    state === 'copied'
      ? 'Copied from openagents.com/AGENTS.md'
      : state === 'failed'
        ? 'Open /AGENTS.md'
        : ''

  return (
    <div
      className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center"
      data-pylon-scene="launch-gate"
    >
      <div className="grid justify-items-center gap-3">
        <button
          aria-busy={state === 'copying'}
          className="pointer-events-auto min-h-[2.75rem] border border-khala-energy-cyan bg-khala-void/86 px-4 py-3 font-mono text-xs font-bold uppercase tracking-[0.08em] text-khala-text shadow-[0_0_28px_rgba(41,121,255,0.22)] hover:bg-[rgba(12,15,19,0.94)] hover:border-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-khala-energy focus-visible:outline-offset-[3px]"
          data-cta="copy-agent-instructions"
          onClick={() => void handleClick()}
          type="button"
        >
          {label}
        </button>
        <div
          aria-live="polite"
          className="min-h-[1rem] font-mono text-[0.68rem] uppercase tracking-[0.08em] text-white/58"
        >
          {status}
        </div>
      </div>
    </div>
  )
}

export function PylonsPage() {
  const [snapshot, setSnapshot] = useState<PylonStatsSnapshot | null>(null)

  useEffect(() => {
    let cancelled = false
    const poll = async (): Promise<void> => {
      const next = await fetchPylonStats()
      if (!cancelled) setSnapshot(next)
    }
    void poll()
    const timer = setInterval(() => void poll(), 3000)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

  const graph = pylonBezierGraph(snapshot)
  const values = pylonStatValues(snapshot)

  return (
    <section
      className="relative h-dvh min-h-dvh w-full overflow-hidden bg-black"
      data-route="pylon"
    >
      <PylonAmbientBackdrop />
      <PylonBezierNetwork graph={graph} />
      <PylonStatsOverlay snapshot={snapshot} values={values} />
      <PylonInstallCta />
      <PylonCopyInstructions />
    </section>
  )
}
