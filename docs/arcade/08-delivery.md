# Arcade delivery into OpenAgents

2026-11-15. Execution plan for [#13624](https://github.com/OpenAgentsInc/openagents/issues/13624).
This implements the owner's full-port request and replaces the execution order
and unsettled assumptions in part 05. Parts 01–04 remain frozen source audits.
The source is Arcade `3f10e1e`, arclib `542ecec`; no old application workspace
or dependency tree is restored.

## Order and release gates

Work one issue at a time, verify it, and push its commit to main before starting
the next. Component work required by an earlier feature belongs to the shared
library in that feature's change; the complete catalog closes in #13635.

| Order | Issue | Usable result / acceptance gate |
| --- | --- | --- |
| 1 | [#13625](https://github.com/OpenAgentsInc/openagents/issues/13625) | This current inventory, dependency map and verification plan |
| 2 | [#13626](https://github.com/OpenAgentsInc/openagents/issues/13626) | Versioned identity, signer, relay-read and messaging contracts; permission/failure fixtures |
| 3 | [#13627](https://github.com/OpenAgentsInc/openagents/issues/13627) | Live read-only kind-1 feed from selected connected relays on phone/web; **first TestFlight upload** |
| 4 | [#13628](https://github.com/OpenAgentsInc/openagents/issues/13628) | Feed transport extended with durable verified messaging state, signer effects and recovery |
| 5 | [#13629](https://github.com/OpenAgentsInc/openagents/issues/13629) | Phone/web/independent-client NIP-17 DMs in the existing inbox and composer |
| 6 | [#13630](https://github.com/OpenAgentsInc/openagents/issues/13630) | Contacts, profiles, private/public follows and synchronized lists; Following feed filter |
| 7 | [#13631](https://github.com/OpenAgentsInc/openagents/issues/13631) | Small encrypted groups, replies and shared message actions |
| 8 | [#13632](https://github.com/OpenAgentsInc/openagents/issues/13632) | NIP-29 groups, discovery, invitations, moderation and actual relay enforcement |
| 9 | [#13633](https://github.com/OpenAgentsInc/openagents/issues/13633) | Images/files, private attachment encryption and voice/video integration |
| 10 | [#13634](https://github.com/OpenAgentsInc/openagents/issues/13634) | Actual messaging notifications, tap routing and lifecycle recovery |
| 11 | [#13635](https://github.com/OpenAgentsInc/openagents/issues/13635) | Complete A01–A28 library/gallery evidence and mixed-surface qualification |
| 12 | [#13636](https://github.com/OpenAgentsInc/openagents/issues/13636) | Legacy/prototype inventory closure, interoperability and full release qualification |

Dependencies: 2 follows 1; 3 needs the plan and its own minimum read contract,
not messaging crypto; 4 needs 2 and 3; 5 needs 4; 6 needs 5; 7 needs 5/6;
8 needs 7; 9 needs 8; 10 needs 5/7/8; 11 needs 1 and consumes the earlier
features' component evidence; 12 needs all delivery issues. The owner selected
serial execution even where infrastructure or component tasks could overlap.

Upload again after each major feature is ready. Freeze the tested source,
resources, runtime fingerprint, signed feed, archive/IPA and symbols for each
release. Record the upload delivery ID and continue the next issue while Apple
processes it. Processing, tester assignment and availability are separate
observations; an accepted upload is never reported as already available.
Use the existing assignment/release tools asynchronously where supported.
Preserve older runtime feeds and verify compatible OTA activation/rollback.
New installed services, shaders, SDKs or entitlements require a binary; do not
promise that a new capability fits an older fingerprint.

## Current foundation and integration rules

Baseline source inspection: `dc0d9ef17a`. These are existing release records,
not a claim that this planning change reran a phone test:

- **E1:** [1087 GPU chrome](../verification/2026-11-14-arcade-mobile/README.md).
  Shared Arcade skin/scene, entry/focus/press, arcs and progress; not all animation parity.
- **E2:** [1088 account/settings](../verification/2026-11-14-arcade-settings/README.md)
  and [mapping](07-account-settings.md). Managed identity, profile publication,
  saved relays, privacy, login keys/passkeys, backup and device sessions exist.
- **E3:** [1090 workspace](../verification/2026-11-14-mobile-workspace/README.md).
  Centered titles, automatic reads, regular-page Worlds, simplified composer,
  fixed-width sidebar reveal and single-line history are shipped.

The shell adds **Feed** alongside Chat / Projects / Worlds, as a regular page.
Existing destinations retain their identity and authority. History presents
account, agent, environment and Nostr conversations with typed references;
displaying them together does not move their stores or grant execution access.
Project/world conversations gain explicit links in #13632 rather than an
implicit storage migration. No browser handoff is needed for native flows.

Keep E3's current controls: no refresh buttons; automatic reads/subscriptions;
title centered independently of adjacent trailing actions; new-project plus;
one-line history truncation with complete accessibility text. The composer is
text and Send/Stop. Its attachment control appears only with functioning
attachments in #13633. Model/effort, microphone and new-chat/clear-draft controls
stay hidden; voice receives a deliberate entry point when its flow is ready.
The central orchestrator remains separate work.

Account-managed identity is canonical where provisioned. Independently linked
keys are preserved; importing a login key does not rotate that identity.
Device credentials and wallet seeds remain separate. Messaging authorization
is a new contract: neither profile signing nor the explicit backup ceremony
silently becomes arbitrary signing or background key provisioning.
Secrets never enter updateable UI state, trees, logs or OTA packages.

Use the existing Nostr relay/filter/verification modules and
`coax/lib/nostr/client_journal.coax`; its exact-byte outbox, acknowledgements,
cursor and crash replay are the starting point, not a new cache written in Swift.
Extend bounds/indexing/retention for the actual consumer. Feed owns the minimum
read transport and generation/cancellation behavior; #13628 extends it for
messaging. A saved relay is called connected only after a real observation.

The first feed is kind 1 only, verified before retention/display, deduplicated
by event ID, newest first with an ID tie-break. Filters: selected relays,
author npub/hex, 24h/7d/all time and hashtag. Paginate equal timestamps safely,
preserve the reader's scroll anchor and show newly arrived notes unobtrusively.
Distinguish empty from incomplete/unavailable results. Following arrives in
#13630. No new signing authority is required to read public notes.

New internal bridge/stored shapes start in `proto/`, with one registry row,
one codec and checks. Existing signed Nostr originals keep their bytes; a
decoded projection is not verification. Product behavior is Coax. Shared
components own Markdown, references, editing, layout, paint and motion; native
hosts supply lifecycle, secure effects, IME, accessibility and GPU bindings.

The [Nostr app service contracts](../protocol/nostr-app-services-v1.md) define
the read-only installed bridge and the later custody boundary. They reserve
separate capabilities, retain the approved account-managed key model and
require per-action contracts before messaging effects are implemented.

## Complete feature disposition

Numbers below are the immutable inventory rows in [part 01 §13](01-codebase.md#13-what-the-port-must-carry-over).
“Existing” refers to E1–E3 and still requires integration regression in #13636.
“Replace” names an intentional modern equivalent, not an unowned omission.

| Row | Arcade feature | OpenAgents destination / required evidence | Delivery issue |
| --- | --- | --- | --- |
| 1 | Welcome / create / log in | Existing email-code front door; preserve cold-launch/login flow (E2/E3) | #13636 |
| 2 | Generate identity | Existing managed identity provisioning; preserve linked identity and device key separation | #13626 |
| 3 | Signup profile setup | Existing optional Account editor; photo picking in media stage | #13630, #13633 |
| 4 | NIP-05 names | Optional opted-in OpenAgents name service; no dependency on Arcade registrar; explicit completion decision | #13636 |
| 5 | Publish profile | Existing saved kind-0 publication (E2), complete field/list interoperability | #13630 |
| 6 | nsec / hex login | Existing native login-key import (E2); no canonical identity replacement | #13626, #13636 |
| 7 | Restore state | Read verified relay/follow/group lists before writes, recover durable history | #13628, #13630 |
| 8 | Default groups | Explicit OpenAgents NIP-29 groups and joined-list policy | #13632 |
| 9 | Key backup | Existing protected backup (E2); reviewed encrypted export / recovery compatibility | #13626, #13636 |
| 10 | Logout | Existing confirmation; add account-scoped event/outbox/plaintext cleanup | #13628 |
| 11 | Unified inbox | Existing history with typed Nostr destinations and unread/activity state | #13629 |
| 12 | Rich inbox row | Shared author/avatar/time/last-message roles; one-line truncation | #13629, #13633 |
| 13 | Refresh | Replace pull/manual controls with live subscriptions and automatic reads (E3) | #13627, #13628 |
| 14 | Drawer | Existing Arcade rail plus fixed-width history; add Feed destination | #13627 |
| 15 | New conversation | Compact explicit message/group actions outside the attachment control | #13629, #13631 |
| 16 | Contacts and removal | Shared contact list, remove and synchronization | #13630 |
| 17 | Find by key / NIP-05 | Shared resolver and bounded credential-free host fetch | #13630 |
| 18 | Suggested people | Follow graph and supported search; no old trending-service dependency | #13630 |
| 19 | Public follow | Kind 3, preserving other-client tags/hints/petnames | #13630 |
| 20 | Private follow | NIP-51 private items, absent from public kind 3 | #13630 |
| 21 | Legacy/blinded mode | Replace both with standard NIP-17; old data read-only where recoverable | #13629, #13636 |
| 22 | Another user's profile | Verified profile, loading/fallback, Message/Follow actions | #13630 |
| 23 | Own profile/settings | Existing Account hub (E2), integrated with messaging state | #13630 |
| 24 | Profile editing | Existing fields/HTTPS images and kind-0 publication; photo selection later | #13630, #13633 |
| 25 | Public chat | NIP-29/NIP-C7 with paged history, replies and admission | #13632 |
| 26 | Channel discovery | Verified relay metadata and followed people's group lists | #13632 |
| 27 | Create channel | Relay-enforced NIP-29 create/metadata flow | #13632 |
| 28 | Join/leave sync | Group operations and preserved kind-10009 lists | #13632 |
| 29 | Channel manager | Group's native settings and moderation | #13632 |
| 30 | Private group | NIP-17 participant-bounded room | #13631 |
| 31 | Invite | Contact selection; new NIP-17 room on membership change / NIP-29 invite | #13631, #13632 |
| 32 | Members | Verified message participants or relay-signed membership, not local invite history | #13631, #13632 |
| 33 | DMs | NIP-17 open/send/sender copy, verified delivery and recovery | #13629 |
| 34 | Blinded DM / badge | Replace proprietary crypto with NIP-17 and accurate privacy indicators | #13629 |
| 35 | Swipe/reply | Shared gesture/quote/action path, one completion and preserved scroll anchor | #13631 |
| 36 | Text/references/links | Existing shared parsers; tappable inert references and safe links | #13627, #13629 |
| 37 | Inline images | Shared resource/decode/cache/render path with privacy settings | #13633 |
| 38 | Video | Platform media effect with shared controls and explicit lifecycle | #13633 |
| 39 | Link preview | Bounded service or user action; no device fetch on message render | #13633 |
| 40 | Image attachments | Working attachment action, progress/cancel, encrypted private upload | #13633 |
| 41 | Reputation | Retire obsolete ArcTrust quality field and fake score; no unsupported reputation claim | #13636 |
| 42 | AI / Faerie | Merge into existing authenticated OpenAgents agent chats; no second persona/service | #13629, #13636 |
| 43 | Voice input | Existing OpenAgents voice/transcription service with deliberate entry point | #13633 |
| 44 | Model picker | Remains hidden per owner; central orchestrator owns provider choice separately | #13625 |
| 45 | Optimistic send | Exact-event durable outbox, accepted/pending/failed state and safe retry | #13628, #13629 |
| 46 | Relay management | Existing saved WSS settings (E2); observed connections and published list reconciliation | #13627, #13630 |
| 47 | Defaults | Existing OpenAgents default, user-selected relays; no silent third-party additions | #13627 |
| 48 | Push preferences | Extend actual security-notice settings to qualified messaging notifications | #13634 |
| 49 | Push registration | Replace custom Nostr event with versioned account/device notification service | #13634 |
| 50 | DM privacy setting | Replace ineffective cipher toggle with actual privacy controls (E2) | #13630 |
| 51 | Private settings | Existing revisioned account settings; never publish in kind 0 | #13630 |
| 52 | Event cache | Extend shared client journal, verified indexing, bounded retention and logout wipe | #13628 |
| 53 | Batched metadata | Shared verified profile cache and bounded author lookups | #13627, #13630 |
| 54 | Changelog | Existing OpenAgents release notes (E2), updated per release | #13636 |
| 55 | Resources | Existing OpenAgents links and native Components (E2/E3) | #13636 |
| 56 | Listings prototype | Explicit prototype closure: NIP-99 BTC/sats flow or recorded owner scope decision | #13636 |
| 57 | Offers/accept prototype | Explicit prototype closure: authenticated NIP-17 offers, correct accepting authority, Bitcoin-only | #13636 |
| 58 | Trade commands prototype | Retire ad hoc regex dispatch; any retained commands use shared typed command interface | #13636 |
| 59 | Geohash prototype | If listings retained, retain precision cap and explicit location consent | #13636 |
| 60 | Discover hub | Merge into real people/group discovery | #13630, #13632 |
| 61 | Error boundary | Existing runtime rollback plus typed per-feature failure/recovery | #13628, #13636 |
| 62 | Haptics | Shared intent, platform effect; menu/swipe only as qualified | #13631 |
| 63 | Long-press popup | Shared menu/focus path; complete lifted blur specimen | #13631, #13635 |
| 64 | Motion/HUD/splash | Every source capability in library/gallery; quiet grayscale product choices | #13635 |
| 65 | Dark appearance | Existing grayscale skin; authored world colors remain separate (E1/E3) | #13635, #13636 |

Rows 4, 56–59 are source prototypes/service-dependent additions, not silently
discarded requirements. #13636 cannot close with them unresolved: implement
their modern flow or record the owner's explicit scope decision in the epic.
Legacy data availability is measured and reported; unavailable source services
or lost original keys cannot be represented as successfully migrated data.

## Complete motion disposition

The pinned source geometry/defaults and exact requirements remain in
[part 06](06-rendering-and-animation.md). All rows belong to #13635; the
consumer column identifies earlier issues that need a functional subset.
Existing evidence is deliberately narrower than full source parity.

| Case | Capability | Existing evidence / next work | Earlier consumer |
| --- | --- | --- | --- |
| A01 | Frame enter/exit | E1 assembly; retain source origins/exit and specimen traces | #13627 |
| A02 | Animated highlight | E1 press; interruption/reversal reference | #13627 |
| A03 | Gesture button | E1 action adapter; cancel and exactly-one action | #13627 |
| A04 | Focus chrome | E1 native input; actual IME/AutoFill and focus reference | #13627 |
| A05 | Small spinner | E1 arcs; complete glow/mask parity | #13627 |
| A06 | Large spinner | Outer spring-loop and reference values required | — |
| A07 | Swipe return | Shared gesture/scroll/RTL arbitration required | #13631 |
| A08 | Reply icon | Opacity/parallax track required | #13631 |
| A09 | Donut trim | E1 arc lowering; round-cap/source sweep evidence | #13631 |
| A10 | Swipe completion | Independent spring/action latch and cancellation required | #13631 |
| A11 | Reply bar | Height/opacity with keyboard and transcript anchor required | #13631 |
| A12 | Bound text | Existing text adapter; update without second editor | #13631 |
| A13 | Drawer slide | E2/E3 rail/history; actual fixed-width reveal and drag evidence | #13627 |
| A14 | Icon morph | E2 shared pose; pinned spring/reference and interruption | #13627 |
| A15 | Row entry | Stable-key stagger and bounded realization required | #13627 |
| A16 | Switch/choice | E2 spring switch; all variants/reference required | #13630 |
| A17 | Press fill | E1 press; cancel/release and high contrast required | #13627 |
| A18 | Lifted blur popup | Owned dual blur layers, sharp child, input fence and focus restore required | #13631 |
| A19 | Tab stagger | Dormant gallery specimen; does not add tabs to product | — |
| A20 | Gradient pulse | Sweep/blur with offscreen pause required | — |
| A21 | Splash blur | Bounded timeline, declared source bug fix; gallery only | — |
| A22 | Typing/caret | Shared grapheme reveal and bounded clock required | — |
| A23 | Animated image | 19 × 50ms source-frame playback/resource specimen required | #13633 |
| A24 | Copied transient | E2 clipboard UI; one announcement and cancellation | #13630 |
| A25 | Route transition | Portable enter/exit/back/cancel specimen required | #13627 |
| A26 | Sheet transition | Shared drag/detent/keyboard/focus qualification required | #13631 |
| A27 | Press/busy variants | E1/E2 subset; retain immediate and timed variants | #13627 |
| A28 | Keyboard/scroll | E3 native docking; Android/accessibility/source cases required | #13629 |

## Verification and completion

Before each feature, write focused Coax checks for its real state/admission
rules, run touched suites remotely and mutate changed authority procedures.
Use existing reference vectors and independent Nostr clients; do not treat
two copies of our own codec as interoperability. Run only required additional
gates for a change and retain passing evidence for unchanged files.

Feed checks cover two actual relays, forged/duplicate/stale responses, all
filters, equal-time pagination, empty versus unavailable, reconnect, scroll
anchor and background/resume. DM/group checks add exact-event retries,
wrong identities, membership transitions, cross-device convergence and actual
relay moderation. Media/push checks add tamper/limits, privacy, device
revocation, real notification taps and no unsolicited playback.

Take native and headless web pictures on each relevant feature. Actual iOS
input/accessibility is required; Android/desktop qualification remains explicit
and cannot be inferred from drawing alone. #13635 qualifies DOM/GPU islands,
independent scene contexts and three concurrent worlds with aggregate resource
budgets, origin isolation, input/audio ownership and device-loss recovery.

The final regression set includes email-code login; managed/linked identities;
keys, passkeys, backups and device revocation; agent/account/computer chats;
Projects and world loading/editing; Usage and optional read-only sats balance;
draft/scroll preservation; grayscale/reduced motion; Updates activation,
rollback and a fresh install with no redundant update prompt.

Release records separate tested source, upload accepted, Apple processing,
tester assignment and device acceptance. Owner-only actions go in
`NEEDS_OWNER.md`. Never close a feature solely because its upload began.
