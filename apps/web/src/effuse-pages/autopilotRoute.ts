import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import { whitePreset } from "@openagentsinc/hud";
import { runAutopilotChat } from "./autopilot";

import type { TemplateResult } from "@openagentsinc/effuse";
import type { AutopilotChatData } from "./autopilot";

/** Simplified layout: dots grid (like homepage), chat + input only. No sidebars. */
type SlotName = "chat";

type SlotKeys = { chat: string };

const stateByContainer = new WeakMap<Element, SlotKeys>();

/** Same background as homepage: dots grid only. */
const autopilotBackgroundStyle = (): string => {
  const backgroundImage = [
    `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.07) 0%, rgba(255,255,255,0) 55%)`,
    `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
    whitePreset.backgroundImage,
  ].join(", ");
  return `background-color: ${whitePreset.backgroundColor}; background-image: ${backgroundImage};`;
};

export const autopilotRouteShellTemplate = (): TemplateResult => {
  return html`
    <div class="fixed inset-0 overflow-hidden text-white font-mono" data-autopilot-shell="1">
      <div class="absolute inset-0" style="${autopilotBackgroundStyle()}">
        <div data-hud-bg="dots-grid" class="absolute inset-0 pointer-events-none"></div>
      </div>
      <div class="relative z-10 flex h-screen min-h-0 w-full flex-col overflow-hidden">
        <div data-autopilot-slot="chat" class="flex-1 min-h-0 flex flex-col overflow-hidden">
          <div class="flex-1 min-h-0 flex items-center justify-center text-xs text-white/60">Loading…</div>
        </div>
      </div>
    </div>
  `;
};

const ensureShell = (container: Element) =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const shell = container.querySelector("[data-autopilot-shell]");
    if (shell) return;
    // Only replace container when it is empty (initial client render without ssrHtml).
    // If container already has children (e.g. from hydration), do not overwrite — slots
    // are already present and we must not replace them with the shell template (which
    // would show "Loading…" in chat/blueprint again).
    if (container.children.length > 0) return;
    yield* dom.render(container, autopilotRouteShellTemplate());
  });

const getSlot = (container: Element, name: SlotName): Element | null => {
  const node = container.querySelector(`[data-autopilot-slot="${name}"]`);
  return node instanceof Element ? node : null;
};

export type AutopilotRouteRenderInput = {
  readonly chatData: AutopilotChatData;
  readonly chatKey: string;
};

/**
 * Render the Autopilot route: dots grid background, chat (messages + input) only.
 */
export const runAutopilotRoute = (
  container: Element,
  input: AutopilotRouteRenderInput,
): Effect.Effect<void> => {
  return Effect.gen(function* () {
    yield* ensureShell(container);

    const prev = stateByContainer.get(container) ?? ({ chat: "" } satisfies SlotKeys);
    const next: SlotKeys = { chat: input.chatKey };
    const chatSlot = getSlot(container, "chat");

    if (chatSlot && (prev.chat !== next.chat || chatSlot.childNodes.length === 0)) {
      yield* runAutopilotChat(chatSlot, input.chatData);
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
