import { html } from "@openagentsinc/effuse";
import type { TemplateResult } from "@openagentsinc/effuse";
import type { Session } from "../effect/atoms/session";

function getDisplayLabel(session: Session): string {
  if (!session.user) return "Not logged in";
  const u = session.user;
  const full = [u.firstName, u.lastName].filter(Boolean).join(" ").trim();
  return (full || u.email) ?? "Account";
}

/**
 * Fixed bottom-left pill showing current user (or "Not logged in") and optional Log out.
 * Rendered into a dedicated container from boot; EZ "app.identity.logout" handles sign out.
 */
export function identityPillTemplate(session: Session): TemplateResult {
  const label = getDisplayLabel(session);
  const isLoggedIn = session.user != null;

  return html`
    <div
      data-identity-pill="1"
      class="flex items-center gap-2 rounded-lg border border-white/10 bg-black/80 px-3 py-2 text-xs font-mono text-white/90 shadow-lg backdrop-blur-sm"
      title="${isLoggedIn ? session.user?.email ?? "" : "Sign in at /login"}"
    >
      <span class="truncate max-w-[180px]">${label}</span>
      ${isLoggedIn
        ? html`
            <button
              type="button"
              data-ez="app.identity.logout"
              class="shrink-0 rounded px-1.5 py-0.5 text-white/60 hover:bg-white/10 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            >
              Log out
            </button>
          `
        : html`
            <a
              href="/login"
              class="shrink-0 rounded px-1.5 py-0.5 text-white/60 hover:bg-white/10 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
            >
              Sign in
            </a>
          `}
    </div>
  `;
}
