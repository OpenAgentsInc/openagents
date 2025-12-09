/**
 * Introduction Card Widget
 *
 * Displays a centered introduction card for the TerminalBench Gym.
 */

import { Effect } from "effect"
import { html } from "../template/html.js"

import type { Widget } from "../widget/types.js"

// ============================================================================
// Types
// ============================================================================

/**
 * Intro Card State (empty for now, but can be extended)
 */
export interface IntroCardState {
  // No state needed for static card
}

/**
 * Intro Card Events (none for now)
 */
export type IntroCardEvent = never

// ============================================================================
// Widget Definition
// ============================================================================

export const IntroCardWidget: Widget<IntroCardState, IntroCardEvent> = {
  id: "intro-card",

  initialState: () => ({}),

  render: (ctx) =>
    Effect.gen(function* () {
      return html`
        <div class="fixed inset-0 flex items-center justify-center" style="padding: 24px; z-index: 10;">
          <div class="bg-zinc-950/90 border border-zinc-800/60 rounded-2xl backdrop-blur-xl shadow-2xl max-w-2xl w-full" style="padding: 48px 64px; font-family: 'Berkeley Mono', monospace;">
            <h1 class="text-5xl font-bold text-zinc-100 text-center tracking-tight" style="font-family: 'Berkeley Mono', monospace; margin: 0;">
              TerminalBench Gym
            </h1>
            <div style="margin-top: 24px; text-align: center;">
              <p class="text-zinc-400 text-sm font-mono" style="font-family: 'Berkeley Mono', monospace; margin: 0;">
                A training environment for AI agents
              </p>
            </div>
          </div>
        </div>
      `
    }),
}
