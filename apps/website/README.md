# OpenAgents Website

This is the public website for OpenAgents (Astro + Cloudflare Pages).

## Local development

From repo root:

```bash
pnpm -C apps/website install
pnpm -C apps/website dev
```

Then open `http://localhost:4321`.

## Build

```bash
pnpm -C apps/website build
```

Output goes to `apps/website/dist/`.

## Deploy (Cloudflare Pages)

```bash
pnpm -C apps/website deploy
```

This runs `astro build` then `wrangler pages deploy`.

## Content

- Home: `apps/website/src/pages/index.astro`
- About: `apps/website/src/pages/about.astro`
- Blog posts: `apps/website/src/content/blog/`
- Knowledge Base: `apps/website/src/content/kb/` (rendered under `/kb`)

