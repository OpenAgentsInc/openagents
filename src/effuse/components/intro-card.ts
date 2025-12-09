/**
 * Introduction Card Component
 *
 * Displays a centered introduction card for the TerminalBench Gym.
 */

import { Effect } from "effect"
import type { Component } from "../component/types.js"
import { html } from "../template/html.js"


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
// Component Definition
// ============================================================================

export const IntroCardComponent: Component<IntroCardState, IntroCardEvent> = {
  id: "intro-card",

  initialState: () => ({}),

  render: (ctx) =>
    Effect.gen(function* () {
      return html`
        <div class="fixed top-4 left-4 intro-card-fade-in" style="z-index: 10;">
          <h1
            class="text-zinc-400"
            style="font-family: 'Berkeley Mono', monospace; margin: 0; font-size: 12px; font-weight: normal;"
          >
            OpenAgents Gym
          </h1>
        </div>
      `
    }),
}
