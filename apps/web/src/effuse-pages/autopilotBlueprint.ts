import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

export type AutopilotBlueprintPanelModel = {
  readonly updatedAtLabel: string | null;
  readonly isLoading: boolean;
  readonly canEdit: boolean;
  readonly isSaving: boolean;
  readonly errorText: string | null;
  readonly mode: "form" | "raw";
  readonly rawErrorText: string | null;
  readonly rawDraft: string | null;
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
          <div class="flex items-center rounded border border-border-dark bg-surface-primary p-0.5">
            <button
              type="button"
              data-ez="autopilot.blueprint.setMode"
              data-ez-vals='{"mode":"form"}'
              ${model.isLoading || !model.canEdit ? "disabled" : ""}
              class="${model.mode === "form"
                ? "bg-accent text-bg-primary border-accent"
                : "bg-transparent text-text-muted border-transparent hover:text-text-primary"} text-[10px] font-mono border rounded px-2 py-1 disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
              aria-pressed="${model.mode === "form" ? "true" : "false"}"
            >
              Form
            </button>
            <button
              type="button"
              data-ez="autopilot.blueprint.setMode"
              data-ez-vals='{"mode":"raw"}'
              ${model.isLoading || !model.canEdit ? "disabled" : ""}
              class="${model.mode === "raw"
                ? "bg-accent text-bg-primary border-accent"
                : "bg-transparent text-text-muted border-transparent hover:text-text-primary"} text-[10px] font-mono border rounded px-2 py-1 disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
              aria-pressed="${model.mode === "raw" ? "true" : "false"}"
            >
              Raw
            </button>
          </div>
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
        <div class="flex flex-col gap-3">
          ${model.errorText
            ? html`<div class="text-xs text-red-400">Blueprint error: ${model.errorText}</div>`
            : null}

          ${!model.canEdit
            ? html`<div class="text-xs text-text-dim">(no blueprint)</div>`
            : model.mode === "raw"
              ? html`
                  <div class="text-[11px] text-text-dim">
                    Edit raw Blueprint JSON. Save will import exactly what you provide.
                  </div>
                  <textarea
                    id="blueprint-raw-json"
                    name="raw"
                    data-ez="autopilot.blueprint.rawDraft"
                    data-ez-trigger="input"
                    rows="18"
                    class="w-full min-h-[320px] resize-y rounded border border-border-dark bg-surface-primary px-2 py-2 text-[11px] leading-4 text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                  >${model.rawDraft ?? ""}</textarea>
                  ${model.rawErrorText
                    ? html`<div class="text-xs text-red-400">JSON error: ${model.rawErrorText}</div>`
                    : null}
                `
              : html`
                  <div class="text-[11px] text-text-dim">
                    Edit Blueprint fields below. (Avoid personal info; handle/nickname only.)
                  </div>

                  <div class="rounded border border-border-dark bg-bg-secondary/40 p-3">
                    <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">User</div>
                    <label class="flex flex-col gap-1">
                      <span class="text-[10px] text-text-dim uppercase tracking-wider">Handle</span>
                      <input
                        id="blueprint-user-handle"
                        name="userHandle"
                        data-ez="autopilot.blueprint.draft"
                        data-ez-trigger="input"
                        value="${model.draft?.userHandle ?? ""}"
                        class="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                      />
                    </label>
                  </div>

                  <div class="rounded border border-border-dark bg-bg-secondary/40 p-3">
                    <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">Identity</div>
                    <label class="flex flex-col gap-1 mb-3">
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
                  </div>

                  <div class="rounded border border-border-dark bg-bg-secondary/40 p-3">
                    <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">Character</div>
                    <label class="flex flex-col gap-1 mb-3">
                      <span class="text-[10px] text-text-dim uppercase tracking-wider">Vibe</span>
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
                  </div>
                `}

          ${model.canEdit
            ? html`
                <button
                  type="button"
                  data-ez="autopilot.blueprint.save"
                  ${model.isSaving ? "disabled" : ""}
                  class="inline-flex h-9 items-center justify-center rounded px-3 text-xs font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
                >
                  ${model.isSaving ? "Saving…" : "Save Blueprint"}
                </button>
              `
            : null}
        </div>
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
