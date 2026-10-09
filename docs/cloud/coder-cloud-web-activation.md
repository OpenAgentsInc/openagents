# Coder Cloud web: packaging, reconnect acceptance, and activation record

> **Removed 2026-10-08.** The `/cloud/app` pages this describes were deleted; see [the Cloud reset](../web/cloud-reset.md).

This is the WEB-17 ([#10963](https://github.com/OpenAgentsInc/openagents/issues/10963))
record for the browser Cloud workspace that WEB-01 to WEB-16 built in
`crates/openagents-web` (roadmap [#10964](https://github.com/OpenAgentsInc/openagents/issues/10964);
specification [Coder Cloud](coder-cloud.md)). It lists how the app is packaged,
what reconnect and browser acceptance was measured on 2026-10-08, and, for each
page or lane, whether it is code-complete on `main` and which owner-only step
still stands before it is offered. Status comes from the issues and the
`NEEDS_OWNER.md` entries named below. Code-complete means merged with its own
checks. It does not mean deployed, qualified, or available. Nothing here
activates a lane.

## Packaging

The whole Cloud app is one binary, `openagents-web` (Axum, Maud, HTMX, and
SSE), plus generated Rust/Wasm assets. There is no TypeScript and no private
sibling backend. Each route belongs to one Rust module under
`crates/openagents-web/src/cloud/`. A lane appears only when its own explicit
configuration is loaded. Otherwise its navigation entry reads **Unavailable**.

| Configuration | Enables | Without it |
| --- | --- | --- |
| `--cloud-config PRIVATE_JSON` and `--cloud-build DIRECTORY` | `/cloud/sign-in`, `/cloud/app`, the session shell, Settings, and Verse public connections (`mod.rs`, `session.rs`) | `/cloud` explains that the workspace is unavailable. |
| `--cloud-hosts PRIVATE_JSON` | Tasks, Projects, Computers, Agents, operator Cloud jobs, Environment, Workbench, and Verse joins (`work.rs`, `controls.rs`, `operator.rs`, `environment.rs`, `agents.rs`, `workbench.rs`, `verse.rs`) | These sections say no connection is admitted. |
| `--cloud-retail PRIVATE_JSON` | Billing → Retail and Purchases (`retail.rs`, `retail/purchases.rs`, `custody.rs`) | The Retail lane is absent. |
| `--cloud-sales PRIVATE_JSON` | Sales, Sales modules, the sales floor, and the sales-owner part of Partners (`sales.rs`, `sales_views.rs`, `partners.rs`) | Sales is unavailable. Partners shows only the viewer's own account records. |
| `--cloud-team PRIVATE_JSON` | Team lanes listed in the owner's browser qualification (`team.rs`) | Team is unavailable. |
| `--cloud-byo PRIVATE_DIR` with `--cloud-byo-keys PRIVATE_JSON` (or `OPENAGENTS_WEB_CLOUD_BYO_KEYS`) | Settings → Manage Claude credential (`byo.rs`), encrypted at rest (#11041) | The section is absent. `--cloud-byo` without a keyring refuses to start. |
| `--everglade DIRECTORY` | The chamber renderer that Verse Join opens | Join is unavailable. Verse connections still show. |

Billing statements and decision resources (`billing.rs`) and Partners
(`partners.rs`) use the viewer's own native session. They need only a selected
workspace.

`/cloud/assets/` serves the Cloud stylesheets, `start.js`, and the four
generated files in `BUILD_ASSETS`: the privacy runtime (`coder_cloud_web`) and
the granted workbench terminal (`coder_browser_web`). Before WEB-17, the site
images built only the privacy runtime, so a deployed workbench would have loaded
a missing module. `Dockerfile` and `Dockerfile.components` now build both crates
into `/srv/cloud` and check all four files. The test
`site_images_package_every_served_cloud_build_asset` keeps the served list and
both images in step. Merging builds no image and deploys nothing.

## Reconnect and browser acceptance

Every private page waits for a current server standing check before it shows
anything. Its content retires when standing changes, when the page is hidden,
when a request or observation stream fails, or when the session expires.
Returning needs navigation, and fresh admission follows. Observation streams
(`/cloud/app/hosts/{binding}/watch` and `/cloud/app/team/watch`) send each
snapshot digest as the SSE event id. WEB-17 adds two things:

- **Resume.** A reconnect that carries `Last-Event-ID` picks up from that id. An
  unchanged snapshot stays quiet. A change made while the browser was detached
  arrives as a `gap` or `refresh` event; it is never adopted as the new baseline
  unseen. The team stream previously had no ids, so a change made during a drop
  was silently lost.
- **Retire on reconnect.** If a reconnect can no longer be admitted (session
  expired, membership or role changed, host revoked, another account, or a
  foreign or malformed id), the server answers with one `retire` event instead
  of an error status. On an error status, `EventSource` and `htmx-sse` would keep
  retrying while the page still said it was observing.

Tests: `reconnect::resident_watch_resumes_from_last_event_id_and_retires_on_lost_admission`,
`team_watch_resumes_from_last_event_id_and_retires_on_lost_admission`, and
`workspace_shell_keeps_keyboard_and_screen_reader_basics` in
`crates/openagents-web/src/cloud/`. Pending-request recovery after a refresh is
covered by each lane's journaled-request tests (WEB-04 sealed results, WEB-05
operator requests, WEB-08 Alice requests, WEB-09/10 retail requests, WEB-12 team
forms, WEB-13/15 sales effects, ENV-07 environment saves). An exact retry
recovers the original outcome, and a lost reply shows **Outcome unknown**
instead of dispatching again.

### Measured in a browser

The browser run used [`bench/web/2026-10-08-web17/`](../../bench/web/2026-10-08-web17/)
(`browser.py`, `results.json`): `openagents browser run` with Google Chrome
154.0.8037.98, headless, with a fresh profile on macOS, against the synthetic
`cloud_task_fixture` (loopback account service, resident host, and one task).
All 17 checks passed:

| Check | Result |
| --- | --- |
| IME composition commits into the sign-in field | Pass |
| First Tab reaches **Skip to content** | Pass |
| Sign-in reveals the workspace only after the standing check | Pass (about 0.25 s on loopback) |
| Workspace switch by button; the current section is marked `aria-current` | Pass |
| No horizontal scroll on the overview and task pages at 320 px and 375 px | Pass (0 px overflow) |
| The task page's SSE observer names its snapshot in the first event | Pass |
| Cutting the observation connection (through a loopback relay) retires the private view and shows **Reopen workspace** | Pass (under 0.2 s) |
| A reload after the drop re-admits and observes the current snapshot | Pass |
| A reload returns to the same task | Pass |
| A real background tab (tab suspension) retires private content | Pass |
| Returning re-admits only through navigation | Pass |
| Revoking the native session retires the open page | Pass (about 4.9 s, within the poll interval) |
| Reopening after expiry asks to sign in again | Pass |

A dropped observation stream retires the view instead of resuming in place. The
privacy runtime retires on `htmx:sseError`, so in the browser a reload, not
`EventSource` auto-reconnect, recovers the view. The server-side resume and
retire behavior covers any reconnect that reaches the server.

Not measured here: screen readers (VoiceOver or NVDA), a narrow layout on every
lane page (only the overview and task pages were measured), clipboard denial,
and WebGPU/WebGL2 renderer fallback. The last two belong to the workbench
terminal and are covered by its owner step (#10686 in the repo `NEEDS_OWNER.md`).
Essential non-workbench pages use no 3D renderer. Performance was measured only
on loopback with synthetic data. Times on a deployed origin are unknown.

## Activation record

Every slice below is code-complete on `main`. None is deployed or available.
The **Owner step** column names the workspace `NEEDS_OWNER.md` entry (or the
repo's own `NEEDS_OWNER.md`) that must pass before the lane is offered.

| Slice | Page or lane | Code | Owner step before offering it |
| --- | --- | --- | --- |
| WEB-01 [#10948](https://github.com/OpenAgentsInc/openagents/issues/10948) | `/cloud` public entry and honest availability | Complete | None; public. |
| WEB-02 [#10949](https://github.com/OpenAgentsInc/openagents/issues/10949) | Sign-in, session shell, workspace switch, Settings | Complete | The deployed account service: repo "Activate native Cloud runtimes on web staging (#10992)". |
| WEB-03 [#10950](https://github.com/OpenAgentsInc/openagents/issues/10950) | Resident tasks, evidence, original bytes | Complete | #10992 (a host binding on staging). |
| WEB-04 [#10951](https://github.com/OpenAgentsInc/openagents/issues/10951) | Granted task controls and request recovery | Complete | #10992 (durable control-journal custody across replicas). |
| WEB-05 [#10952](https://github.com/OpenAgentsInc/openagents/issues/10952) | Projects and operator Cloud jobs | Complete | #10992. |
| WEB-06 [#10953](https://github.com/OpenAgentsInc/openagents/issues/10953) | Workbench terminal | Complete (now packaged) | Repo "Browser host workbench (#10686)": real device, IME, clipboard denial, WebGPU/WebGL2. |
| WEB-07 [#10954](https://github.com/OpenAgentsInc/openagents/issues/10954) (e9c0d09803) | Verse connections and world Join | Complete | "Join a host world from the Cloud workspace in a real browser (#10954)". |
| WEB-08 [#10955](https://github.com/OpenAgentsInc/openagents/issues/10955) (ee41db7104) | Agents: Alice and Studio | Complete | "Optional: try Alice and the Studio from the Cloud web workspace (#10955)". |
| WEB-09 [#10956](https://github.com/OpenAgentsInc/openagents/issues/10956) (a143f6ae41) | Billing → Retail delegation and key custody | Complete | "Before anyone buys Cloud from the browser: qualify the retail delegation (#10956)", after O3/O4/O8. |
| WEB-10 [#10970](https://github.com/OpenAgentsInc/openagents/issues/10970) (a8a7e35e39) | Retail purchases | Complete | "Before Cloud purchases are offered in the browser: qualify the deployed lane (#10970)". |
| WEB-11 [#10957](https://github.com/OpenAgentsInc/openagents/issues/10957) (ba548c13f5) | Billing statements and decision resources | Complete | "Before browser billing statements are shown to customers: review one real statement (#10957)". Browser purchases of gateway and plugin resources stay unavailable. |
| WEB-12 [#10958](https://github.com/OpenAgentsInc/openagents/issues/10958) (b6f53bf2ab) | Team | Complete; off until `--cloud-team` | "Before turning on browser Team pages: qualify them once on the deployed site (#10958)". |
| WEB-13 [#10959](https://github.com/OpenAgentsInc/openagents/issues/10959) (8f29783139) | Sales through the sales-owner adapter | Complete | "Before the browser Sales page is used: qualify the sales-owner adapter (#10959)". |
| WEB-14 [#10960](https://github.com/OpenAgentsInc/openagents/issues/10960) (3a3d47faf0) | Sales modules (read-only) | Complete | "Before the Sales modules are used: check them once with real records (#10960)". |
| WEB-15 [#10961](https://github.com/OpenAgentsInc/openagents/issues/10961) (f5f55763c9) | Sales floor, outbox decisions, and Agora board | Complete | "Optional: supervise the sales floor from the Cloud web workspace (#10961)". |
| WEB-16 [#10962](https://github.com/OpenAgentsInc/openagents/issues/10962) (823dd0c497) | Partners, referrals, earnings, and payouts (read-only) | Complete | "Before the Partners page is shown to partners or referrers: check it once with real records (#10962)". |
| ENV-07 (210722c05b) | Project Environment panel | Complete | "Before repository environments are offered: qualify them once on real Boat (#11004)". |
| BYO-01 to BYO-05 | Settings → Claude credential, Sign in to Claude, and background Claude Code | Complete | "Before offering Claude Code to customers…" (#11008), "…background Claude Code tasks…" (#11010), and "…parallel Claude Code…" (#11011). |
| WEB-17 [#10963](https://github.com/OpenAgentsInc/openagents/issues/10963) | Packaging, reconnect resume and retire, `aria-current`, and this record | Complete | "Before the Cloud web workspace is offered: run the WEB-17 browser checks on the deployed site (#10963)". |

Launching any lane still requires its applicable
[O1 to O8 activation gates](../sales/revenue-roadmap.md#owner-qualification-and-activation-gates)
and the deployed browser check above.
