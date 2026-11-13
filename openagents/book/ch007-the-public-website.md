# 7. The public website

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Part I ended with a small app that runs on the web and the iPhone. This
chapter builds the part of openagents.com that anyone can read without
logging in: the homepage, the docs, the projects and the download page.

## Draft

A public page has to work as a web page before it works as an app. It has a
URL you can share, a title and a description that a search engine and a link
preview read, headings in order, links that are real links, and text that a
screen reader can follow. The browser's back button and a new tab behave as
you expect.

On the web, Coax Native draws text surfaces as DOM and worlds on the GPU
([Coax Native Web](../../coax/web.md#decision-2026-11-11-text-surfaces-use-the-browser)).
A docs page, a project page and the download page are text surfaces, so the
browser lays out and selects their text. The homepage's Grid is a world, so
it draws in WebGPU, and it keeps a text version of what it shows for people
and programs that can't see the canvas.

Each page already has a Markdown twin: add `index.md` to a page's address,
and `/llms.txt` lists the site for agents. That stays. A page and its twin
come from one source, so they can't disagree.

The site today mixes the two eras. The homepage at `/` is the Grid in
WebGPU, in [`newland.coax`](../../apps/openagents-web/newland.coax). The
docs, projects and download pages are built pages in
[`apps/openagents-web`](../../apps/openagents-web). This chapter moves the
text pages onto the app's routes and components from Part I, one page at a
time, and keeps every URL that works today.

The download page says what you can install and on which platform. It shows
only what exists: a platform with no build gets no button, and a store
listing appears when the store has approved it, not when the build is
uploaded (chapter 76).

## What we build

A plan for what this chapter ships once the draft is approved:

- The docs, projects and download pages as routes of the app, drawn with the
  DOM backend from `components/kit/`.
- One source per page for the page, its Markdown twin, its title, its
  description and its preview image.
- The homepage's text version, built from the same data as the Grid.

It builds on what exists today:

- [`apps/openagents-web/README.md`](../../apps/openagents-web/README.md)
- [`apps/openagents-web/newland.coax`](../../apps/openagents-web/newland.coax)
- [`apps/openagents-web/docs`](../../apps/openagents-web/docs)
- [`apps/openagents-web/projects`](../../apps/openagents-web/projects)
- [`apps/openagents-web/download`](../../apps/openagents-web/download)
- [`apps/openagents-web/robots.txt`](../../apps/openagents-web/robots.txt)
- [`components/backend/dom.coax`](../../components/backend/dom.coax)
- [`coax/web.md`](../../coax/web.md)

### The chapter's check

- **Source:** the page routes in `apps/openagents-web/`, over chapter 4's
  route codec.
- **Trace:** `coax drive tree` for `/docs`, `/projects` and `/download`,
  printing each page's title, its heading outline and its links.
- **Refusal:** a page with no title, a skipped heading level or a link with
  no text fails the build with the page and the element named.
- **Independent check:** every page's Markdown twin and its drawn text give
  the same headings and links, and every URL in today's sitemap still
  answers.

Planned paths: none. The pages live in
[`apps/openagents-web`](../../apps/openagents-web).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, the download page
- **Also:**
  - [Coax Native Web](../../coax/web.md)
  - [Fast loads](../../docs/metaverse/fast-loads.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16
