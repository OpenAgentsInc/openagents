/**
 * Introduction Card Widget
 *
 * Displays a centered introduction card for the TerminalBench Gym.
 */

import { Effect } from "effect"
import { html } from "../template/html.js"
import type { ComponentContext } from "../component/types.js"
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

  render: (ctx: ComponentContext<IntroCardState, IntroCardEvent>) =>
    Effect.gen(function* () {
      return html`
        <div class="fixed inset-0 flex items-center justify-center" style="padding: 24px; z-index: 10;">
          <div
            class="bg-zinc-950/10 border border-zinc-800/60 shadow-2xl w-full intro-card-fade-in"
            style="padding: 32px 48px; font-family: 'Berkeley Mono', monospace; border-radius: 0; max-width: 600px;"
          >
            <h1
              class="font-bold text-zinc-100 text-center tracking-tight"
              style="font-family: 'Berkeley Mono', monospace; margin: 0; font-size: 24px;"
            >
              OpenAgents Gym
            </h1>
            <div style="margin-top: 16px; text-align: center;">
              <p
                class="text-zinc-400 font-mono"
                style="font-family: 'Berkeley Mono', monospace; margin: 0; font-size: 12px;"
              >
                A training environment for AI agents
              </p>
            </div>
          </div>
        </div>
      `
    }),
}
