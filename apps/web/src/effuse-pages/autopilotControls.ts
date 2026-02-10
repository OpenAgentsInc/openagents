import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

export type AutopilotControlsModel = {
  readonly isExportingBlueprint: boolean;
  readonly isResettingAgent: boolean;
  readonly dseStrategyId: "direct.v1" | "rlm_lite.v1";
  readonly dseBudgetProfile: "small" | "medium" | "long";
  readonly isRunningDseRecap: boolean;
  readonly dseErrorText: string | null;
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

      <div class="mt-2 border-t border-white/15 pt-2">
        <div class="text-[11px] uppercase tracking-wider text-white/50">DSE Debug</div>
        <div class="mt-2 flex flex-col gap-2 items-end">
          <label class="flex items-center gap-2 text-xs text-white/60">
            <span>Strategy</span>
            <select
              name="strategyId"
              data-ez="autopilot.controls.dse.strategy"
              data-ez-trigger="change"
              class="text-xs font-mono rounded border border-white/15 bg-black/40 text-white/90 px-2 py-1 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            >
              <option value="direct.v1" ${model.dseStrategyId === "direct.v1" ? "selected" : ""}>direct.v1</option>
              <option value="rlm_lite.v1" ${model.dseStrategyId === "rlm_lite.v1" ? "selected" : ""}>rlm_lite.v1</option>
            </select>
          </label>

          <label class="flex items-center gap-2 text-xs text-white/60">
            <span>Budget</span>
            <select
              name="budgetProfile"
              data-ez="autopilot.controls.dse.budget"
              data-ez-trigger="change"
              class="text-xs font-mono rounded border border-white/15 bg-black/40 text-white/90 px-2 py-1 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            >
              <option value="small" ${model.dseBudgetProfile === "small" ? "selected" : ""}>small</option>
              <option value="medium" ${model.dseBudgetProfile === "medium" ? "selected" : ""}>medium</option>
              <option value="long" ${model.dseBudgetProfile === "long" ? "selected" : ""}>long</option>
            </select>
          </label>

          <button
            type="button"
            data-ez="autopilot.controls.dse.recap"
            ${model.isRunningDseRecap ? "disabled" : ""}
            class="text-xs font-mono text-white/80 hover:text-white disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30 rounded px-2 py-1 border border-white/15 bg-white/5"
          >
            ${model.isRunningDseRecap ? "Running recap…" : "Run recap (canary)"}
          </button>

          ${model.dseErrorText
            ? html`<div class="text-xs text-red-300 font-mono max-w-[240px] break-words">${model.dseErrorText}</div>`
            : html``}
        </div>
      </div>
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
