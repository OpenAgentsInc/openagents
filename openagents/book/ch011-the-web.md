# 11. The web

> **Status: partly implemented.** **Coax Web** (`coax/web.md`), **Coax UI Core**
> (`components/ui/runtime.coax`, `components/ui/policy.coax`,
> `components/backend/dom.coax`), **Coax UI Kit** (`components/ui/kit/`),
> **Coax UI Router** (`components/ui/router.coax`), **Native SSR/Hydration**
> (`components/backend/html.coax`, `coax/web/dom-hydrate.js`,
> `components/ui/test_harness.coax`), and **0-KB-Wasm SSR prerendering** for all
> 8 repository books (`apps/openagents-web/demos/docs-starter/{books,build,page}.coax`)
> pass on `cpu` (`coax/web/ui-core-check.coax`, `coax/web/ui-kit-check.coax`,
> `coax/web/ui-ssr-harness-check.coax`). Remaining: migrating `/chat`
> (`apps/openagents-web/chat-web.coax`) onto the shared app state/actions and
> recording headless Chrome `coax drive` trace parity.

## Premise

Chapter 10 took the app to every native host on the GPU kit. This chapter
brings it to openagents.com, where text surfaces stay real DOM: the chat
page, history and settings from the same state and actions, and the public
pages anyone can read without logging in.

## Draft

