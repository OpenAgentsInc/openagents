import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

export type AutopilotBlueprintPanelModel = {
  readonly updatedAtLabel: string | null;
  readonly isLoading: boolean;
  readonly isEditing: boolean;
  readonly canEdit: boolean;
  readonly isSaving: boolean;
  readonly errorText: string | null;
  readonly blueprintText: string | null;
  readonly draft: {
    readonly userHandle: string;
    readonly agentName: string;
    readonly identityVibe: string;
    readonly characterVibe: string;
    readonly characterBoundaries: string;
  } | null;
};

export const autopilotBlueprintPanelTemplate = (
  model: AutopilotBlueprintPanelModel,
): TemplateResult => {
  return html`
    <div class="flex flex-col h-full min-h-0 w-full">
      <div class="flex items-center justify-between h-11 px-3 border-b border-border-dark">
        <div class="text-xs text-text-dim uppercase tracking-wider">Blueprint</div>
        <div class="flex items-center gap-2">
          ${model.updatedAtLabel
            ? html`<div class="text-[10px] text-text-dim">${model.updatedAtLabel}</div>`
            : null}
          <button
            type="button"
            data-ez="autopilot.blueprint.toggleEdit"
            ${model.isLoading || !model.canEdit ? "disabled" : ""}
            class="text-[10px] font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus rounded px-2 py-1"
          >
            ${model.isEditing ? "Cancel" : "Edit"}
          </button>
          <button
            type="button"
            data-ez="autopilot.blueprint.refresh"
            ${model.isLoading ? "disabled" : ""}
            class="text-[10px] font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus rounded px-2 py-1"
          >
            ${model.isLoading ? "Syncing…" : "Refresh"}
          </button>
        </div>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto p-3 overseer-scroll">
        ${model.errorText
          ? html`<div class="text-xs text-red-400">Blueprint error: ${model.errorText}</div>`
          : model.isEditing
            ? html`
                <div class="flex flex-col gap-3">
                  <div class="text-[11px] text-text-dim">
                    Edit the Blueprint fields below. (Avoid personal info; handle/nickname only.)
                  </div>

                  <label class="flex flex-col gap-1">
                    <span class="text-[10px] text-text-dim uppercase tracking-wider">Your handle</span>
                    <input
                      id="blueprint-user-handle"
                      name="userHandle"
                      data-ez="autopilot.blueprint.draft"
                      data-ez-trigger="input"
                      value="${model.draft?.userHandle ?? ""}"
                      class="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    />
                  </label>

                  <label class="flex flex-col gap-1">
                    <span class="text-[10px] text-text-dim uppercase tracking-wider">Agent name</span>
                    <input
                      id="blueprint-agent-name"
                      name="agentName"
                      data-ez="autopilot.blueprint.draft"
                      data-ez-trigger="input"
                      value="${model.draft?.agentName ?? ""}"
                      class="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    />
                  </label>

                  <label class="flex flex-col gap-1">
                    <span class="text-[10px] text-text-dim uppercase tracking-wider">Agent vibe</span>
                    <input
                      id="blueprint-identity-vibe"
                      name="identityVibe"
                      data-ez="autopilot.blueprint.draft"
                      data-ez-trigger="input"
                      value="${model.draft?.identityVibe ?? ""}"
                      class="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    />
                  </label>

                  <label class="flex flex-col gap-1">
                    <span class="text-[10px] text-text-dim uppercase tracking-wider">Character vibe</span>
                    <input
                      id="blueprint-character-vibe"
                      name="characterVibe"
                      data-ez="autopilot.blueprint.draft"
                      data-ez-trigger="input"
                      value="${model.draft?.characterVibe ?? ""}"
                      class="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    />
                  </label>

                  <label class="flex flex-col gap-1">
                    <span class="text-[10px] text-text-dim uppercase tracking-wider">
                      Boundaries (one per line)
                    </span>
                    <textarea
                      id="blueprint-character-boundaries"
                      name="characterBoundaries"
                      data-ez="autopilot.blueprint.draft"
                      data-ez-trigger="input"
                      rows="8"
                      class="w-full resize-y rounded border border-border-dark bg-surface-primary px-2 py-2 text-[12px] leading-4 text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    >${model.draft?.characterBoundaries ?? ""}</textarea>
                  </label>

                  <button
                    type="button"
                    data-ez="autopilot.blueprint.save"
                    ${model.isSaving ? "disabled" : ""}
                    class="inline-flex h-9 items-center justify-center rounded px-3 text-xs font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
                  >
                    ${model.isSaving ? "Saving…" : "Save Blueprint"}
                  </button>
                </div>
              `
            : model.blueprintText
              ? html`<pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${model.blueprintText}</pre>`
              : html`<div class="text-xs text-text-dim">(no blueprint)</div>`}
      </div>
    </div>
  `;
};

export function runAutopilotBlueprintPanel(
  container: Element,
  model: AutopilotBlueprintPanelModel,
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotBlueprintPanelTemplate(model));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse blueprint panel]", err);
      return Effect.void;
    }),
  );
}
