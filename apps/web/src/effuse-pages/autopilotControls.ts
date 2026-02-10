import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

export type AutopilotControlsModel = {
  readonly isExportingBlueprint: boolean;
  readonly isResettingAgent: boolean;
};

export const autopilotControlsTemplate = (model: AutopilotControlsModel): TemplateResult => {
  return html`
    <div class="flex flex-col gap-1 text-right">
      <button
        type="button"
        data-ez="autopilot.controls.exportBlueprint"
        ${model.isExportingBlueprint ? "disabled" : ""}
        class="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
      >
        ${model.isExportingBlueprint ? "Exporting…" : "Export Blueprint JSON"}
      </button>
      <button
        type="button"
        data-ez="autopilot.controls.clearMessages"
        class="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
      >
        Clear messages
      </button>
      <button
        type="button"
        data-ez="autopilot.controls.resetAgent"
        ${model.isResettingAgent ? "disabled" : ""}
        class="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
      >
        ${model.isResettingAgent ? "Resetting..." : "Reset agent"}
      </button>
    </div>
  `;
};

export function runAutopilotControls(
  container: Element,
  model: AutopilotControlsModel,
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotControlsTemplate(model));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse controls]", err);
      return Effect.void;
    }),
  );
}