The web keeps DOM for text. On a native host the GPU kit draws everything
and the platform lends only an editor and accessibility. In a browser, the
browser already does text input, selection, Find, accessibility and links
well, so a text surface goes to the browser as DOM through
[`components/backend/dom.coax`](../../components/backend/dom.coax)
([Coax Web](../../coax/web.md#decision-2026-11-11-text-surfaces-use-the-browser)),
backed by **Coax UI Core** (`components/ui/runtime.coax` — per-component MVU,
Commands, keyed Subscriptions, Query Cache, Providers, subtree patching),
**Coax UI Kit** (`components/ui/kit/{parts,stream_markdown,virtual_list}.coax`),
**Coax UI Router** (`components/ui/router.coax`), and **Native SSR/Hydration**
(`components/backend/html.coax`, `coax/web/dom-hydrate.js`, plus 0-KB-Wasm
prerendered SSR routes across docs and all 8 books in
[`apps/openagents-web/demos/docs-starter/`](../../apps/openagents-web/demos/docs-starter)).
A DOM page can mount a GPU island that uses the same display-list
components, such as a world or a scene
([rendering.md](../../components/rendering.md#dom-pages-with-gpu-components)).

One library, two presentations. The rules and state of a component live in
`components/core/`; the GPU kit and the DOM kit each keep only their
presentation. Both use the same IDs, roles and actions for the same
component, and a check holds them equal for each family the app uses
([GPU components, "The web"](../../components/gpu-components.md#the-web)).
So the chat page's state, actions and effects from chapters 2 to 9 run in
the browser as Wasm, and `coax drive` gives the same answers in headless
Chrome as on the iPhone.

Every route has a URL. `/chat/ID` is a conversation, `/login` is the login
page, and `/settings/limits` is a settings section. The address bar, the
history and a shared link all work, because the app parses the URL into a
route and writes the route back as a URL. Use "Log in" and `/login`, never
"Sign in". A URL is input, not permission, as chapter 7 says for deep links.

Public pages need no account: the homepage, the docs, the projects and the
download page. A public page works as a web page first. It has a title and a
description that a search engine and a link preview read, headings in
order, real links, and text a screen reader can follow. Each page has a
Markdown twin (add `index.md` to its address), and `/llms.txt` lists the
site for agents. A page and its twin come from one source. The homepage's
Grid is a world in WebGPU and keeps a text version of what it shows.

The download page shows only what exists: a platform with no build gets no
button, and a store listing appears when the store has approved it, not when
the build is uploaded.

Today the site mixes eras. The homepage is the Grid in
[`newland.coax`](../../apps/openagents-web/newland.coax), and `/chat` is
[`chat-web.coax`](../../apps/openagents-web/chat-web.coax), with live models
behind it and its own state. This chapter moves `/chat` onto the shared
state and actions, and the text pages onto the app's routes, one page at a
time, keeping every URL that works today.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chat page, history and settings on the web from the shared state and
  actions, drawn as DOM by the DOM kit.
- URL codecs for every route.
- The docs, projects and download pages as routes, each with one source for
  the page, its Markdown twin, its title and its description.
- A check that holds the DOM and GPU presentations of each family to the
  same IDs, roles and actions.

It builds on what exists today:

- [`coax/web.md`](../../coax/web.md)
- [`components/ui/runtime.coax`](../../components/ui/runtime.coax) and [`components/ui/policy.coax`](../../components/ui/policy.coax) (Coax UI Core)
- [`components/ui/kit/parts.coax`](../../components/ui/kit/parts.coax), [`components/ui/kit/stream_markdown.coax`](../../components/ui/kit/stream_markdown.coax), and [`components/ui/kit/virtual_list.coax`](../../components/ui/kit/virtual_list.coax) (Coax UI Kit)
- [`components/ui/router.coax`](../../components/ui/router.coax) (Coax UI Router)
- [`components/backend/dom.coax`](../../components/backend/dom.coax), [`components/backend/html.coax`](../../components/backend/html.coax), [`coax/web/dom-hydrate.js`](../../coax/web/dom-hydrate.js), and [`components/ui/test_harness.coax`](../../components/ui/test_harness.coax) (Native SSR/Hydration & zero-browser Story/Scene/Play harness)
- [`apps/openagents-web/demos/docs-starter/books.coax`](../../apps/openagents-web/demos/docs-starter/books.coax) and [`apps/openagents-web/demos/docs-starter/build.coax`](../../apps/openagents-web/demos/docs-starter/build.coax) (0-KB-Wasm SSR books)
- [`components/kit/frames.coax`](../../components/kit/frames.coax)
- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`apps/openagents-web/README.md`](../../apps/openagents-web/README.md)
- [`apps/openagents-web/chat-web.coax`](../../apps/openagents-web/chat-web.coax)
- [`apps/openagents-web/newland.coax`](../../apps/openagents-web/newland.coax)
- [`apps/openagents-web/download`](../../apps/openagents-web/download)

### The chapter's check

- **Source:** [`components/ui/runtime.coax`](../../components/ui/runtime.coax),
  [`components/ui/router.coax`](../../components/ui/router.coax),
  [`components/backend/dom.coax`](../../components/backend/dom.coax),
  [`components/backend/html.coax`](../../components/backend/html.coax),
  [`components/ui/test_harness.coax`](../../components/ui/test_harness.coax),
  and [`apps/openagents-web/demos/docs-starter/books.coax`](../../apps/openagents-web/demos/docs-starter/books.coax)
  (remaining: shared `/chat` web target on `openagents/examples/cases/chat-page.ops`).
- **Trace:** `coax/web/ui-core-check.coax`, `coax/web/ui-kit-check.coax`, and
  `coax/web/ui-ssr-harness-check.coax` pass on `cpu`, and `docs-starter/build.coax`
  emits prerendered 0-KB-Wasm SSR HTML across docs and all 8 books with 0 broken
  `/books/*` routes (remaining: headless Chrome `coax drive` trace parity for `/chat`).
- **Refusal:** `/chat/ID` with no session opens the login page with the
  conversation as its return target; `books.coax` refuses overflow entries cleanly
  (292/292 mutants killed); a page with no title, a skipped heading level or a
  link with no text fails the build with the page and the element named.
- **Independent check:** `coax/web/dom-hydrate.test.js` verifies browser hydration
  against native Coax HTML output; every page's Markdown twin and its drawn text
  give the same headings and links, and every URL in today's sitemap still answers.

Planned paths: none. The site is
[`apps/openagents-web`](../../apps/openagents-web).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, the download page
- **Also:**
  - [Coax Web](../../coax/web.md)
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md), sections 10 and 11: text surfaces use the browser
  - [Fast loads](../../docs/metaverse/fast-loads.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P13, P16
