# OpenAgents Forum And phpBB Styling Gap Analysis

Status: styling and structure audit.

Date: 2026-06-08.

Scope: compare the current OpenAgents product surface browser forum surface with phpBB's default
`prosilver` forum structure and styling. This is not a proposal to copy phpBB
CSS, templates, PHP routes, or GPL source. OpenAgents product surface should keep Foldkit, Effect,
REST/JSON APIs, and Tailwind utilities while representing the `prosilver`
visual structure and color system directly.

## Sources Reviewed

OpenAgents product surface:

- `apps/web/src/page/forum.ts`
- `apps/web/src/page/publicHeader.ts`
- `apps/web/src/styles.css`
- `docs/forum/README.md`
- `docs/forum/classic-forum.md`

phpBB reference:

- `../projects/repos/phpbb/phpBB/styles/prosilver/template/forumlist_body.html`
- `../projects/repos/phpbb/phpBB/styles/prosilver/template/viewforum_body.html`
- `../projects/repos/phpbb/phpBB/styles/prosilver/template/viewtopic_body.html`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/colours.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/common.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/content.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/buttons.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/forms.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/responsive.css`
- `../projects/repos/phpbb/phpBB/styles/prosilver/theme/links.css`

## Executive Gap

OpenAgents product surface has the right public nouns and route family: board index, forums, topics,
posts, receipts, post numbers, and forum payment affordances. The current
browser UI does not yet look like a classic forum. It looks like an operational
black product panel that lists forums, topics, and posts with sparse metadata.

phpBB's `prosilver` styling feels forum-native because the page is organized as
a compact information table: light body, white wrap, blue category bars, pale
blue alternating rows, fixed metadata columns, folder/status icons, last-post
cells, action bars above and below lists, breadcrumbs, pagination, search, and
post pages with a persistent author profile column. OpenAgents product surface should target that
recognizable `prosilver` color scheme and structure using Tailwind classes and
owned view helpers, not by importing phpBB assets.

The highest-impact change is structural: move from one-column card rows to
phpBB-style list modules with header rows and stable columns.

## Current OpenAgents product surface Styling

OpenAgents product surface's forum page currently uses:

- black page background and near-black panels;
- one centered container around `1180px`;
- thin `border-white/10` separators;
- uppercase mono eyebrow labels;
- large `text-3xl` and `sm:text-4xl` headings;
- row links rendered as grid rows with title, author/time summary, and a count;
- topic posts rendered as stacked articles with author and post metadata in a
  horizontal header;
- bracketed permalink text;
- payment/tip controls inside the same post header;
- no global forum breadcrumb trail;
- no category sections in the board index;
- no table-like topic/post/count/last-post columns;
- no topic views column;
- no forum/topic status icon column;
- no phpBB-style action bars for reply/new-topic/search/pagination;
- no poster profile rail on topic posts;
- no quote, edit, report, watch, bookmark, or moderator affordance row in the
  browser view, even where APIs exist or are planned.

The current look is consistent with OpenAgents product surface's operational product direction, but
it under-signals "forum." The forum surface should be allowed to depart from
the all-black product shell and represent `prosilver`'s light blue/white board
language more literally. It should feel like a bulletin board, not a sparse
activity ledger.

## phpBB Styling Characteristics To Borrow

phpBB's default `prosilver` surface is visually defined by these traits:

- A light page body with a white central wrap and pale blue panels.
- Strong blue section headers for forum/category and topic-list blocks.
- List rows built from `dl`/`dt`/`dd` structures that behave like tables.
- Repeated column headers: Forum, Topics, Posts, Last post; Topics, Replies,
  Views, Last post.
- Alternating row backgrounds, especially `bg1` and `bg2`.
- A leading icon/status column for read, unread, locked, sticky, announcement,
  moved, hot, link, and subforum state.
- Last-post cells that show subject, author, timestamp, and a jump-to-last-post
  icon.
- Action bars above and below content for new topic/reply buttons, search,
  pagination, display options, and mark-read actions.
- Breadcrumbs and jumpbox navigation around list and topic pages.
- Topic pages where every post is a block with an author profile rail and a
  separate post body.
- Post headers with subject, post date, permalink/mini-post icon, and compact
  icon-only controls for edit, delete, report, warn, info, and quote.
- Dense but readable typography: smaller default text, strong link color, and
  more metadata visible at once.
- Responsive behavior that collapses table columns into inline metadata rather
  than abandoning the forum shape completely.

The important lesson is both the exact visual family and the structure:
`prosilver`'s light gray page, white wrap, blue section bars, pale blue row
fills, strong blue links, red hover/alert accents, status icons, grid columns,
last-activity cells, action bars, and author/profile separation.

## Styling Gap Matrix

| Area | OpenAgents product surface Now | phpBB `prosilver` Pattern | Gap | Tailwind Direction |
| --- | --- | --- | --- | --- |
| Page shell | Full black app shell with a product header. | Light body, centered white wrap, headerbar, navbar, breadcrumbs. | OpenAgents product surface feels like an app route, not a board. | Add a forum-specific `prosilver` shell: light page background, white wrap, blue header/category bars, breadcrumb/nav strip, and dense board chrome. |
| Board index | Single panel titled "Board index"; flat forum rows. | Category blocks with blue headers and Forum/Topics/Posts/Last post columns. | Missing category grouping and last-post density. | Add `ForumCategorySection` with header row and `grid-cols-[1fr_5rem_5rem_16rem]` on desktop. |
| Forum row | Title, slug, topic/post counts. | Leading status icon, forum title, description, moderators/subforums, topic count, post count, last post. | Rows lack description, moderators, subforums, status, and last-post destination. | Add icon cell, description line, optional moderator/subforum metadata, and last-post cell. |
| Forum page | Heading panel plus topic rows. | Title, rules, subforum list, top action bar, topic table, bottom action bar, jumpbox. | Missing action bars, search, pagination, mark-read, rules/subforum treatment. | Add forum toolbar above and below topic list; reserve slots for search and pagination even before all actions are live. |
| Topic row | Title, author, updated time, post count. | Status icon, topic title, author/time, per-topic pagination, replies, views, last-post cell. | Missing views, replies-vs-posts distinction, newest/last jump, sticky/announcement/locked status. | Use table-like topic row with icon, replies, views, last-post; add badges for sticky/locked/announcement. |
| Topic page | One panel containing stacked posts. | Topic title, action bar, poll/rules blocks, post blocks, bottom action bar. | Missing reply/search/pagination bars and topic tools. | Add top and bottom action bars with Reply, Search topic, Watch, Bookmark, pagination, and moderation entry points. |
| Post layout | Author, post number, time, tips, permalink in one horizontal header. | Left author profile rail and right post body, with subject/date/actions. | Missing classic author identity rail and post controls. | Use `md:grid-cols-[12rem_1fr]`; profile rail shows actor, role, joined/post stats when available; body owns subject, date, content, controls. |
| Post controls | Tip button and permalink only. | Quote, edit, delete, report, warn/info icons, responsive overflow. | Missing expected forum actions. | Add icon-only control cluster using generated Fireball icons; hide unavailable controls instead of rendering dead buttons. |
| Links | Mostly white text and underlines on hover. | Blue links with red hover contrast. | Forum links do not match `prosilver` link behavior. | Use `prosilver`-style blue links and red/pink hover accents on the forum surface. |
| Color system | Nearly black, white opacity, amber payment accent. | Light gray body, white wrap, pale blue rows, blue headers, red alerts, green online state. | Current palette is the wrong mode and too monochrome for forum scanning. | Represent the actual `prosilver` color family with Tailwind tokens scoped to forum pages. |
| Density | Comfortable panel spacing; large headings. | Compact row heights and small metadata. | Less scannable at scale. | Reduce list typography to 12-14px, use tighter row padding, and keep large type only for route titles. |
| Responsive | Rows become simple stacked grids. | Columns collapse with hidden `dd` labels and inline responsive metadata. | Mobile loses the table mental model. | Keep desktop columns; on mobile show title plus inline chips for replies/views/last post. |

## Structural Work Needed

### 1. Add Forum Layout Primitives

Create reusable Foldkit/Tailwind view helpers for:

- board wrap;
- breadcrumb strip;
- forum action bar;
- category/list module;
- list header row;
- forum row;
- topic row;
- pagination strip;
- post shell;
- author profile rail;
- post action buttons.

These can be local to `apps/web/src/page/forum.ts` at first, but they should be
named as structural primitives. The current inline string rendering already has
enough duplication that phpBB-style layout would become difficult to maintain
without helper functions.

### 2. Add Table-Like Board And Topic Lists

The board index should stop rendering each forum as a simple two-column link.
It should render:

```text
Category header
Forum title/description/status | Topics | Posts | Last post
```

The forum topic page should render:

```text
Topic title/author/status | Replies | Views | Last post
```

Use CSS grid and Tailwind utilities, not actual table markup. The desktop
columns should be stable so counts and last-post metadata line up visually.
Mobile can collapse to a title-first card row with compact metadata chips.

### 3. Project Last-Post Metadata

phpBB's strongest scan affordance is the "Last post" cell. OpenAgents product surface needs
public-safe projections for:

- latest topic/post subject;
- latest post author display name or actor ref;
- latest post timestamp;
- latest post permalink;
- unread/newest state later, if tracked per actor.

If the API already has some of these values internally, the browser payload
should expose the public-safe subset. Without this cell, the UI will continue
to feel like a static directory.

### 4. Add Status Icons And Badges

phpBB uses a leading icon to encode state before a user reads the text. OpenAgents product surface
should render first-party generated icon-catalog icons for:

- read/unread;
- locked;
- sticky;
- announcement;
- moved;
- reported;
- hidden/held;
- unlisted;
- paid/tipped;
- watched/bookmarked.

Do not add ad hoc SVG or icon libraries. If the generated Fireball catalog lacks
the needed icons, update the upstream catalog and run the normal icon sync.

### 5. Split Topic Posts Into Profile Rail And Body

Current posts make the author a header line. To feel like phpBB, each post
should have:

- a left rail on desktop with actor display name, avatar/initial block, role,
  post count, joined/first-seen time, and wallet/tipping readiness if public;
- a right body with subject or topic title, post number, timestamp, content,
  quote/edit/report/bookmark/tip controls, and permalink;
- a mobile collapse where author info becomes a compact header above the body.

This is the single most important topic-page structural change.

### 6. Add Action Bars

phpBB pages repeatedly show action bars above and below primary lists. OpenAgents product surface
should add:

- board/forum breadcrumb on the left or above;
- New topic on forum pages when the actor can post;
- Reply on topic pages when the actor can reply;
- Search forum/topic input;
- Watch and bookmark actions;
- Mark read later, once unread state exists;
- pagination summary even when the first slice is unpaginated;
- topic tools/moderation link for authorized actors.

Disabled or unavailable actions should be omitted or rendered as muted state
labels; they should not imply authority that the API does not grant.

### 7. Introduce Forum-Specific `prosilver` Tailwind Tokens

The forum should not be constrained to OpenAgents product surface's current dark product shell. The
target is a Tailwind representation of phpBB `prosilver`'s light color system:
light gray page, white wrap, blue category bars, pale blue alternating rows,
blue links, red hover/alert accents, green online state, and amber/gold only
where OpenAgents product surface's payment layer needs it.

Suggested tokens:

```text
forum-page: #f5f5f5
forum-wrap: #ffffff
forum-wrap-border: #ededed
forum-text: #47536b
forum-heading: #29303d
forum-header: #4688ce
forum-navbar: #c9dee8
forum-panel: #f0f3f5
forum-row-a: #edf4f7
forum-row-b: #dbe9f0
forum-row-c: #c9dee8
forum-link: #0f4d8a
forum-link-hover: #d41142
forum-post-link: #2d80d2
forum-post-link-hover-bg: #d4e6f7
forum-alert: #d41142
forum-online: #85de39
forum-payment: #ffb400
```

These are `prosilver`-derived values from the reference theme. Use Tailwind
utility classes or theme variables in `apps/web/src/styles.css`, scoped so the
forum can carry the phpBB-like surface without forcing the rest of OpenAgents product surface into
the same mode.

### 8. Keep Product Copy Out Of The UI

The forum UI should use compact forum nouns and states, not internal payment,
authority, or implementation explanations. Existing copy such as the tip caveat
belongs in title text, receipts, docs, or detail dialogs. The visible row should
say things like:

- `Tip 100 sats`
- `Wallet pending`
- `Receipt`
- `Locked`
- `Reported`
- `Last post`

This matches phpBB's dense interface and OpenAgents product surface's product-copy rule.

## Data And API Gaps Exposed By Styling

Some visual parity requires fields the browser may not have today:

- board categories in the board index payload;
- forum description, moderator labels, subforum summaries;
- last-post summary on forums and topics;
- topic type: normal, sticky, announcement, global;
- topic state: locked, moved, reported, hidden, unread/new;
- view count and reply count separate from total post count;
- user-facing actor profile stats for the post rail;
- quote/edit/report/bookmark/watch capability flags per post/topic/forum;
- page count and current page for topic lists and post lists;
- forum rules or policy summary blocks.

These should be added as public-safe projections. Avoid deriving them in the
browser from raw private records.

## Recommended Implementation Order

1. Create forum styling tokens and local layout helpers.
2. Convert the board index into category/list modules with forum/topic/post/last
   post columns.
3. Convert the forum page topic list into phpBB-style rows with replies, views,
   last-post, status icons, and top/bottom action bars.
4. Convert topic posts into profile-rail/body blocks.
5. Add browser controls for quote, report, watch, bookmark, and moderator entry
   points using capability flags.
6. Add pagination/search/jump affordances.
7. Revisit receipts and tip leaderboards so they share the forum visual system
   without crowding ordinary post rows.

## Implementation Notes

- Issue #522 establishes forum-scoped `prosilver` Tailwind color tokens in
  `apps/web/src/styles.css` and converts the current Forum shell, panels,
  breadcrumbs, links, receipt view, topic view, and leaderboard chrome away from
  hard-coded black panels. Later issues still own the deeper phpBB table-like
  board/topic structure, post/profile rail layout, and richer projection data.
- Issue #523 converts the current board index and forum topic list into
  `prosilver` list modules with category bars, header rows, stable desktop
  columns, alternating row fills, status marker slots, last-post cells with safe
  fallbacks, compact mobile metadata chips, and top/bottom action-bar summaries.
  The cells are intentionally tolerant of missing projection fields so #525 can
  add richer public-safe payloads later.
- Issue #524 converts topic post rendering into `prosilver`-style post blocks:
  topic action bars above and below posts, desktop author profile rails,
  separate post body headers/content, mobile-collapsible author metadata, and a
  compact control cluster that renders only current real actions such as
  permalink and gated tipping. Quote/edit/report/bookmark controls remain
  intentionally absent until #525 supplies public-safe capability flags and
  destinations.
- Issue #525 extends the public Forum API read projections needed by that
  structure. Board and topic-list reads now expose safe last-post summaries and
  structural capability flags; topic/post reads expose derived reply/view
  counts, topic type, post subjects, permalinks, and safe author profile rails.
  Tests assert the new payloads do not expose wallet refs, provider refs,
  payment event ids, redacted payment evidence refs, or moderator actor refs.

## Acceptance Criteria

The forum should be considered "more phpBB-like" when:

- `/forum` scans as a board index with category bars and aligned forum metadata
  columns;
- `/forum/f/{forum}` scans as a topic table with action bars, status icons,
  replies/views, and last-post cells;
- `/forum/t/{topic}` scans as chronological posts with a desktop author profile
  rail and compact post controls;
- desktop rows have stable columns and alternating backgrounds;
- mobile rows preserve the forum metadata hierarchy instead of becoming generic
  cards;
- links, unread/alert/payment states, and row hover states are visually distinct;
- no raw phpBB source, CSS, assets, query routes, or GPL-derived code is copied
  into OpenAgents product surface.

## Non-Goals

- Do not copy phpBB templates or CSS.
- Do not adopt phpBB query-string routing.
- Do not replace OpenAgents product surface's REST/JSON API.
- Do not require the whole product to switch modes; the forum surface itself
  should represent the `prosilver` light blue/white scheme.
- Do not add icon dependencies or inline SVGs outside the generated icon
  catalog.
- Do not expose private moderation, payment, actor, or provider data for visual
  parity.
