# Migration from apps/website to apps/web

Everything relevant from `apps/website/` has been moved into `apps/web/` and adapted to use shadcn UI and the Convex + Better Auth stack.

## What was migrated

- **Content**: All `src/content/blog/*.md` and `src/content/kb/*.mdx` (blog posts and Knowledge Base articles).
- **Pages**: Home (index), About, Blog (index + `[...slug]`), Knowledge Base (index + `[...slug]`), Feed, Get API key, Posts (single page at `/posts?id=<id>` for API posts), RSS (`/rss.xml`), Login, Register.
- **Layout**: Single `Layout.astro` with nav (Home, Feed, Knowledge, Blog, About), theme toggle, auth UI, and footer. `BlogPost.astro` layout for blog/kb article pages.
- **Components**: BaseHead, Footer, FormattedDate (and shadcn used throughout).
- **Config**: Content collections (blog, kb), MDX, sitemap, RSS; `consts.ts` (SITE_TITLE, SITE_DESCRIPTION); `lib/api.ts` for OpenAgents API (feed, get-api-key, posts).
- **Assets**: `public/favicon.ico`, `public/fonts/` from website.

## What was not moved

- **Auth**: Web uses Convex + Better Auth (no D1/Cloudflare API route). No `api/auth/[...all].ts` or `migrations/0001_better_auth.sql`.
- **Posts URL**: Website had `/posts/[id]` (server-rendered). Web is static; post detail is `/posts?id=<id>` and loads via client-side fetch from OpenAgents API.

## After migration

You can remove `apps/website/` when ready. Point the main domain (e.g. openagents.com) at the `apps/web` deployment (Cloudflare Pages) and set `site` in `astro.config.mjs` to that URL. Configure `PUBLIC_API_URL` if the API is not same-origin.
