# GPUI everywhere on openagents.com

Recorded 2026-10-20, at `15067682c6`. Owner request: "Convert the entire web
UI into GPUI except for the simple pages like Privacy etc. Flip that gpui
mode to prod mode, deprecate the rest. Anything not fully ready like
payments, hide or put behind a feature flag. Then deploy everything to
production."

This document lists every route `crates/openagents-web` serves, classifies
each group as SIMPLE, APP, or RETIRE, audits which user-facing features are
ready, defines the feature flags that hide the rest, and sets the flip plan
for #12226, the deploy checklist, and the order of work.

It builds on the [GPUI web chat plan](2026-10-19-gpui-web-chat.md) (phases
0a to 6, #12216 to #12226) and the
[desktop and web parity plan](2026-10-20-desktop-web-parity.md) (phases 7 to
13, #12262 to #12294). Those plans put the chat, the sidebar, computers,
cloud computers, Settings (General, Appearance, Notifications, Shortcuts,
Files, Archived, Earn, Your ID), check-ins, files, and the work panel into
the GPUI app. This plan covers the pages they didn't.

- [Summary](#summary)
- [Classes](#classes)
- [Route inventory](#route-inventory)
- [Feature readiness and flags](#feature-readiness-and-flags)
- [Flip plan for #12226](#flip-plan-for-12226)
- [Deprecation list](#deprecation-list)
- [Production deploy checklist](#production-deploy-checklist)
- [Order of work](#order-of-work)
- [Issues](#issues)

## Summary

- The site mounts about 790 routes. About 120 are pages a person opens; the
  rest are JSON APIs, assets, and HTMX fragments.
- **Already GPUI:** `/gpui` and, with `?ui=gpui`, `/`, `/new`, and
  `/chat/{id}`; `/agent/{id}` and `/agent/foreman` (AgentGraph); `/pylons`
  (the pylon card); `/demo/gpui`; and the landers that embed GPUI cards.
  `/verse`, `/grid`, and `/everglade` are their own wgpu apps, not GPUI.
- **APP pages still in server HTML:** the rest of Settings (11 pages),
  projects, the standalone work addresses, Lab document pages, CAD, and the
  admin pages. Each group has an issue below, sized for one session.
- **SIMPLE pages stay server HTML:** legal, docs, blog, download, roadmap,
  profiles, shared chats and traces, sign-in, the well-known files, and
  every page that must work without JavaScript or be indexed.
- **Not ready, now hidden:** the browser wallet, Earn, billing and checkout,
  the affiliate program, the CAD parts shop, the live payment pages, agent
  replies, Verse hands, and moving a chat to a cloud computer. One registry,
  `crates/openagents-web/src/features.rs`, holds them, and
  `OPENAGENTS_WEB_FEATURES` turns them on (#12336).
- **The flip:** GPUI becomes the default for every APP route on desktop
  browsers with WebGPU. Browsers without WebGPU, JavaScript-off visitors,
  and phones keep the HTML chat as the fallback until #12297 (the WebGL2
  renderer) and a phone check land. The HTMX chat stops being a primary UI
  but isn't deleted, because the GPUI page's text view and side panel are
  drawn from it.

## Classes

- **SIMPLE.** Stays server HTML: legal, docs, status, blog, well-known and
  agent-readable files, error pages, sign-in, and anything that must work
  without JavaScript or be indexed. Public profiles, shared chats, shared
  traces, and event pages stay HTML for search and link previews; a GPUI
  layer is optional and not planned.
- **APP.** Becomes GPUI. A signed-in tool: chat, Settings, projects, work,
  Labs, CAD, admin.
- **RETIRE.** Dead or duplicate; redirect for one release, then 404.
- **WASM.** Its own non-GPUI WebAssembly app (Verse, games, the sealed
  inference demo). Unchanged by this plan.
- **API.** JSON, assets, or HTMX fragments. Not a page; listed for
  completeness.

## Route inventory

Paths are under `crates/openagents-web/src/`. "Who": out (signed out),
in (signed in), admin (site admin), local (the local address only). "Tech":
HTML (server HTML, maud, often HTMX), GPUI, WASM, JSON.

### Chat and the front door

| Routes | What | Who | Tech today | Class | Issue |
| --- | --- | --- | --- | --- | --- |
| `/`, `/new` (`pages/home.rs`) | New chat and the home page | out, in | HTML; GPUI with `?ui=gpui` or the `oa_chat_ui` cookie | APP | #12226 |
| `/chat/{id}` and `/chat/{id}/*` (`pages/chat*.rs`, `chat_files.rs`, `chat_export.rs`, `github_tools/`, `lab.rs`, `game_lab.rs`, `cad_lab.rs`, `robot_lab.rs`) | One chat, its stream, and its side surfaces | out (visitor), in | HTML plus HTMX; GPUI with the flag | APP | #12226, #12316 |
| `/chat/sessions/{id}` (`pages/chat_sessions.rs`) | A computer session's summary | in | HTML | APP | #12224 |
| `/chat/list`, `/chat/archived`, `/chats/events`, `/composer/*`, `/composer/row/*`, `/chat/assets/*` | Sidebar and composer fragments | out, in | HTMX fragments, SSE | API | #12337 after #12226 |
| `/gpui`, `/gpui/chat/{id}`, `/gpui/side`, `/gpui/text`, `/gpui/build/*` (`pages/gpui_chat.rs`) | The GPUI chat on its own address, its side panel and text view | out, in | GPUI | APP (done) | #12337 redirects `/gpui` after #12226 |
| `/home` (`pages/home.rs`) | The classic home under the front door | out, in | HTML | RETIRE | #12337 |
| `/lander3` to `/lander11`, `/landing` (`pages/lander*.rs`, `pages/landing.rs`) | Earlier landing experiments | out | HTML and GPUI cards | RETIRE | #12337 |
| `/demo`, `/demo/{chat}` (`demo/`) | Scripted HTML demo | out | HTML | RETIRE | #12337 |
| `/demo/gpui` (`pages/gpui_demo.rs`) | GPUI demo with offline chats | out | GPUI | APP (done) | |
| `/ask` (`ask.rs`) | Ask endpoint behind the composer | out, in | JSON | API | |

### Settings and account

| Routes | What | Who | Tech today | Class | Issue |
| --- | --- | --- | --- | --- | --- |
| `/settings` (`settings.rs`) | Profile, theme, Claude credential, plan, chats, computers, affiliate | in | HTML | APP | #12325 |
| `/settings/claude`, `/settings/claude/subscription` (`settings/subscription.rs`, `settings/link.rs`) | Add or remove a Claude credential or subscription | in | HTML | APP | #12325 |
| `/settings/export`, `/chat/delete-all` | Export data, delete all chats | in | HTML | APP | #12325 |
| `/settings/plan/*` (`plan.rs`) | Pro plan, extra hours, Stripe checkout | in | HTML | APP, hidden (`billing`) | #12336 |
| `/settings/api-keys*` (`api_keys.rs`) | API keys | in | HTML | APP | #12326 |
| `/settings/sign-in-keys*` (`sign_in_keys.rs`) | Sign-in keys | in | HTML | APP | #12326 |
| `/settings/agent-accounts*` (`agent_accounts.rs`) | Agent accounts | in | HTML | APP | #12326 |
| `/settings/memory*` (`account_memory.rs`) | Memory | in | HTML | APP | #12327 |
| `/settings/schedules*` (`account_schedules.rs`) | Scheduled prompts | in | HTML | APP | #12327 |
| `/settings/connections*`, `/auth/google/*` (`connections/`) | Google connections | in | HTML; OAuth hops stay server routes | APP | #12328 |
| `/settings/vault`, `/projects/{id}/vault`, `/vault/*` (`vault/`) | The vault | in | HTML plus its own wasm | APP | #12328 |
| `/settings/terminal*` (`terminal_connect.rs`) | Approve a terminal sign-in | in | HTML | APP | #12328 |
| `/settings/traces*` (`traces.rs`) | The account's traces | in | HTML | APP | #12329 |
| `/settings/agents*` (`agents_page.rs`) | Running agents and the land queue | in | HTML | APP | #12330 |
| `/settings/mac-jobs*` (`mac_jobs_page.rs`) | Mac jobs | in | HTML; the GPUI work panel draws them | APP | #12332 |
| `/settings/computers/remove` (`device.rs`) | Remove a computer | in | HTML form | APP (GPUI computers page, #12280) | |
| `/account/avatar` (`account.rs`) | The account's picture | in | image | API | |
| `/@{name}`, `/u/{login}`, `/event/{id}`, profile edit (`profiles.rs`, `pages/profile.rs`) | Public profiles and Nostr events | out, in | HTML | SIMPLE | |

### Work, projects, Labs, CAD

| Routes | What | Who | Tech today | Class | Issue |
| --- | --- | --- | --- | --- | --- |
| `/environments*` (`environments/`) | Set up a repository and run Claude Code | in (allowed) | HTML, SSE | APP | #12332 |
| `/work`, `/work/{id}` (`work_runs.rs`) | Work runs | in | HTML | APP | #12332 |
| `/projects*` (`projects/`, `connections/sources.rs`) | Projects, members, repositories, sources | in | HTML | APP | #12331 |
| `/labs/agent`, `/labs/agent/tasks/{id}` (`agent_lab_page.rs`) | Agent Lab | in | HTML with the Rust Native browser adapter | APP | #12285 |
| `/labs/{kind}/{document}`, `/labs/files/{id}/play`, `/labs/agent/{id}/card` (`lab_card_view.rs`, `agent_card_page.rs`) | Lab documents | in | HTML plus card scripts | APP | #12333 |
| `/cad`, `/cad/{file}`, `/cad/{file}/configurator`, `/cad-world` (`pages/cad.rs`, `cad_web*`) | CAD Lab | in (Pro) | HTML plus the CAD wasm viewer | APP | #12334 |
| `/cad-config/{token}` | A shared configurable design | out | HTML plus viewer | SIMPLE | |
| `/cad-shop*`, `/cad-order/*`, `/cad-purchases`, `/cad/{file}/unlist` (`cad_web/sales/`) | Selling CAD parts for sats | out, in | HTML | APP, hidden (`cad_shop`) | #12336 |
| `/app`, `/app/tasks/{id}`, `/app/purchases*` (`tasks.rs`, `purchases.rs`) | Local task and purchase browser | local | HTML | SIMPLE (local developer tool) | |

### Explore, agents, pylons, Verse, games

| Routes | What | Who | Tech today | Class | Issue |
| --- | --- | --- | --- | --- | --- |
| `/explore` (`profiles/explore.rs`) | Agents, plugins, skills, pylons from relay events | out, in | HTML | SIMPLE (a public, indexed directory; links into GPUI) | |
| `/agent/foreman`, `/agent/{id}` (`pages/foreman.rs`) | AgentGraph and agent pages | out, in | GPUI with first-frame painter | APP (done) | |
| `/pylons`, `/pylons/live` (`pages/pylons.rs`) | Pylons online now | out | GPUI card over HTML | APP (done) | |
| `/verse`, `/grid`, `/everglade`, `/druid` (`pages/everglade.rs`) | Verse and the Grid | out, in | WASM (wgpu) | WASM | |
| `/games/*`, `/studios/blue-rush*` | Blue Rush games and studio site | out | WASM and HTML | WASM / SIMPLE | |
| `/att` (`pages/att.rs`) | Sealed inference demo | out | WASM | WASM | |
| `/layertree/*`, `/gpui-fonts/*`, `/agent-graph/*` | Build assets | out | assets | API | |

### Content, sign-in, and machine-readable files

| Routes | What | Who | Tech today | Class |
| --- | --- | --- | --- | --- |
| `/privacy`, `/terms`, `/contact` | Legal and contact | out | HTML | SIMPLE |
| `/docs*`, `/docs/api*`, `/docs/agents*`, `/docs/sdk*` | Documentation | out | HTML | SIMPLE |
| `/blog*`, `/roadmap`, `/promises`, `/efficiency` | Blog, roadmap, promises, the efficiency study | out | HTML | SIMPLE |
| `/download`, `/desktop`, `/install`, `/cli/install.sh`, `/cli/install.ps1` | Downloads | out | HTML, scripts | SIMPLE |
| `/connect`, `/.well-known/apple-app-site-association`, `/.well-known/assetlinks.json` | Phone pairing link | out | HTML | SIMPLE |
| `/pricing` | Pricing | out | HTML, or a redirect to `/docs/pricing` | SIMPLE, hidden (`billing`) |
| `/affiliates` | Affiliate terms | out | HTML | SIMPLE, hidden (`affiliates`) |
| `/live`, `/stats` | Live payment picture and payment numbers | out | HTML canvas, tables | SIMPLE, hidden (`payment_stats`) |
| `/pilot`, `/pilot/install` | Pilot intake | out | HTML (404 without a configured intake) | SIMPLE |
| `/login`, `/signup`, `/login/key*`, `/login/scan`, `/sign-in`, `/sign-out`, `/device*`, `/auth/github*`, `/oauth/*` | Sign-in (WorkOS work is #12320 to #12324) | out | HTML | SIMPLE |
| `/share/{token}`, `/chat/{id}/share` | A shared chat | out | HTML | SIMPLE |
| `/trace/{id}` | A shared trace | out | HTML | SIMPLE |
| `/.well-known/*`, `/llms.txt`, `/llms-full.txt`, `/*.md` twins, `/robots.txt`, `/sitemap.xml`, `/openapi.json`, `/security.txt`, `/skill.md` | Agent-readable files | out | text, JSON | SIMPLE |
| `/og/*` | Link preview images | out | images | SIMPLE |
| `/go/{code}`, `/r/{token}` | Attribution links | out | redirects | SIMPLE |
| `/cloud`, `/cloud/app*` | Former Cloud page | out | redirects | RETIRE |
| 404 page (`not_found`) | Errors | out | HTML | SIMPLE |

### Admin and developer pages

| Routes | What | Who | Tech today | Class | Issue |
| --- | --- | --- | --- | --- | --- |
| `/admin/analytics`, `/admin/attribution`, `/admin/access/drive-connections`, `/board`, `/limited`, `/revoked` | Analytics, attribution, Drive access, issue board | admin | HTML | APP | #12335 |
| `/ui`, `/ui/screens*`, `/components*` | Component catalog and screen gallery | out | HTML | SIMPLE (developer tool); prune with #12337 |

### APIs

Every `/v1/*`, `/api/*`, `/coder/*`, `/mcp*`, and asset route is API. The
GPUI app is a client of them through the cookie bridge
(`cookie_api.rs`, #12262). Old API addresses keep answering (AGENTS.md).

The gateway's own `/dashboard` and `/playground` (`crates/gateway`) are
internal operator pages, not served on openagents.com (both answer 404
there). They're out of scope; the account's API keys move with #12326.

## Feature readiness and flags

### Signals used

- Open issues for the feature.
- [After-close checks](2026-10-16-master-delegation.md#after-close-checks)
  that haven't passed.
- `~/work/NEEDS_OWNER.md` items.
- Whether production has the configuration the feature needs (read from
  the live `coder` service spec on 2026-10-20).
- Whether the path has ever run outside tests.

### Not ready: hidden by default

| Feature (flag name) | What it hides when off | Why it isn't ready |
| --- | --- | --- |
| `wallet` | `/v1/wallet/browser*` answers 404 | U5's phone approvals (#12306) and the Wallet > Browse device checks (#12310, #12308) haven't run; the Lightning address (#12307) is open; production has no `OPENAGENTS_WEB_AGENT_KEY_SEED`, from which the grantee key comes |
| `earn` | The Earn page in GPUI Settings | #12294's scratch-host check (turn sharing on and off, read earnings) hasn't run |
| `billing` | Settings' plan section; `/settings/plan/*` answers 404; `/pricing` goes to the pricing guide | Production has no `--plan-checkout`, so Subscribe can't work; the trial `/pricing` promises isn't built (`docs/launch/1.0/claims.md`) |
| `affiliates` | `/affiliates` answers 404; Settings' "Your affiliate earnings" is left out | Payouts come from Stripe sales, and Stripe test mode isn't set up (#11727, After-close "to do") |
| `cad_shop` | `/cad-shop*`, `/cad-order/*`, `/cad-purchases`, `/cad/{file}/unlist` answer 404; the CAD page's shop links and "Sell this version" are left out; a `sell` post answers 404 | Lightning checkout has never run outside tests |
| `payment_stats` | `/live` and `/stats` answer 404 and leave the sitemap | They picture real payments, and production shows 0 sats received (`/api/stats`, 2026-10-20). `/api/stats` and `/api/flow/*` keep answering |
| `agent_replies` | Published agents don't answer room mentions or messages | #12256's staging check hasn't run; the agent key seed isn't set |
| `verse_hands` | The Hands button on `/verse` | #12309's Chrome check hasn't run |
| `move_to_cloud` | "Move to a cloud computer" in the GPUI chat's row menu ("Bring back" stays) | #12282's scratch-host check hasn't run |

### Ready enough to show, with checks still listed

These stay on; their checks are in After-close checks and the deploy
checklist below. A failure there opens a new issue and, if it's serious,
adds the feature to the registry.

- The GPUI chat, sidebar, computers, cloud computers, Settings pages, files
  panel, work panel, and check-ins (#12221 to #12225, #12262 to #12291).
- `/explore` (#12259), profiles and NIP-05 names (#12254), agent pages
  (#12255).
- `/pylons`: its payment column shows only reports from
  `OPENAGENTS_PYLON_PAYMENT_ISSUERS`, which is empty in production, so it
  shows no payments already.
- x402 and the other ways to pay per request appear only in docs, and only
  while the gateway lists them (`payments.rs`).

### Already off by configuration

- **Room tokens.** Production had no `OPENAGENTS_WEB_ROOM_TOKEN_KEY` on
  2026-10-20, so Coder chats over `coder-sync` rooms and the registry
  sidebar fell back to the web chats. `c403fee919` (#12263) makes
  `scripts/deploy/web.sh promote` add the key from Secret Manager, so the
  next production deploy turns room tokens on. It's a deploy step, not a
  flag; #12224's check covers it.
- **Front door.** `OPENAGENTS_WEB_FRONT_DOOR=on` (#12260) stays off until
  the owner confirms.
- **WorkOS sign-in.** Another agent builds it behind `OPENAGENTS_AUTH`
  (#12320 to #12324); it's not in this registry.
- **Dictation** (#12292), the **providers page** (#12283), and the GPUI
  **Agent Lab** (#12285) landed on 2026-10-20 while this plan was written.
  They act only on the person's own paired computer and move no money, so
  they stay shown with their checks in After-close checks, as the other
  parity pages do.
- **The voice stage** (#12293), the command palette (#12279), agent visuals
  (#12287), and transcript extras (#12270) have no web controls yet, so
  there's nothing to hide.

### The mechanism

`crates/openagents-web/src/features.rs`:

- `Feature` lists each not-ready feature, with its name and the routes it
  owns.
- `OPENAGENTS_WEB_FEATURES` is a comma list of names to turn on. `all` turns
  every one on, and `-name` turns one off after `all`. Unknown names are
  logged at start and ignored. In production the variable is unset, so
  every feature is off.
- The older switches still work: `OPENAGENTS_WEB_AGENT_REPLIES=on` turns on
  `agent_replies`, and `OPENAGENTS_PRICING_PAGE=1` turns on `billing`.
- `features::gate`, just inside the host guard, answers 404 for a hidden
  feature's routes: the site's not-found page, or a JSON `not_found` under
  `/v1/`.
- Pages check `app.config.features` and leave hidden controls out instead
  of showing them disabled (AGENTS.md: hide what doesn't work).
- The GPUI chat page carries an `openagents-features` meta tag with the
  features that are on. `openagents_web_client::features::on` reads it, so
  the chat and the Settings module hide the same things. A page without
  the tag (an older server) hides nothing.
- A `Config` built in code (`Config::development`, which tests use) has
  every feature on, so each feature's own tests keep running.
- A feature leaves the registry when it's ready: delete its variant and
  its checks.

To try a feature on staging: set `OPENAGENTS_WEB_FEATURES=billing,earn` (for
example) on the `web` container of `openagents-web-1-staging`, then remove it.

## Flip plan for #12226

### What becomes the default

GPUI becomes the default renderer for every APP route on desktop browsers
with a WebGPU adapter: `/`, `/new`, `/chat/{id}`, and, as their issues land,
the Settings, projects, work, Labs, CAD, and admin addresses.

1. In `pages/gpui_chat.rs::choice`, make "no query and no cookie" mean
   GPUI, and keep `?ui=html` (once) and `?ui=off` (remember HTML) as the
   escape hatches. Put the default behind one switch so rollback is a
   config change: a registry entry `gpui_default`, on by default once the
   checks pass, so `OPENAGENTS_WEB_FEATURES=-gpui_default` is the rollback.
2. The start script already sends a browser without WebGPU to the HTML
   chat (`?ui=html`). Keep that.
3. Phones keep the HTML chat until a phase passes on iOS Safari and
   Android Chrome (the chat plan's phone rule). Decide by user agent in
   `choice`, as `pages/first_frame.rs` does for the phone frame.

### What browsers without WebGPU get

**Recommendation: keep the HTML chat (the current HTMX chat) as the
fallback until #12297 lands, and don't build a separate reduced view.**

- The text view (`/gpui/text`, #12220) is a read-only transcript with no
  composer, and the side panel's "Send with files" is the HTML chat's own
  composer. A reduced fallback would be the HTML chat with parts removed,
  which is more work than keeping it.
- The HTML chat already streams, sends, and stops. People without WebGPU
  lose nothing they have today.
- Once #12297's WebGL2 renderer draws the chat, the fallback shrinks to
  JavaScript-off visitors, and the HTML chat can drop to read and send
  only.

### What "remove the HTMX chat path" means

- The HTML chat stops being the default and stops getting new features.
  New chat features go to GPUI only.
- Its fragments that only the HTML chat's own navigation used
  (`/composer/row/*` pickers, `/chat/list` sidebar swaps) are removed once
  the fallback no longer needs them (#12337).
- The transcript renderer, the stream, and the composer stay: the GPUI
  page's text view, side panel, and the fallback all use them.
- `/share/{token}` stays server HTML.

### Checklist

Posted on #12226 as its checklist:

- [ ] `gpui_default` switch in the registry, with tests for the default,
  `?ui=html`, `?ui=off`, a phone user agent, and an HTMX request.
- [ ] Staging: signed-in test account and a signed-out visitor each start a
  chat at `/`, get an answer, reload during an answer, rename, pin,
  archive, and delete, in Chrome, Safari, and Firefox.
- [ ] Staging: a browser without WebGPU (Chrome with `--disable-webgpu`)
  lands on the HTML chat and can send.
- [ ] Staging: a phone user agent gets the HTML chat.
- [ ] Production for a week with no open regression, then remove the
  `oa_chat_ui` cookie's "gpui" requirement from docs and the parity table.
- [ ] Remove the HTML-only fragments listed in #12337.

## Deprecation list

| Page | Replacement | When | Issue |
| --- | --- | --- | --- |
| `/lander3` to `/lander11`, `/landing` | `/` (front door) | When `OPENAGENTS_WEB_FRONT_DOOR` is on in production | #12337 |
| `/home` | `/` | Same | #12337 |
| `/demo`, `/demo/{chat}` | `/demo/gpui` | Now | #12337 |
| `/gpui`, `/gpui/chat/{id}` | `/`, `/chat/{id}` | After #12226 | #12337 |
| `/cloud`, `/cloud/app*` redirects | none | One release after this deploy | #12337 |
| HTML Settings pages | GPUI Settings pages | As each of #12325 to #12330 lands; the HTML stays as the fallback | each issue |
| `/settings/mac-jobs*`, `/environments*`, `/work/{id}` HTML | The GPUI work panel | #12332 | #12332 |
| HTMX chat as the default | GPUI chat | #12226 | #12226 |
| `/earn`, `/forum`, `/gym`, `/traces`, `/weights`, `/qa`, `/doc` | none | Already 404 (`upstream::REMOVED`) | |

`/lander9/hands/*` is still loaded by `/verse`; move it under `/verse/`
before deleting `/lander9`.

## Production deploy checklist

Use `scripts/deploy/web.sh` ([runbook](../deployment/openagents-web.md)).
Deploy only from a commit rebased on current `origin/main`. Another agent
deploys the web backlog; check `gcloud run services describe coder` first,
and if their candidate holds the `new` tag, use `WEB_DEPLOY_TAG` with a
different tag.

1. **Stage.** `scripts/deploy/web.sh stage --keep-spec origin/main`. It
   builds the image, swaps it into staging, and runs
   `scripts/smoke/staging.sh`.
2. **Staging smoke for the flags.** On the staging host:
   - `GET /v1/wallet/browser`, `/affiliates`, `/cad-shop`, `/live`,
     `/stats` answer 404; `/pricing` redirects to `/docs/pricing`.
   - `/sitemap.xml` lists neither `/live` nor `/stats`.
   - `/gpui` carries `<meta name="openagents-features" content="">`.
3. **Signed-in staging chat check.** As the dedicated test account (never
   the owner's), open `/?ui=gpui`, send a message, get an answer, open
   Settings and confirm there's no Earn page and no Plan section, and open
   a computer chat's row menu and confirm there's no "Move to a cloud
   computer". Delete the chat afterward.
4. **Promote.** `scripts/deploy/web.sh promote DIGEST`, which smokes the
   `new` tag with `--candidate`.
5. **Shift.** `scripts/deploy/web.sh shift`, which runs the production
   smoke after traffic moves.
6. **Production smoke.** Repeat step 2 against `https://openagents.com`,
   and check `/`, `/chat/{id}` for the test account, `/docs`, `/privacy`,
   `/verse`, and `/explore` answer 200.
7. **Rollback.** `scripts/deploy/web.sh rollback` (the revision the last
   promote recorded), or
   `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions REVISION=100`
   with the revision that served before. To bring a hidden feature back
   without a rollback, set `OPENAGENTS_WEB_FEATURES` on the `web`
   container.

## Order of work

1. **Feature flags (#12336).** Done first, so production stops showing
   unready features. Deployed with this document.
2. **#12226 flip.** Needs #12216 (the shared transcript, in progress by
   another agent) and the checklist above. The `gpui_default` switch can
   land before the views finish.
3. **Settings groups (#12325 to #12330)** in parallel; each is one session
   and touches its own HTML module and one Settings page.
4. **Projects (#12331), work addresses (#12332), Lab documents (#12333).**
   #12333 builds on #12316's cards.
5. **CAD (#12334) and admin (#12335).**
6. **Retire (#12337)** once the front door is on and #12226 has run a week.

Each feature in the registry leaves it when its own check passes: the
issue that owns the check deletes the variant.

## Issues

| Group | Issue |
| --- | --- |
| Settings: the `/settings` index, Claude credential, data | [#12325](https://github.com/OpenAgentsInc/openagents/issues/12325) |
| Settings: API keys, sign-in keys, agent accounts | [#12326](https://github.com/OpenAgentsInc/openagents/issues/12326) |
| Settings: memory and schedules | [#12327](https://github.com/OpenAgentsInc/openagents/issues/12327) |
| Settings: connections, vault, terminal connect | [#12328](https://github.com/OpenAgentsInc/openagents/issues/12328) |
| Traces list and viewer | [#12329](https://github.com/OpenAgentsInc/openagents/issues/12329) |
| Agents page and land queue | [#12330](https://github.com/OpenAgentsInc/openagents/issues/12330) |
| Projects | [#12331](https://github.com/OpenAgentsInc/openagents/issues/12331) |
| Environments, work runs, Mac jobs addresses | [#12332](https://github.com/OpenAgentsInc/openagents/issues/12332) |
| Lab document pages | [#12333](https://github.com/OpenAgentsInc/openagents/issues/12333) |
| CAD pages | [#12334](https://github.com/OpenAgentsInc/openagents/issues/12334) |
| Admin pages | [#12335](https://github.com/OpenAgentsInc/openagents/issues/12335) |
| Feature flags: hide unready features | [#12336](https://github.com/OpenAgentsInc/openagents/issues/12336) |
| Retire old pages | [#12337](https://github.com/OpenAgentsInc/openagents/issues/12337) |
| Flip and HTMX deprecation (checklist comment) | [#12226](https://github.com/OpenAgentsInc/openagents/issues/12226) |
| Already filed: Agent Lab | [#12285](https://github.com/OpenAgentsInc/openagents/issues/12285) |
| Already filed: cards in the canvas | [#12316](https://github.com/OpenAgentsInc/openagents/issues/12316) |
| Already filed: WebGL2 renderer | [#12297](https://github.com/OpenAgentsInc/openagents/issues/12297) |
