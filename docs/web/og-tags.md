# Link previews (Open Graph tags)

What a page shows when its link is shared in a chat app or a feed: an image,
a title, a line and the address. The workbench at
[openagents.com/og](https://openagents.com/og) shows every page's preview at
once, with what is wrong under each.

## What we had (production, 2026-10-30)

Checked with `curl` against openagents.com and the staged site (67 pages).

| Pages | og:title / description / url | og:image | twitter:card | Notes |
|---|---|---|---|---|
| 15 realm pages (`realms.json`), the Garden, `/verse`, `/verse/galaxy` | yes | yes, 1200x630 | large image | 4 realms have their own card (Everglade, Fairy Forest, Hecate's Cemetery, Ice Cream Man). The other 11, the Garden, `/verse` and the galaxy all showed one picture, `/verse/og-verse.jpg`: the Haunted House's. 8 realm descriptions ran 200 to 291 characters, so previews cut them off mid-sentence. |
| `/realms/new`, `/realms/characters` | yes | none | small or none | A text-only preview. |
| Homepage, `/projects` and its 9 project pages, `/pylon`, `/download`, `/docs/connect-a-pylon`, `/nip` and its 22 NIP pages | none | none | none | Chat apps fell back to the `<title>` and description, with no picture. `/download` had no description either. |
| `/login`, `/chat`, `/admin/*`, `/new-land`, the 404, private and demo project pages | none | none | none | All `noindex`; they still get shared. |
| Made realms (`/realms/<slug>` from `/realms/new`, written by oa-api, `0048_realm_builder.sql`) | title, url | the Haunted House picture | none | No description tag. |
| A project's first realm (`0040_project_realm.sql`) | yes | its drop's cover, when there is one | large or small | Fine. |

How they were made: realm pages are written by `coax/web/realms.ts` from
`realms.json` (`title`, `description`, `alt`, and `card` when there's a
hand-made one, else the Verse picture). Every other page was hand-written HTML
with whatever tags its author added, and no check looked at them.

## What happens now

**Every page gets complete tags at release, without anyone writing them.**
`coax/web/og.ts` runs on the staged site (`deploy.sh`, before anything is
uploaded) and fills in whatever a page lacks, from what the page already says:

- **title**: `og:title`, else its `<title>` without " · OpenAgents".
- **line**: `og:description`, else its description, cut to 200 characters at
  a sentence (or a word, with an ellipsis).
- **address**: `og:url`, else `https://openagents.com/<its path>`.
- **image**: in this order: the page's own `og:image`; a hand-made `og.jpg`
  or `card.jpg` in its directory; its rendered card `og/<path>.jpg`; the
  branded default `og/default.jpg`.
- `og:type`, `og:site_name`, the image's size and alt text, and
  `twitter:card` (large image), `twitter:title`, `twitter:description` and
  `twitter:image` follow from those.

**A release refuses a page whose preview is broken** (exit 1, naming each):
no title, no description on an indexed page, an image that isn't on the site
or isn't a JPEG or PNG at 1200x630, a rendered card over 300 KB, and a realm
or project still on the branded default. Warnings (shown on `/og`, they
don't stop a release): a title over 60 characters, a description over 200 or
under 50, `twitter:card` set to the small preview, a picture shared by
several pages, the branded default on an ordinary page.

**Rendered cards, per kind of page** (`coax/web/og-render.ts`, outputs
committed in `apps/openagents-web/og/`):

- **Realm**: a still of the realm itself, shot in headless WebGPU in golden
  mode, under its name. A realm with a hand-made `card` in `realms.json`
  keeps it (the Haunted House keeps `/verse/og-verse.jpg`). A realm's line is
  its `share` field when it has one (20 to 200 characters), else its
  description, shortened.
- **Project** (`projects/<slug>/project.json`): its name, its line, and a
  bar of tasks done out of all of them (from `projects/list.coax`).
- **Any other page**: its title and line in the site's line-art style.
- **Default** (`og/default.jpg`): the name and the homepage's line, for a
  page made since the last render.

Each card is `/og#card=PATH` (the workbench's own app, `og.coax`, drawing one
page's card at 1200x630) captured headless and saved as a JPEG under 300 KB.

## Adding a page, a realm or a project

- **A page**: give it a `<title>` and a `<meta name="description">`. That's
  all it needs to ship; it shares the branded card until its own is rendered.
- **A realm**: its `realms.json` entry as now; add a `share` line if its
  description runs long. The release refuses it until it has a card: run
  `bun coax/web/og-render.ts` (it stages a draft site, renders every missing
  card, about 10 seconds a page), look at the result on `/og`, commit
  `apps/openagents-web/og/`. Or set `card` to a hand-made 1200x630 JPEG.
- **A project**: the same command renders its card. `--all` renders every
  rendered card again, which moves the projects' done bars along.

`COAXIAL_OG_DRAFT=1` stages a site with the refusals as warnings (og-render
uses it). `bun coax/web/og.ts --report apps/openagents-web` prints every
page's preview from the sources, without a build.

## Next

- Made realms (`/realms/<slug>` from the builder) and private project pages
  are written by oa-api, not the site build: they should take the same rules
  (a description tag, twitter tags, and a card that isn't the Haunted
  House's). That is a migration replacing `verse.builder_page`.
- Hand-made cards for the realms people share most (Dead Gears, Orbit
  League, Belt 2200), from a staged moment (`?stage=highlight`,
  `docs/verse/clips.md`) instead of the first frame.
- Render cards at release time instead of committing them, once the release
  machine runs headless Chrome reliably, so a project's bar is never stale.
