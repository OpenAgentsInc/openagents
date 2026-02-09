import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

/**
 * Placeholder for a future expandable right panel (Blueprint, controls, etc.).
 * Expand later without changing the main Autopilot layout.
 */
export type AutopilotExpandablePanelModel = {
  readonly open: boolean;
};

export const autopilotExpandablePanelTemplate = (
  model: AutopilotExpandablePanelModel,
): TemplateResult => {
  return html`<div data-autopilot-expandable-panel class="hidden" data-open="${model.open ? "1" : "0"}"></div>`;
};

export function runAutopilotExpandablePanel(
  container: Element,
  model: AutopilotExpandablePanelModel,
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotExpandablePanelTemplate(model));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse autopilot expandable panel]", err);
      return Effect.void;
    }),
  );
}
