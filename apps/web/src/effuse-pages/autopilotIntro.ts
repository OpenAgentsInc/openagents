import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import { hatcheryButton } from "@openagentsinc/effuse-ui";

import type { TemplateResult } from "@openagentsinc/effuse";

/**
 * Renders the Autopilot intro view for unauthenticated users:
 * welcome copy and CTA to sign in to start the agent.
 */
export function autopilotIntroTemplate(): TemplateResult {
  return html`
    <div class="flex-1 min-h-0 flex flex-col overflow-hidden" data-autopilot-chat data-autopilot-intro="1">
      <header class="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
        <span class="text-xs text-text-dim uppercase tracking-wider">Autopilot</span>
      </header>
      <section class="flex-1 min-h-0 flex flex-col items-center justify-center p-6 sm:p-8">
        <div class="mx-auto w-full max-w-lg text-center">
          <h2 class="text-xl font-semibold text-text-primary tracking-tight sm:text-2xl">
            Your personal agent
          </h2>
          <p class="mt-3 text-sm text-text-muted sm:text-base">
            No Mac Mini required. Sign in to start a session and chat with Autopilot.
          </p>
          <div class="mt-8">
            ${hatcheryButton({
              href: "/login",
              label: "Sign in to start",
              size: "large",
              className: "w-full sm:w-auto",
            })}
          </div>
        </div>
      </section>
    </div>
  `;
}

export function runAutopilotIntro(container: Element): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotIntroTemplate());
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse autopilot intro]", err);
      return Effect.void;
    }),
  );
}
