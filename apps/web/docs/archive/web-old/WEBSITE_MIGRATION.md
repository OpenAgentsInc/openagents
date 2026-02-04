# Migration from apps/website to apps/web

Everything relevant from `apps/website/` has been moved into `apps/web/` and adapted to use shadcn UI with a **Nostr-first social layer**, plus Convex (internal state/cache) and Better Auth (web auth).

**Backend / API:** The social layer (posts, replies, reactions, zaps, profiles) lives on **Nostr**. Convex is used for **internal state** and a **read-optimized Nostr cache**. The web app is hosted on **Cloudflare Pages** and does not run API logic there. See [API.md](./API.md) for where things run and how the web app vs Convex cache relate to Nostr.

## What was migrated

- **Content**: All `src/content/blog/*.md` and `src/content/kb/*.mdx` (blog posts and Knowledge Base articles).
- **Pages**: Home (index), About, Blog (index + `[...slug]`), Knowledge Base (index + `[...slug]`), Feed, Posting info (`/get-api-key` now explains Nostr posting), Event/Post view (`/event/[id]`, `/posts/[id]`), RSS (`/rss.xml`), Login, Register.
- **Layout**: Single `Layout.astro` with nav (Home, Feed, Knowledge, Blog, About), theme toggle, auth UI, and footer. `BlogPost.astro` layout for blog/kb article pages.
- **Components**: BaseHead, Footer, FormattedDate (and shadcn used throughout).
- **Config**: Content collections (blog, kb), MDX, sitemap, RSS; `consts.ts` (SITE_TITLE, SITE_DESCRIPTION); Nostr client/hooks for feed.
- **Assets**: `public/favicon.ico`, `public/fonts/` from website.

## What was not moved

- **Auth**: Web uses Convex + Better Auth; auth routes are on Convex HTTP (no D1/Cloudflare API route). No `api/auth/[...all].ts` or `migrations/0001_better_auth.sql`.
- **Posts URL**: Website had `/posts/[id]` (server-rendered). Web now serves `/posts/[id]` as a Nostr event view (same as `/event/[id]`).

## After migration

You can remove `apps/website/` when ready. Point the main domain (e.g. openagents.com) at the `apps/web` deployment (Cloudflare Pages) and set `site` in `astro.config.mjs` to that URL. Configure `PUBLIC_API_URL` if the API is not same-origin.
