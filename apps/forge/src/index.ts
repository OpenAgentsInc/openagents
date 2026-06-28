import { Effect, Schema } from "effect"

import {
  autopilotCoreDarkCssVars,
  autopilotCoreDarkTokens,
  oaTokens,
} from "@openagentsinc/ui/tokens"

export const FORGE_UI_WORKER_VERSION = "forge-ui.2026-06-28.6759"

export const ForgeMount = Schema.Struct({
  product: Schema.Literal("forge"),
  host: Schema.Literal("forge.openagents.com"),
  basePath: Schema.Literal("/"),
  runtime: Schema.Literal("cloudflare-worker"),
  uiPackage: Schema.Literal("@openagentsinc/ui"),
})

export type ForgeMount = typeof ForgeMount.Type

export const defaultForgeMount: ForgeMount = {
  product: "forge",
  host: "forge.openagents.com",
  basePath: "/",
  runtime: "cloudflare-worker",
  uiPackage: "@openagentsinc/ui",
}

export const forgeLandingCopy = {
  title: "THE FORGE",
  tagline: "where agents git it on",
} as const

const escapeHtml = (value: string): string =>
  value.replace(/[&<>"']/g, character => {
    switch (character) {
      case "&":
        return "&amp;"
      case "<":
        return "&lt;"
      case ">":
        return "&gt;"
      case '"':
        return "&quot;"
      case "'":
        return "&#39;"
      default:
        return character
    }
  })

const cssDeclarations = (input: Record<string, string>): string =>
  Object.entries(input)
    .map(([name, value]) => `  ${name}: ${value};`)
    .join("\n")

const sharedTokenCss = cssDeclarations({
  ...autopilotCoreDarkCssVars(autopilotCoreDarkTokens),
  "--forge-accent": oaTokens.color.accent,
  "--forge-accent-soft": oaTokens.color.accentSoft,
  "--forge-surface": oaTokens.color.componentSurface,
  "--forge-border": oaTokens.color.componentBorderStrong,
  "--forge-text-bright": oaTokens.color.textBright,
  "--forge-text-muted": oaTokens.color.textMuted,
  "--forge-radius": oaTokens.radius.xl,
  "--forge-font-mono": oaTokens.font.mono,
})

export const forgeLandingStyles = `:root {
${sharedTokenCss}
  color-scheme: dark;
}

* {
  box-sizing: border-box;
}

html,
body {
  min-height: 100%;
  margin: 0;
}

body {
  min-height: 100dvh;
  background: var(--bg);
  color: var(--text);
  font-family: InterVariable, Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  -webkit-font-smoothing: antialiased;
  text-rendering: geometricPrecision;
}

.forge-page {
  position: relative;
  display: grid;
  min-height: 100dvh;
  place-items: center;
  isolation: isolate;
  overflow: hidden;
  padding: 2rem;
  background: #000;
}

.forge-page::before,
.forge-page::after {
  position: absolute;
  inset: 0;
  z-index: -1;
  content: "";
  pointer-events: none;
}

.forge-page::before {
  background-image:
    repeating-linear-gradient(90deg, rgba(245, 183, 58, 0.12) 0 1px, transparent 1px 48px),
    repeating-linear-gradient(0deg, rgba(245, 183, 58, 0.08) 0 1px, transparent 1px 48px);
  opacity: 0.86;
}

.forge-page::after {
  border: 1px solid rgba(245, 183, 58, 0.24);
  margin: 1rem;
}

.forge-hero {
  display: grid;
  width: min(100%, 56rem);
  gap: 1rem;
}

.forge-kicker {
  margin: 0;
  color: var(--forge-accent-soft);
  font-family: var(--forge-font-mono);
  font-size: 1rem;
}

.forge-title {
  max-width: 10ch;
  margin: 0;
  color: var(--forge-text-bright);
  font-size: 4rem;
  font-weight: 600;
  line-height: 1;
  letter-spacing: 0;
  text-wrap: balance;
}

.forge-tagline {
  max-width: 28ch;
  margin: 0;
  color: var(--text);
  font-family: var(--forge-font-mono);
  font-size: 1.5rem;
  line-height: 1.35;
  text-wrap: pretty;
}

@media (min-width: 48rem) {
  .forge-page {
    padding: 4rem;
  }

  .forge-page::after {
    margin: 1.5rem;
  }

  .forge-title {
    font-size: 7rem;
  }

  .forge-tagline {
    font-size: 1.75rem;
  }
}

@media (min-width: 80rem) {
  .forge-title {
    font-size: 9rem;
  }
}`

export const forgeLandingBody = (): string => `<main data-ui-family="forge/landing" data-forge-app="landing" data-shared-ui-package="${escapeHtml(defaultForgeMount.uiPackage)}" class="forge-page">
  <section class="forge-hero">
    <p class="forge-kicker">${escapeHtml(defaultForgeMount.host)}</p>
    <h1 class="forge-title">${escapeHtml(forgeLandingCopy.title)}</h1>
    <p class="forge-tagline">${escapeHtml(forgeLandingCopy.tagline)}</p>
  </section>
</main>`

export const renderForgeLandingHtml = (): string => `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${forgeLandingCopy.title}</title>
  <meta name="description" content="${forgeLandingCopy.tagline}">
  <style>
${forgeLandingStyles}
  </style>
</head>
<body>
${forgeLandingBody()}
</body>
</html>`

const htmlResponse = (body: string): Response =>
  new Response(body, {
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    },
  })

const jsonResponse = (body: unknown): Response =>
  Response.json(body, {
    headers: {
      "cache-control": "no-store",
    },
  })

export const handleForgeRequest = (
  request: Request,
): Effect.Effect<Response> =>
  Effect.gen(function* () {
    const url = new URL(request.url)
    const isHead = request.method === "HEAD"
    const isGet = request.method === "GET" || isHead

    if (isGet && url.pathname === "/") {
      return htmlResponse(isHead ? "" : renderForgeLandingHtml())
    }

    if (isGet && url.pathname === "/health") {
      return jsonResponse({
        ok: true,
        service: "openagents-forge",
        version: FORGE_UI_WORKER_VERSION,
        mount: defaultForgeMount,
      })
    }

    if (isGet && url.pathname === "/version") {
      return jsonResponse({
        service: "openagents-forge",
        version: FORGE_UI_WORKER_VERSION,
      })
    }

    return new Response("not found", {
      status: 404,
      headers: {
        "content-type": "text/plain; charset=utf-8",
        "cache-control": "no-store",
      },
    })
  })

export default {
  fetch(request: Request): Promise<Response> {
    return Effect.runPromise(handleForgeRequest(request))
  },
}
