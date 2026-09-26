---
id: tool.nextjs-production-build-checks
version: 1
kind: tool
title: Judge Next.js performance on a production build, and keep client JavaScript small
summary: >-
  Next.js dev mode is unoptimized, so build and measure with next build and
  next start. Keep client components at the leaves, load heavy client-only
  code with next/dynamic, size images with next/image, self-host fonts with
  next/font, and read the build's per-route size report.
tags: [nextjs, react, frontend, performance, bundle-size, ssr, images, fonts]
applies_when: >-
  Optimizing, fixing, or verifying a Next.js application's load performance,
  bundle size, rendering mode, or build output.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Next.js documentation: Server and Client Components ('use client' boundaries)"
    - "Next.js documentation: Optimizing Images (next/image: width, height, sizes, priority), Fonts (next/font), Lazy Loading (next/dynamic)"
    - "Next.js documentation: next build output, next start; @next/bundle-analyzer"
    - "Next.js documentation: Caching and Revalidating (fetch cache options, revalidate), noting defaults differ between major versions"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Measure the right thing.** `next dev` compiles on demand without
minification; timing and size measured there are meaningless. Run
`next build` (depending on the version it also type-checks and lints; it
prints each route and whether it is static or dynamic, and in many versions
its size and first-load JavaScript) and serve with `next start` before measuring.

**Client JavaScript.** In the App Router, components are server components by
default; a `"use client"` file and everything it imports ships to the
browser. Push `"use client"` down to the small interactive leaves and pass
server-rendered children into them. Load heavy client-only libraries
(charts, editors, maps) with `next/dynamic(() => import(...), { ssr: false })`
from a client component (recent versions reject `ssr: false` in server
components), or load them on interaction. Avoid importing a whole library for one function, and
check `@next/bundle-analyzer` output for duplicates.

**Images and fonts.** `next/image` needs `width` and `height` (or `fill` with
`sizes`) so space is reserved and the right size is served; mark the
above-the-fold hero image `priority` so it is not lazy-loaded. `next/font`
self-hosts fonts and generates fallback metrics to avoid reflow.

**Data.** Fetch on the server where possible instead of in a client
`useEffect` waterfall; fetch independent data in parallel (`Promise.all`).
Caching and revalidation defaults for `fetch` and route segments changed
across major versions, so read the installed version (`package.json`) and its
docs before relying on a default; set `cache` or `revalidate` explicitly when
freshness matters.

**Common build breakers.** Browser-only APIs (`window`, `localStorage`) used
during server rendering, hooks in server components, and non-serializable
props passed from server to client components.

## How to check

Compare `next build` route sizes before and after a change, then measure the
production server with Lighthouse or a Playwright script under throttling,
several runs each. Confirm the page renders the same content with JavaScript
disabled where it is meant to be server-rendered.
