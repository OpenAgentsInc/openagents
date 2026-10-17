# Home dashboard and widgets

2026-10-16. The implementation is tracked in
[#11912](https://github.com/OpenAgentsInc/openagents/issues/11912). Home uses a
shared Rust controller in `apps/openagents/crates/account`, native iOS
and GPUI adapters, and the [public Home SDK](sdk/dashboard.md). Android and
other app hosts can mount the same Rust cards when those hosts are available.

## Product outcome

Opening OpenAgents shows a Home dashboard that answers two questions without
opening individual chats: how is my project doing, and what are my agents doing?
People choose the information, arrangement, and level of detail through small
widgets. They can save several dashboards and share reusable configurations
that other people can find and adapt to their own projects.

The first useful dashboard is an OpenAgents project overview: open issues in
`OpenAgentsInc/openagents`, issues assigned to the signed-in person, agents
currently working, and work that needs attention. This is a starter layout,
not a hard-coded repository or a special dashboard available only to its owner.

## Home and navigation

The phone uses Coder Noir in dark appearance and Coder Light in light appearance.
Sessions stays visible in the tab bar while scrolling, and Home includes a
Sessions shortcut. Home shows up to five current tasks; the full overview and
Sessions keep the rest accessible. Editing sits in the navigation bar. Import,
sharing, saved dashboards, and account sync belong in Manage dashboards.


- Add **Home** as a primary destination. After sign-in, a normal cold launch
  opens the dashboard chosen as the default. An explicit chat, issue, or approval
  link opens its target; returning from the background preserves the current view.
- Keep chat creation and existing destinations one tap away. Opening Home,
  refreshing it, or adding a widget never starts an agent or a model call.
- The header contains the dashboard name, project scope, and an edit menu.
  Switching dashboards preserves each dashboard's scope and arrangement.
- Start new accounts with a small **My work** dashboard. Offer a project picker
  for issue widgets. With no connected project, show **Choose a project**;
  with no GitHub access, show **Connect GitHub**. Do not fill the screen with
  widgets that require services the person has not connected.
- Let people create, rename, duplicate, delete, and set a default dashboard.
  Deleting the default selects another dashboard; deleting the last restores
  the starter. Confirm deletion. No operation changes the underlying project.

Example phone arrangement, with illustrative data:

```text
Home                         Edit
OpenAgents                   Change project

Open issues                  My open issues
128                          17
OpenAgents repo              Assigned to me

Agents                       See all
3 working · 1 needs you · 2 unavailable
Alice    Fixing sign-in       Working
Codex    Checking tests       Working
Solver   Needs your answer    Needs you

Needs attention              See all
Solver has a question        Open chat
Issue #123 needs a review    Open issue

                         Add widget
```

The first screenful prioritizes counts and current work. Cards use short labels,
structured rows, and useful destinations; they do not contain generated essays.
Numbers and states have text labels and never rely on color alone.

## Customization

**Edit widgets** enables adding, removing, reordering, resizing, and configuring
widgets. The picker groups widgets by purpose and offers a sample preview,
supported sizes, required connection, and settings before adding one.

Use an ordered layout with compact and full-width cards. On phones, two compact
cards can share a row; full-width cards span it. Large text or narrow displays
stack compact cards vertically. Content height expands for accessibility.
Desktop can pack the same order into more columns without changing the saved
phone arrangement. Optional desktop placement overrides come later.

Dragging shows the result before saving. Provide **Move up**, **Move down**,
and size controls for people who do not drag. **Done** saves one complete edit;
**Cancel** discards it. Removing a widget offers undo. Updates to its data do not
move the widget, steal focus, or discard unsaved settings.

A widget inherits the dashboard project unless configured with an explicit
override. Show overrides on the card. Duplicate instances are allowed, such as
issues assigned to me beside all open issues. Settings use project pickers,
filters, and toggles. Arbitrary query code is outside the initial editor.

## Initial widget catalog

| Widget | Information at a glance | Settings and destination |
| --- | --- | --- |
| Open issues | Exact count for the selected repositories or GitHub Project board; optional short list | Repositories, optional board, assignee, labels; open the matching issue list |
| My open issues | Open issues assigned to the connected GitHub identity | Same scope controls; open the same filtered list |
| Agents | Working, needs you, queued, idle, and unavailable counts, plus current work rows | All authorized work or selected OpenAgents project, optional agent/host filters; open a session or the full overview |
| Needs attention | Pending questions, approvals, and reported failures with clear reasons | Supported attention types and project; open the existing action screen |
| Recent work | Recently finished or updated work with its reported result | Project and time window; open the chat, task, or result |

The first delivery includes Open issues, My open issues, and Agents. Add Needs
attention and Recent work as their sources become available. Hide unsupported
controls and catalog entries rather than shipping placeholders. Future widgets
can cover builds, pull requests, computers, budgets, schedules, and other data
through the same contract.

### Issue scope and counting

An OpenAgents project, a GitHub repository, and a GitHub Project board are
different selections. Start from the OpenAgents project's attached repositories,
then optionally narrow to an explicitly selected GitHub board. Never infer a
board from a matching name. Persist stable project, repository, and board IDs;
names are display labels. Offer an explicit repository selection as well.

Count open GitHub issues, excluding pull requests, closed issues, and board
draft items. **My open issues** means assigned to the connected GitHub person;
authored issues are a separate selectable filter. Without a board filter, count
all matching issues in the repositories. With one, count the intersection of
repository scope and board membership, independent of the board's status field.
Deduplicate issues appearing through several sources by repository ID and issue
number. The detail list uses exactly the count's filters.

Compute totals through an authorized source that can establish completeness.
A page length is not a total. If a source is incomplete or capped, return
coverage metadata and show **At least 100** or **Some repositories unavailable**,
not a fabricated exact count. Label a complete zero **No open issues**. Report a
fetch failure separately. Losing access removes the affected private data.

### Agent overview

The overview tracks work sessions, not just saved agent definitions. Each row
contains agent name, task/chat title, project, host when useful, reported state,
last activity time, and a link to that work. Group sessions under an agent when
it has several; make clear whether a count refers to agents or sessions. Counts
in the example refer to sessions. A saved agent with no work can appear as idle
only when its source actually reports that state.

Keep execution state separate from connection health. A disconnected working
session remains **Last seen working · 5m ago**, with unavailable health; silence
never proves idle, completion, or failure. Unknown states read **Status
unavailable**. Distinguish waiting for the person from waiting on another system.
Completion requires a reported terminal result, not a missing row.

Aggregate authorized account and paired-host sources without opening every
transcript. Deduplicate mirrored work using the existing stable session/task ID
and source mapping; if no mapping exists, label sources separately rather than
guessing. Parent and child sessions retain their relationship and do not inflate
top-level counts. The full overview provides filters and pagination; the widget
shows a bounded subset with **See all**. Sort attention first, then working,
queued, idle, and finished work, with stable ordering within each group.

## Data updates and failures

Render saved layout and permitted cached data immediately, then update widgets
independently. Each result carries its source, scope, observation time, freshness,
and coverage. A broken source cannot block Home or blank unrelated cards.

Use one shared subscription or request per account, source, and filter set;
duplicate widgets reuse it. While Home is visible, prefer existing session
updates, with a 15-second polling fallback for active work and 60-second refresh
for issue summaries. These are initial scheduling targets, not permissions to
exceed provider limits. Honor rate limits and `Retry-After`, back off failures,
bound concurrent reads, and coalesce pull-to-refresh. Suspend Home polling in
the background and refresh on return. Do not promise continuous iOS background
execution. Other screens can retain their own existing subscriptions.

| State | What the person sees |
| --- | --- |
| First load | Card-sized loading treatment; other widgets stay usable |
| Complete empty result | **No open issues** or **No active work** |
| Disconnected integration | **Connect GitHub** or **Reconnect GitHub** |
| Temporarily unavailable with cache | Previous value with **Updated 5m ago** and retry |
| Temporarily unavailable without cache | **Couldn't load issues** and **Try again** |
| Partial result | Available values with a short coverage label |
| Access removed or account changed | Clear private values immediately; offer an appropriate project or account choice |

Cache private snapshots only in the app's protected account storage. Bind late
responses to account, deployment, scope, and subscription generation; discard
them after switching accounts or filters. Sign-out cancels reads and clears
private views and cached values. A shared template never includes these caches.

## Shared Rust implementation and SDK

Target the [current desktop and mobile apps](../apps/openagents/OPENAGENTS.md),
not the removed `openagents-mobile` crate or retired OpenAgents mobile hosts.
Mobile uses the Rust core under `apps/openagents/crates/mobile`, with UIKit and
Kotlin presentation; desktop uses GPUI. Follow the
[app integration rules](../AGENTS.md#one-openagents-desktop-app-and-one-mobile-app-owner-2026-10-16-binding).

| Layer | Responsibility |
| --- | --- |
| Proposed shared Rust dashboard library | Layout validation, settings, state updates, source scheduling, cache interfaces, and typed widget data/actions |
| `openagents-sdk` | Account-authorized remote reads and dashboard/template operations through public contracts; no UI, host execution, or database dependency |
| Existing domain libraries | Session state, host authority, projects, and integration access; adapters supply dashboard summaries without rebuilding those systems |
| App integration | Account lifecycle, secure storage, navigation, and mounting the shared dashboard |
| Rust Native and shell adapters | Shared card, sheet, and settings semantics; native layout, gestures, scrolling, and accessibility |

Keep OpenAgents widget definitions and product styles in application code.
Build cards from [Rust Native primitives](../crates/rust-native/docs/spec.md);
extend generic components only for reusable semantics. Swift and Kotlin do not
own filters, counts, permissions, or refresh logic. Keep downstream hooks small
and record implementation integrations in the app's downstream patch log.

The [SDK](sdk/README.md) already has an account/session boundary,
[saved-history readers](sdk/chat-history.md), and
[private agent definitions](sdk/agents.md). These do not establish a complete
live-work feed or issue-count API. Qualify the missing public summaries before
adding typed SDK methods. Reuse [project contracts](api/projects.md) and existing
GitHub integration authority; project membership alone does not grant repository
access. Never require a phone shell command, a developer's local GitHub CLI,
or a server credential embedded in the app.

Proposed SDK operations cover widget discovery, issue summaries, paged work
summaries and updates, and dashboard/template CRUD and discovery. Exact route
names and Rust signatures follow a route audit and schema qualification; do not
overload existing `/v1/agents` or voice `/v1/sessions`. Publish only implemented
operations in OpenAPI. Return typed filters, totals with coverage, timestamps,
source health, stable references, and bounded pagination. Do not download whole
transcripts to draw a status card.

Use the current app session, explicit deployment, cancellation, bounded bodies,
and typed errors. Recheck access at the source and destination. A widget action
opens the existing issue/chat/approval screen, where current authority is checked
again. No imported configuration can grant execution, approvals, or spending.
Session observation follows the apps' existing `coder-sync` path; Home does not
create another session transport. Respect the [Coder 1.0 freeze](coder/1.0-freeze.md):
mobile, desktop, SDK, and additive server delivery need no new frozen CLI binary.

## Configuration and portability

Keep three concepts separate: a widget type describes what can be displayed;
a widget instance holds a person's settings; a dashboard orders those instances.
A template is an exportable dashboard configuration with named input slots.

Proposed versioned records:

| Record | Required contents |
| --- | --- |
| Widget type | Namespaced ID and version, short name/description, supported sizes, typed settings and output schemas, required capabilities, and supported actions |
| Dashboard | Schema version, ID, owner, revision, name, default scope, ordered widget instances, and optional surface layout overrides |
| Widget instance | Stable ID, widget type/version, size, inherited or explicit scope, typed filters, and display preferences |
| Template package | Package schema/version, template ID/version, author, title, description, tags, minimum client/widget versions, dashboard layout, input slots, required capabilities, and content hash |

Use a bounded declarative JSON format, initially at most 256 KiB, 32 widgets,
and 16 input slots per package. Validate size before decoding and reject duplicate
instance IDs, invalid filters, unsupported versions, and excessive nesting.
Unknown widget types remain saved but inactive with a short explanation; they
cannot execute a fallback. Never silently reinterpret a newer schema. Imports
validate completely before changing the person's dashboards.

Local edits save atomically and survive restart. The mobile first release can
keep layouts on-device. Account sync later stores private dashboards separately
from session documents, with expected-revision writes. A conflict offers reload
or save as a copy and preserves the local draft; it cannot silently overwrite
another device's layout. Keep unresolved writes across restart and reuse the
same request identity after response loss. Device-specific default-dashboard
preferences let phone and desktop open different dashboards.

## Save, share, and discover layouts

**Save as template** opens a preview of the layout, required connections, and
input slots such as **Project**, **Repositories**, and **GitHub board**. Export
replaces private bindings with slots and uses synthetic preview data. Strip
credentials, account IDs, local paths, host addresses, cached values, issue/chat
content, and private project/repository names. A user may explicitly choose a
public repository default after reviewing it. Revalidate the final package on
the service; a UI preview alone cannot enforce export privacy.

Support local export/import first, then a directory of layouts. Sharing a file,
publishing a listing, and installing a template are distinct actions. Saving a
private layout does not list it publicly. The later directory supports private,
unlisted-link, and public visibility, search by title/tag, featured starters,
author pages, mobile/desktop compatibility, and previews with sample data.
An unlisted link is discoverable by anyone holding it and contains no private
data. Public listing requires an explicit publish action and a final preview.

**Use this layout** performs this flow:

1. Preview widgets, author, version, required connections, and compatibility.
2. Map slots to the recipient's own authorized projects and repositories.
3. Connect missing services or skip dependent widgets; never copy the author's
   credentials or grant access to the author's resources.
4. Create a new editable private dashboard. Offer **Make default** separately;
   leave the person's existing dashboard intact.

Templates have immutable versions. An installed dashboard is a personal copy
with its source template/version recorded. Updates offer a preview and explicit
apply; they preserve slot mappings and local edits or offer another copy.
Unpublishing removes discovery and new installs, without deleting copies people
already made. The directory needs author removal, reporting, and moderation
before public launch. Paid layouts and revenue sharing are outside this spec.

## Extension boundary

The first release uses reviewed built-in Rust widget types. Sharing a layout
shares configuration, not executable code. The future SDK can let developers
register typed data providers and reusable widget definitions with capability
declarations. Prove this with one additional provider using the same public
summary contract and generic card renderer before calling it extensible.

Packages cannot contain JavaScript, TypeScript, native libraries, shell commands,
arbitrary network endpoints, or raw native class names. New executable providers
follow the existing plugin/host installation and permission model separately.
Treat returned titles, links, and text as untrusted data; render text safely and
allow only validated navigation targets. A bad provider gets bounded time and
data, fails within its widget, and cannot block the dashboard.

## Delivery and acceptance

| Stage | Deliverable | Acceptance |
| --- | --- | --- |
| 1: Mobile Home | Default Home, issue widgets, cross-session agent overview, local customization and persistence | A person selects the OpenAgents project, sees correctly scoped issue counts and current work without visiting each chat, rearranges widgets, and sees the saved result after restart |
| 2: Portable configurations | Versioned package, slot mapping, export/import, optional private account sync | A second account imports the layout with its own resources; no author data or grants cross accounts; invalid packages change nothing |
| 3: Layout directory | Preview, search, explicit publication/install, version updates, removal and moderation | A person finds a public mobile-compatible layout, reviews it, installs a private copy, and can keep edits when an update appears |
| 4: Desktop and extensions | GPUI Home, responsive layout, shared Rust cards, public SDK example and one additional provider | The same configuration works on phone and desktop, and an outside-checkout Rust client reads the qualified summaries without app-private APIs |

For stage 1, exercise complete, zero, partial, paged, duplicate, rate-limited,
and revoked issue sources; multiple repositories and an optional GitHub board;
and assigned-to-me filtering. Agent fixtures cover several hosts, mirrored work,
children, waiting states, unknown states, disconnect/reconnect, and real terminal
results. Test source failure isolation, account/filter switching with late
responses, background suspension, restart, edit cancellation, and large text.
Check VoiceOver and TalkBack reading order, accessible reordering, tap targets,
and opening the exact filtered issue list or selected work session.

Later-stage checks add malformed/oversized imports, export privacy, unsupported
widget versions, concurrent layout edits, uncertain saves, directory access,
unpublication, and template updates that preserve customizations. Use synthetic
fixtures and dedicated accounts; do not leave test work on the owner's account.
Run targeted checks for the crates changed in each implementation stage under
the [verification policy](verification.md). Documentation edits need local link and diff checks. App and server changes
need their targeted Rust and native checks before release.
