---
id: method.web-vitals-layout-stability
version: 1
kind: method
title: Measure and fix Core Web Vitals, especially unexpected layout shifts
summary: >-
  Layout shifts come from content that arrives without reserved space: images
  and embeds without dimensions, late-inserted banners, web-font swaps, and
  animations of layout properties. Reserve space, animate with transforms,
  and measure with the Layout Instability API or Lighthouse in a real browser.
tags: [frontend, performance, web-vitals, layout-shift, lcp, inp, lighthouse, css]
applies_when: >-
  Improving or verifying page performance metrics (layout stability, largest
  contentful paint, interaction latency) in a web app, including React or
  Next.js pages.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "web.dev (Google), Web Vitals; Cumulative Layout Shift (CLS); Optimize CLS; Largest Contentful Paint; Interaction to Next Paint"
    - "W3C Web Performance Working Group, Layout Instability API (LayoutShift entries, hadRecentInput)"
    - "MDN Web Docs, CSS aspect-ratio; @font-face font-display; size-adjust"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Metric definitions.** Each layout shift scores impact fraction times
distance fraction. Shifts within 500 ms of user input are excluded
(`hadRecentInput`). The page's layout-shift metric is the largest *session
window*: shifts less than 1 s apart grouped, each window capped at 5 s. Good
thresholds: layout shift ≤ 0.1, LCP ≤ 2.5 s, INP ≤ 200 ms (75th percentile).

**Causes and fixes.**

- Images, videos, iframes, and ads without size: set `width` and `height`
  attributes (browsers derive the aspect ratio) or CSS `aspect-ratio`, and
  give ad or embed slots a `min-height`.
- Content injected above existing content (cookie banners, notices,
  "load more" results, client-fetched sections): reserve its space with a
  placeholder of the final size, render it in an overlay, or insert it below
  the viewport.
- Web fonts: a fallback font with different metrics reflows text when the
  web font loads. Preload the font, use `font-display: optional`, or match
  fallback metrics with `size-adjust` and related descriptors (framework font
  helpers do this).
- Animations: animate `transform` and `opacity`, not `top`, `height`, or
  `margin`.
- Client-only rendering that replaces server HTML with a different layout on
  hydration also shifts; render the same structure on both sides.

LCP improves by making the largest element's resource discoverable early
(no lazy-loading of the hero image, a preload or high fetch priority) and
reducing render-blocking CSS and JS; INP improves by breaking up long tasks
in event handlers.

## How to check

Measure in a real browser against a production build, not a dev server: run
Lighthouse (`npx lighthouse URL --only-categories=performance`) or load the
page with Playwright and collect `PerformanceObserver` entries of type
`layout-shift` (with `buffered: true`), summing values without recent input
into session windows. Repeat under throttled CPU and network, since
late-arriving resources only shift the layout when they are slow, and repeat
several runs because the numbers vary.
