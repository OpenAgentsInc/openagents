import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";

export type SidebarUser = {
  readonly email: string | null;
  readonly firstName: string | null;
  readonly lastName: string | null;
};

export type AutopilotSidebarModel = {
  readonly collapsed: boolean;
  readonly pathname: string;
  readonly user: SidebarUser | null;
  readonly userMenuOpen: boolean;
};

const BERKELEY = "var(--font-berkeley)";

function cx(...parts: Array<string | null | undefined | false>): string {
  return parts.filter(Boolean).join(" ");
}

function getInitials(user: SidebarUser): string {
  const first = user.firstName?.trim().slice(0, 1) ?? "";
  const last = user.lastName?.trim().slice(0, 1) ?? "";
  if (first || last) return (first + last).toUpperCase();
  const email = user.email?.trim() ?? "";
  if (email) return email.slice(0, 2).toUpperCase();
  return "?";
}

function getDisplayName(user: SidebarUser): string {
  const fullName = [user.firstName, user.lastName].filter(Boolean).join(" ");
  return fullName || user.email || "Account";
}

export const autopilotSidebarTemplate = (model: AutopilotSidebarModel): TemplateResult => {
  const widthClass = model.collapsed ? "w-12" : "w-64";

  const chatItems = [{ id: "welcome", label: "Welcome" }] as const;
  const activeChatId = "welcome";

  const nav = html`
    <nav class="flex flex-col gap-0 px-2 py-2" aria-label="Chats">
      ${chatItems.map((item) => {
        const active = item.id === activeChatId;
        return html`
          <a
            href="/autopilot"
            class="${cx(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium transition-colors",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30",
              active
                ? "bg-[#1a1a1a] text-white"
                : "text-white/70 hover:bg-[#1a1a1a] hover:text-white",
            )}"
            title="${model.collapsed ? item.label : ""}"
          >
            <span class="${model.collapsed ? "hidden" : "block truncate"}">
              ${item.label}
            </span>
          </a>
        `;
      })}
    </nav>
  `;

  const userMenu = (() => {
    if (!model.user) return null;
    const initials = getInitials(model.user);
    const displayName = getDisplayName(model.user);

    if (model.collapsed) {
      return html`
        <div class="relative flex justify-center">
          <button
            type="button"
            data-ez="autopilot.sidebar.toggleUserMenu"
            class="flex size-8 items-center justify-center rounded-full border border-white/10 bg-[#1a1a1a] text-xs font-medium text-white hover:bg-white/10 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            aria-expanded="${model.userMenuOpen ? "true" : "false"}"
            aria-haspopup="true"
            aria-label="User menu"
          >
            ${initials}
          </button>
          ${model.userMenuOpen
            ? html`
                <div
                  class="absolute bottom-full left-1/2 mb-1 min-w-[140px] -translate-x-1/2 rounded-md border border-white/10 bg-[#0a0a0a] py-1 shadow-lg"
                  role="menu"
                >
                  <div class="truncate px-3 py-2 text-xs text-white/60">${model.user.email ?? ""}</div>
                  <button
                    type="button"
                    role="menuitem"
                    data-ez="autopilot.sidebar.logout"
                    class="w-full px-3 py-2 text-left text-sm text-white hover:bg-[#1a1a1a] focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
                  >
                    Log out
                  </button>
                </div>
              `
            : null}
        </div>
      `;
    }

    return html`
      <div class="relative">
        <button
          type="button"
          data-ez="autopilot.sidebar.toggleUserMenu"
          class="flex w-full items-center gap-3 rounded-md px-2 py-2 text-left text-white hover:bg-[#1a1a1a] focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30 data-[state=open]:bg-[#1a1a1a]"
          aria-expanded="${model.userMenuOpen ? "true" : "false"}"
          aria-haspopup="true"
          aria-label="User menu"
        >
          <span
            class="flex size-8 shrink-0 items-center justify-center rounded-full border border-white/10 bg-[#1a1a1a] text-sm font-medium text-white"
            aria-hidden
          >
            ${initials}
          </span>
          <div class="flex min-w-0 flex-1 flex-col leading-tight">
            <span class="truncate text-sm font-medium text-white">${displayName}</span>
            <span class="truncate text-xs text-white/70">${model.user.email ?? "Account"}</span>
          </div>
          <svg
            class="size-4 shrink-0 text-white/70"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            aria-hidden
          >
            <path d="M6 9l6 6 6-6" />
          </svg>
        </button>
        ${model.userMenuOpen
          ? html`
              <div
                class="absolute bottom-full left-0 right-0 mb-1 rounded-md border border-white/10 bg-[#0a0a0a] py-1 shadow-lg"
                role="menu"
              >
                <button
                  type="button"
                  role="menuitem"
                  data-ez="autopilot.sidebar.logout"
                  class="w-full px-3 py-2 text-left text-sm text-white hover:bg-[#1a1a1a] focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
                >
                  Log out
                </button>
              </div>
            `
          : null}
      </div>
    `;
  })();

  const titleClass = cx(
    "flex flex-1 items-center justify-center text-base font-semibold text-white transition-opacity duration-200 ease-linear",
    model.collapsed ? "w-0 overflow-hidden opacity-0" : "opacity-100",
  );

  return html`
    <div
      data-autopilot-sidebar-root="1"
      class="${cx(
        "hidden md:flex h-full flex-col shrink-0 border-r border-white/10 bg-[#0a0a0a] text-white/88 transition-[width] duration-200 ease-linear",
        widthClass,
      )}"
      style="font-family: ${BERKELEY}"
    >
      <aside class="flex h-full flex-col" aria-label="Autopilot sidebar">
        <header class="relative flex h-12 shrink-0 flex-row items-center gap-2 border-b border-white/10 px-3">
          <button
            type="button"
            data-ez="autopilot.sidebar.toggleCollapse"
            class="absolute left-2 z-10 flex size-9 shrink-0 items-center justify-center rounded-md text-white/70 hover:bg-[#1a1a1a] hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            aria-label="${model.collapsed ? "Expand sidebar" : "Collapse sidebar"}"
          >
            <svg class="size-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden>
              <path d="M6.835 4c-.451.004-.82.012-1.137.038-.386.032-.659.085-.876.162l-.2.086c-.44.224-.807.564-1.063.982l-.103.184c-.126.247-.206.562-.248 1.076-.043.523-.043 1.19-.043 2.135v2.664c0 .944 0 1.612.043 2.135.042.515.122.829.248 1.076l.103.184c.256.418.624.758 1.063.982l.2.086c.217.077.49.13.876.162.316.026.685.034 1.136.038zm11.33 7.327c0 .922 0 1.654-.048 2.243-.043.522-.125.977-.305 1.395l-.082.177a4 4 0 0 1-1.473 1.593l-.276.155c-.465.237-.974.338-1.57.387-.59.048-1.322.048-2.244.048H7.833c-.922 0-1.654 0-2.243-.048-.522-.042-.977-.126-1.395-.305l-.176-.082a4 4 0 0 1-1.594-1.473l-.154-.275c-.238-.466-.34-.975-.388-1.572-.048-.589-.048-1.32-.048-2.243V8.663c0-.922 0-1.654.048-2.243.049-.597.15-1.106.388-1.571l.154-.276a4 4 0 0 1 1.594-1.472l.176-.083c.418-.18.873-.263 1.395-.305.589-.048 1.32-.048 2.243-.048h4.334c.922 0 1.654 0 2.243.048.597.049 1.106.15 1.571.388l.276.154a4 4 0 0 1 1.473 1.594l.082.176c.18.418.262.873.305 1.395.048.589.048 1.32.048 2.243zm-10 4.668h4.002c.944 0 1.612 0 2.135-.043.514-.042.829-.122 1.076-.248l.184-.103c.418-.256.758-.624.982-1.063l.086-.2c.077-.217.13-.49.162-.876.043-.523.043-1.19.043-2.135V8.663c0-.944 0-1.612-.043-2.135-.032-.386-.085-.659-.162-.876l-.086-.2a2.67 2.67 0 0 0-.982-1.063l-.184-.103c-.247-.126-.562-.206-1.076-.248-.523-.043-1.19-.043-2.135-.043H8.164L8.165 4z"/>
            </svg>
          </button>
          <a href="/" class="${titleClass}">OpenAgents</a>
        </header>

        <div class="flex min-h-0 flex-1 flex-col overflow-auto">
          ${nav}
        </div>

        <footer class="shrink-0 border-t border-white/10 p-2">
          ${userMenu}
        </footer>
      </aside>
    </div>
  `;
};

export function runAutopilotSidebar(
  container: Element,
  model: AutopilotSidebarModel,
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotSidebarTemplate(model));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse sidebar]", err);
      return Effect.void;
    }),
  );
}
