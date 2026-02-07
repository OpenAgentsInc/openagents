import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";

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

function isActive(pathname: string, href: string): boolean {
  if (href === "/") return pathname === "/";
  if (href === "/autopilot") return pathname === "/autopilot";
  return pathname.startsWith(href);
}

export function runAutopilotSidebar(
  container: Element,
  model: AutopilotSidebarModel,
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const widthClass = model.collapsed ? "w-12" : "w-64";

    const navItems = [
      { href: "/autopilot", label: "Autopilot", icon: "A" },
      { href: "/modules", label: "Modules", icon: "M" },
      { href: "/tools", label: "Tools", icon: "T" },
      { href: "/signatures", label: "Signatures", icon: "S" },
    ] as const;

    const nav = html`
      <nav class="flex flex-col gap-1 px-2 py-3">
        ${navItems.map((item) => {
          const active = isActive(model.pathname, item.href);
          return html`
            <a
              href="${item.href}"
              class="${cx(
                "flex items-center gap-2 rounded px-2 py-2 text-sm transition-colors",
                "focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus",
                active
                  ? "bg-surface-primary text-text-primary"
                  : "text-text-muted hover:text-text-primary hover:bg-surface-primary",
              )}"
              title="${model.collapsed ? item.label : ""}"
            >
              <span
                class="flex size-6 shrink-0 items-center justify-center rounded border border-border-dark bg-surface-primary/40 text-[10px] text-text-dim"
                aria-hidden
              >
                ${item.icon}
              </span>
              <span class="${model.collapsed ? "hidden" : "block"}">
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
              class="flex size-8 items-center justify-center rounded-full border border-border-dark bg-surface-primary text-xs font-medium text-accent hover:bg-surface-primary/80 focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
              aria-expanded="${model.userMenuOpen ? "true" : "false"}"
              aria-haspopup="true"
              aria-label="User menu"
            >
              ${initials}
            </button>
            ${model.userMenuOpen
              ? html`
                  <div
                    class="absolute bottom-full left-1/2 mb-1 min-w-[120px] -translate-x-1/2 rounded border border-border-dark bg-bg-secondary py-1 shadow-lg"
                    role="menu"
                  >
                    <div class="truncate px-3 py-2 text-xs text-text-dim">${model.user.email ?? ""}</div>
                    <button
                      type="button"
                      role="menuitem"
                      data-ez="autopilot.sidebar.logout"
                      class="w-full px-3 py-2 text-left text-sm text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
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
            class="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
            aria-expanded="${model.userMenuOpen ? "true" : "false"}"
            aria-haspopup="true"
            aria-label="User menu"
          >
            <span
              class="flex size-8 shrink-0 items-center justify-center rounded-full border border-border-dark bg-surface-primary text-xs font-medium text-accent"
              aria-hidden
            >
              ${initials}
            </span>
            <span class="min-w-0 flex-1 truncate text-sm text-text-primary">${displayName}</span>
            <svg
              class="size-4 shrink-0 text-text-dim"
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
                  class="absolute bottom-full left-0 right-0 mb-1 rounded border border-border-dark bg-bg-secondary py-1 shadow-lg"
                  role="menu"
                >
                  <button
                    type="button"
                    role="menuitem"
                    data-ez="autopilot.sidebar.logout"
                    class="w-full px-3 py-2 text-left text-sm text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
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
      "flex flex-1 items-center justify-center text-sm font-semibold text-accent transition-opacity duration-200 ease-linear",
      model.collapsed ? "w-0 overflow-hidden opacity-0" : "opacity-100",
    );

    const content = html`
      <div
        data-autopilot-sidebar-root="1"
        class="${cx(
          "hidden md:flex h-full flex-col shrink-0 border-r border-border-dark bg-bg-secondary text-text-primary transition-[width] duration-200 ease-linear",
          widthClass,
        )}"
      >
        <aside class="flex h-full flex-col" aria-label="Autopilot sidebar">
          <header class="relative flex h-12 shrink-0 flex-row items-center gap-2 border-b border-border-dark px-2">
            <button
              type="button"
              data-ez="autopilot.sidebar.toggleCollapse"
              class="flex size-8 shrink-0 items-center justify-center rounded text-text-muted hover:text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
              aria-label="${model.collapsed ? "Expand sidebar" : "Collapse sidebar"}"
            >
              <svg
                class="size-5"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden
              >
                <rect x="3" y="3" width="7" height="18" rx="1" />
                <rect x="14" y="3" width="7" height="18" rx="1" />
              </svg>
            </button>
            <a href="/" class="${titleClass}">OpenAgents</a>
          </header>

          <div class="flex min-h-0 flex-1 flex-col overflow-auto">
            ${nav}
          </div>

          <footer class="shrink-0 border-t border-border-dark p-2">
            ${userMenu}
          </footer>
        </aside>
      </div>
    `;

    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse sidebar]", err);
      return Effect.void;
    }),
  );
}
