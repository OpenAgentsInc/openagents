import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import { whitePreset } from "@openagentsinc/hud";
import { runAutopilotChat } from "./autopilot";
import { runAutopilotBlueprintPanel } from "./autopilotBlueprint";
import { runAutopilotControls } from "./autopilotControls";
import { runAutopilotSidebar } from "./autopilotSidebar";

import type { TemplateResult } from "@openagentsinc/effuse";
import type { AutopilotChatData } from "./autopilot";
import type { AutopilotBlueprintPanelModel } from "./autopilotBlueprint";
import type { AutopilotControlsModel } from "./autopilotControls";
import type { AutopilotSidebarModel } from "./autopilotSidebar";

type SlotName = "sidebar" | "chat" | "blueprint" | "controls";

type SlotKeys = {
  sidebar: string;
  chat: string;
  blueprint: string;
  controls: string;
};

const stateByContainer = new WeakMap<Element, SlotKeys>();

const autopilotBackgroundStyle = (): string => {
  const backgroundImage = [
    `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 55%)`,
    `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
    whitePreset.backgroundImage,
  ].join(", ");

  return `background-color: ${whitePreset.backgroundColor}; background-image: ${backgroundImage};`;
};

export const autopilotRouteShellTemplate = (): TemplateResult => {
  return html`
    <div class="fixed inset-0 overflow-hidden text-text-primary font-mono" data-autopilot-shell="1">
      <div class="absolute inset-0" style="${autopilotBackgroundStyle()}">
        <div data-hud-bg="dots-grid" class="absolute inset-0 pointer-events-none"></div>
      </div>

      <div class="relative z-10 flex h-screen min-h-0 w-full flex-col overflow-hidden">
        <main class="flex-1 min-h-0 w-full flex overflow-hidden">
          <div data-autopilot-slot="sidebar"></div>

          <div data-autopilot-slot="chat" class="flex-1 min-h-0 flex flex-col overflow-hidden">
            <div class="flex-1 min-h-0 flex items-center justify-center text-xs text-text-dim">
              Loading…
            </div>
          </div>

          <div
            data-autopilot-slot="blueprint"
            class="hidden lg:flex lg:w-[360px] shrink-0 border-l border-border-dark bg-bg-secondary shadow-[0_0_0_1px_rgba(255,255,255,0.04)]"
          >
            <div class="m-auto text-xs text-text-dim">Loading…</div>
          </div>
        </main>

        <div data-autopilot-slot="controls" class="absolute bottom-4 right-4"></div>
      </div>
    </div>
  `;
};

const ensureShell = (container: Element) =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const shell = container.querySelector("[data-autopilot-shell]");
    if (shell) return;
    yield* dom.render(container, autopilotRouteShellTemplate());
  });

const getSlot = (container: Element, name: SlotName): Element | null => {
  const node = container.querySelector(`[data-autopilot-slot="${name}"]`);
  return node instanceof Element ? node : null;
};

export type AutopilotRouteRenderInput = {
  readonly sidebarModel: AutopilotSidebarModel;
  readonly sidebarKey: string;
  readonly chatData: AutopilotChatData;
  readonly chatKey: string;
  readonly blueprintModel: AutopilotBlueprintPanelModel;
  readonly blueprintKey: string;
  readonly controlsModel: AutopilotControlsModel;
  readonly controlsKey: string;
};

/**
 * Render the Autopilot route UI by updating independent slots.
 *
 * This keeps chat/blueprint isolated: blueprint refreshes won't rerender chat unless
 * the chat key changes (avoids input/caret glitches).
 */
export const runAutopilotRoute = (
  container: Element,
  input: AutopilotRouteRenderInput,
): Effect.Effect<void> => {
  return Effect.gen(function* () {
    yield* ensureShell(container);

    const prev =
      stateByContainer.get(container) ??
      ({
        sidebar: "",
        chat: "",
        blueprint: "",
        controls: "",
      } satisfies SlotKeys);

    const next: SlotKeys = {
      sidebar: input.sidebarKey,
      chat: input.chatKey,
      blueprint: input.blueprintKey,
      controls: input.controlsKey,
    };

    const sidebarSlot = getSlot(container, "sidebar");
    const chatSlot = getSlot(container, "chat");
    const blueprintSlot = getSlot(container, "blueprint");
    const controlsSlot = getSlot(container, "controls");

    if (sidebarSlot && (prev.sidebar !== next.sidebar || sidebarSlot.childNodes.length === 0)) {
      yield* runAutopilotSidebar(sidebarSlot, input.sidebarModel);
    }

    if (chatSlot && (prev.chat !== next.chat || chatSlot.childNodes.length === 0)) {
      yield* runAutopilotChat(chatSlot, input.chatData);
    }

    if (
      blueprintSlot &&
      (prev.blueprint !== next.blueprint || blueprintSlot.childNodes.length === 0)
    ) {
      yield* runAutopilotBlueprintPanel(blueprintSlot, input.blueprintModel);
    }

    if (
      controlsSlot &&
      (prev.controls !== next.controls || controlsSlot.childNodes.length === 0)
    ) {
      yield* runAutopilotControls(controlsSlot, input.controlsModel);
    }

    stateByContainer.set(container, next);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse autopilot route]", err);
      return Effect.void;
    }),
  );
};
