import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { PROMPT_MESSAGE } from "./constants.js"

export const renderUnifiedStreamView = (
  formatted: TemplateResult
): TemplateResult => html`
  <div class="flex h-full items-start gap-6 p-0">
    <section class="w-[400px] border border-border bg-background">
      <table class="w-full text-[11px]">
        <tbody>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground flex items-center gap-2">
              Agent:
              <select 
                class="bg-surface border border-border text-foreground text-[10px] px-1 py-0.5 outline-none cursor-pointer"
                data-role="agent-selector"
              >
                <option value="Adjutant" selected>Adjutant</option>
                <option value="Gemini">Gemini</option>
                <option value="Codex">Codex (requires codex-acp)</option>
              </select>
              <button
                class="inline-flex items-center justify-center border border-border px-3 py-1 text-[10px] uppercase text-accent hover:bg-surface-strong disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer ml-auto"
                data-ez="unified.send"
                data-role="send-button"
                disabled
                aria-disabled="true"
              >
                Run
              </button>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground">
              Command: <span class="text-foreground">${PROMPT_MESSAGE}</span>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground">
              Status:
              <span class="inline-flex items-center gap-2 text-muted-foreground">
                <span class="h-2 w-2 bg-status-idle" data-role="status-dot"></span>
                <span data-role="status-text">Connecting</span>
                <span class="text-destructive" data-role="status-error"></span>
              </span>
              <span class="ml-4 text-muted-foreground">Events:</span>
              <span class="font-mono text-foreground" data-role="events-count">0</span>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground">
              Workspace:
              <span class="font-mono text-foreground" data-role="workspace-path">Pending...</span>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground">
              Session:
              <span class="font-mono text-foreground" data-role="session-id">Waiting for SessionStarted</span>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs text-muted-foreground">
              Usage: <span class="text-foreground">Session</span>
              <span class="ml-2 text-muted-foreground" data-role="usage-session-reset"></span>
              <span class="ml-2 font-semibold text-muted-foreground" data-role="usage-session-percent">--</span>
              <div class="mt-2 h-1 bg-surface-strong">
                <div
                  class="h-full bg-accent transition-all duration-300 ease-out"
                  data-role="usage-session-bar"
                  style="width: 0%"
                ></div>
              </div>
              <div class="hidden mt-2" data-role="usage-weekly">
                <div>
                  Weekly
                  <span class="ml-2 text-muted-foreground" data-role="usage-weekly-reset"></span>
                  <span class="ml-2 font-semibold text-muted-foreground" data-role="usage-weekly-percent">--</span>
                </div>
                <div class="mt-1 h-1 bg-surface-strong">
                  <div
                    class="h-full bg-accent transition-all duration-300 ease-out"
                    data-role="usage-weekly-bar"
                    style="width: 0%"
                  ></div>
                </div>
              </div>
              <div class="hidden mt-2 text-[10px] text-muted-foreground" data-role="usage-credits"></div>
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1 text-xs uppercase text-muted-foreground">
              Formatted
            </td>
          </tr>
          <tr class="border-b border-border">
            <td class="p-1">
              <div class="flex max-h-[900px] flex-col gap-3 overflow-y-auto border border-border bg-surface-muted p-2" data-role="formatted-feed">
                ${formatted}
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </section>
    <section class="w-[400px] border border-border bg-background">
      <table class="w-full text-[11px]">
        <tbody>
          <tr class="border-b border-border">
            <td class="p-1 text-xs uppercase text-muted-foreground">
              Raw Events
            </td>
          </tr>
          <tr>
            <td class="p-1">
              <div class="flex max-h-[560px] flex-col gap-3 overflow-y-auto border border-border bg-surface-muted p-2" data-role="event-feed">
                <div class="text-xs italic text-muted-foreground" data-role="feed-empty">No unified events yet.</div>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  </div>
`
