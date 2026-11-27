# Plan

This is the plan for writing and building The OpenAgents Book. It gives an
order and no dates.

The first draft (2026-11-10) followed
[the port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md) and
started from the agent loop; the apps waited for chapter 61. On 2026-11-13
([#13512](https://github.com/OpenAgentsInc/openagents/issues/13512)) the book
was reordered so that working web and iPhone apps came first, on
[Coax Native](../../coax/native.md), from shared DOM and UIKit components.
On 2026-11-15
([#13645](https://github.com/OpenAgentsInc/openagents/issues/13645)) the
opening, chapters 1 to 11, was rewritten for the compiled-Coax GPU architecture:
every component drawn on the GPU by the shared display-list kit, no JavaScript
and no over-the-air updates. Subsequent desktop-first shipping made
[`apps/openagents-desktop`](../../apps/openagents-desktop) the primary native
host (macOS Metal 4, Linux Vulkan, and Windows Win32/Vulkan `#14046`), backed
by shared Coax screen/app modules in [`apps/shared/`](../../apps/shared) that
[`apps/openagents-ios`](../../apps/openagents-ios) and Android consume, the
unified 24-layer pure-Coax runtime in [`openagents/core/`](../../openagents/core)
(`#14050`–`#14057`), **Coax Web** with Coax UI Core/Kit/Router and 0-KB-Wasm
SSR in [`components/ui/`](../../components/ui), OpenAgents Cloud pricing/usage
APIs (`#14069`–`#14072`), and the locked **WorkOS hybrid `acct_` auth** SDK in
[`coax/lib/workos/`](../../coax/lib/workos) (`#14097`–`#14103`). Chapters 12
to 78 keep their numbers and structure. The [chapter map](#chapter-map) gives
each chapter's number in the first draft and in the 2026-11-13 order, so
links and notes against the old numbers can be traced.

## Where the code goes

A chapter adds a path only where no current owner exists. Everything else
extends the owner.

| What | Owner | Chapters |
| --- | --- | --- |
| App state, actions, routes and the driver | Coax Native ([`coax/native.md`](../../coax/native.md)); shared app state in [`apps/shared/`](../../apps/shared) | 2, 4, 6, 7 |
| Components on the GPU: the frame context, keyed state, the semantic table, the surface commit, text, Markdown, the edit buffer | [`components/`](../../components) ([GPU components on native](../../components/gpu-components.md)) | 1–3, 5, 7–9 |
| Coax Web, Coax UI Core/Kit/Router, Native SSR/Hydration & DOM presentation | [`components/ui/`](../../components/ui), [`components/backend/dom.coax`](../../components/backend/dom.coax), [`components/backend/html.coax`](../../components/backend/html.coax), [`coax/web/`](../../coax/web) | 11 |
| The apps | [`apps/openagents-desktop`](../../apps/openagents-desktop/README.md) primary (macOS Metal 4, Linux Vulkan, Windows Win32/Vulkan [`windows/`](../../apps/openagents-desktop/windows)), with [`apps/openagents-ios`](../../apps/openagents-ios), Android ([`apps/shared/host/android`](../../apps/shared/host/android)), and [`apps/openagents-web`](../../apps/openagents-web) consuming shared Coax modules in [`apps/shared`](../../apps/shared) | 1–11, 30, 31, 34, 36, 75, 77 |
| Login, accounts, WorkOS hybrid `acct_` spine, agent grants and the HTTP API | [`coax/web/api/`](../../coax/web/api), [`coax/lib/workos/`](../../coax/lib/workos), [`tools/oagen-coax/`](../../tools/oagen-coax), GPU login screen ([`apps/shared/login/`](../../apps/shared/login)), and native auth adapter ([`AppAuth.swift`](../../apps/shared/host/apple/AppAuth.swift)) | 4, 5, 59, 64 |
| Engines, typed router, sessions, the journal, observation | [`coax/lib/agent/`](../../coax/lib/agent) (including [`router/`](../../coax/lib/agent/router)) | 3, 4, 12, 15, 18, 25, 26, 37 |
| Unified 24-layer pure-Coax agent runtime, member packs, subagents, permissions & memory | [`openagents/core/`](../../openagents/core) and [`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax) / [`oa-subagent.coax`](../../coder/tui/oa-subagent.coax) | 12–14, 17–18, 23–28, 33–36 |
| Money rules, paid capabilities & author payouts | The Payments context, [`pay/`](../../pay) (`wallet.coax`, `wallet-rpc.coax`, `price-policy.coax`, `receipt-policy.coax`, `payout-policy.coax`, `executor-settlement.coax`) | 32, 33, 56–58, 61, 70 |
| Signed capability registry & hash-pinned installs | [`openagents/registry/`](../../openagents/registry) (`registry.coax`, `install.coax`, `#14060`) | 47, 54–55 |
| The wallet | Shared Apple Breez Spark host ([`SparkWallet.swift`](../../apps/shared/host/apple/SparkWallet.swift)), Desktop wallet panel & RPC ([`walletpane.coax`](../../apps/openagents-desktop/page/walletpane.coax), [`Window+Wallet.swift`](../../apps/openagents-desktop/Window+Wallet.swift), [`Window+WalletRPC.swift`](../../apps/openagents-desktop/Window+WalletRPC.swift)), and `openagents wallet` CLI ([`wallet.coax`](../../apps/openagents-cli/wallet.coax)) over `#authority` [`pay/`](../../pay) | 32, 33 |
| Voice | [`metaverse/voice.coax`](../../metaverse/voice.coax), [`apps/shared/dictation.coax`](../../apps/shared/dictation.coax), [`apps/openagents-desktop/page/voice.coax`](../../apps/openagents-desktop/page/voice.coax), [`Window+Voice.swift`](../../apps/openagents-desktop/Window+Voice.swift) | 31 |
| Worlds & Embedded GameBuilder | The Metaverse Engine ([`metaverse-engine/`](../../metaverse-engine)), [`gamebuilder/worlds/`](../../gamebuilder/worlds), [`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax) | 71–74 |
| Releases (every iOS change ships in a TestFlight or store build; Desktop via signed macOS/Linux/Windows builds) | [`coax/remote/`](../../coax/remote), each app's release script | 76 |
| Remaining planned product specs (knowledge base, sandboxed plugin SDK, public Gym runner) | `openagents/agent/`, `openagents/knowledge/`, `openagents/eval/` (planned; tracked in `#14058`–`#14059`) | 20–22, 38–53, 60–61, 73 |

The wire shapes live in [`proto/`](../../proto/README.md), and
[the OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md)
describes them. Chapter 4 gives the app the minimum; chapters 62 to 67 give
the rest.

The [environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
owns the proto-first desktop/phone connection to Claude on this Mac and
CoderOS, and subsequent realm editing/preview. It composes HOST, ENV, WS and
SESS rather than defining another app-specific agent wire. The desktop app
and optional headless CLI attach to the same resident execution owner. Chapters 8
and 10 use it.

The checker refuses a planned path that already exists or that sits where a
current owner lives ([CONTRACT.md](CONTRACT.md#what-the-checker-enforces)).

## Chapter map

Each chapter, its part, its number in the first draft ("new" for a chapter
added in the 2026-11-13 reorder), what it replaces in the 2026-11-13 order,
and the earlier chapters it depends on. Chapters 12 to 78 kept their numbers
in the 2026-11-15 rewrite.

<!-- map:begin -->
| Chapter | Part | First draft | 2026-11-13 order | Depends on |
| --- | --- | --- | --- | --- |
| [1. A text input on the GPU](ch001-a-text-input-on-the-gpu.md) | I | new | part of 3 (components) | none |
| [2. State and actions](ch002-state-and-actions.md) | I | new | 2 | 1 |
| [3. A transcript](ch003-a-transcript.md) | I | new | part of 11 (the transcript) | 2 |
| [4. A reply from the service](ch004-a-reply-from-the-service.md) | I | new | 5 (effects), 9 (the API), part of 11 (reply and Stop) | 3 |
| [5. Logging in when a send needs it](ch005-logging-in-when-a-send-needs-it.md) | I | 50 | 8 (accounts and login), part of 4 (modals) | 4 |
| [6. One case, every step](ch006-one-case-every-step.md) | I | new | 6, and 11's milestone as history | 5 |
| [7. History and the sidebar](ch007-history-and-the-sidebar.md) | II | new | 4 (routes), part of 10 (the side) | 6 |
| [8. Environment and repository chips](ch008-environment-and-repository-chips.md) | II | new | new | 7 |
| [9. Settings](ch009-settings.md) | II | new | part of 10 (settings) | 7 |
| [10. Mobile and other native hosts](ch010-mobile-and-other-native-hosts.md) | II | 2 | 1 (one app, every surface) | 6, 7, 8, 9 |
| [11. The web](ch011-the-web.md) | II | new | 7 (the public website), part of 3 (DOM) | 6, 7, 9 |
| [12. The simplest agent](ch012-the-simplest-agent.md) | III | 1 | 12 | 6 |
| [13. Members](ch013-members.md) | III | 3 | 13 | 12 |
| [14. A typed plan](ch014-a-typed-plan.md) | III | 4 | 14 | 13 |
| [15. Decide before you generate](ch015-decide-before-you-generate.md) | III | 5 | 15 | 14 |
| [16. Measure the front](ch016-measure-the-front.md) | III | 6 | 16 | 15 |
| [17. Grants and approvals](ch017-grants-and-approvals.md) | III | 7 | 17 | 14 |
| [18. On the record](ch018-on-the-record.md) | III | 8 | 18 | 14, 17 |
| [19. Conversation](ch019-conversation.md) | III | 9 | 19 | 15 |
| [20. Prepared answers](ch020-prepared-answers.md) | III | 10 | 20 | 16, 19 |
| [21. A knowledge base](ch021-a-knowledge-base.md) | III | 11 | 21 | 18, 20 |
| [22. Retrieval by meaning](ch022-retrieval-by-meaning.md) | III | 12 | 22 | 21 |
| [23. Memory and consent](ch023-memory-and-consent.md) | III | 13 | 23 | 17, 21 |
| [24. Coder as a member](ch024-coder-as-a-member.md) | IV | 14 | 24 | 13, 17 |
| [25. Other engines](ch025-other-engines.md) | IV | 15 | 25 | 24 |
| [26. Subagents you can see](ch026-subagents-you-can-see.md) | IV | 16 | 26 | 18, 25 |
| [27. Programs](ch027-programs.md) | IV | 17 | 27 | 14, 24 |
| [28. The command line behind the agent](ch028-the-command-line-behind-the-agent.md) | IV | 18 | 28 | 15, 17 |
| [29. Where work runs](ch029-where-work-runs.md) | IV | 21 | 29 | 17, 24 |
| [30. Files and device services](ch030-files-and-device-services.md) | IV | new | 30 | 4, 6 |
| [31. Voice](ch031-voice.md) | IV | new | 31 | 6, 30 |
| [32. A wallet](ch032-a-wallet.md) | IV | 19 | 32 | 17, 28 |
| [33. Spending approval](ch033-spending-approval.md) | IV | 20 | 33 | 17, 32 |
| [34. State that survives](ch034-state-that-survives.md) | V | new | 34 | 4, 6 |
| [35. Sync across devices](ch035-sync-across-devices.md) | V | 51 | 35 | 5, 10, 34 |
| [36. Background, resume and cancel](ch036-background-resume-and-cancel.md) | V | new | 36 | 4, 34 |
| [37. One owner per task](ch037-one-owner-per-task.md) | V | new | 37 | 29, 35, 36 |
| [38. The plugin interface](ch038-the-plugin-interface.md) | VI | 22 | 38 | 13, 17 |
| [39. The sandbox](ch039-the-sandbox.md) | VI | 23 | 39 | 38 |
| [40. Installing grants nothing](ch040-installing-grants-nothing.md) | VI | 24 | 40 | 17, 39 |
| [41. Write a plugin](ch041-write-a-plugin.md) | VI | 25 | 41 | 40 |
| [42. The authoring interview](ch042-the-authoring-interview.md) | VI | 26 | 42 | 27, 41 |
| [43. Test-time capabilities](ch043-test-time-capabilities.md) | VI | 27 | 43 | 40 |
| [44. Cases and graders](ch044-cases-and-graders.md) | VI | 28 | 44 | 43 |
| [45. A two-arm run](ch045-a-two-arm-run.md) | VI | 29 | 45 | 18, 44 |
| [46. A claim's key](ch046-a-claims-key.md) | VI | 30 | 46 | 45 |
| [47. Reproduction by another key](ch047-reproduction-by-another-key.md) | VI | 31 | 47 | 46 |
| [48. External validation](ch048-external-validation.md) | VI | 32 | 48 | 47 |
| [49. Adoption is policy](ch049-adoption-is-policy.md) | VI | 33 | 49 | 48 |
| [50. Credit](ch050-credit.md) | VI | 34 | 50 | 47, 49 |
| [51. Revalidation](ch051-revalidation.md) | VI | 35 | 51 | 46, 49 |
| [52. The Gym](ch052-the-gym.md) | VI | 36 | 52 | 41, 49, 50, 51 |
| [53. Why an open market](ch053-why-an-open-market.md) | VI | 37 | 53 | 52 |
| [54. Releases as signed events](ch054-releases-as-signed-events.md) | VI | 38 | 54 | 38, 53 |
| [55. Everyone gets it](ch055-everyone-gets-it.md) | VI | 39 | 55 | 49, 54 |
| [56. Paid calls](ch056-paid-calls.md) | VI | 40 | 56 | 32, 33, 54 |
| [57. Metering and receipts](ch057-metering-and-receipts.md) | VI | 41 | 57 | 56 |
| [58. Payouts to authors](ch058-payouts-to-authors.md) | VI | 42 | 58 | 32, 57 |
| [59. One identity](ch059-one-identity.md) | VI | 43 | 59 | 17, 32 |
| [60. Reputation from reproduced claims](ch060-reputation-from-reproduced-claims.md) | VI | 44 | 60 | 50, 57, 59 |
| [61. Markets for work](ch061-markets-for-work.md) | VI | 45 | 61 | 26, 58, 60 |
| [62. Signed JSON over WebSockets](ch062-signed-json-over-websockets.md) | VI | 46 | 62 | 18 |
| [63. Every step is an event](ch063-every-step-is-an-event.md) | VI | 47 | 63 | 52, 61, 62 |
| [64. The OpenAgents API](ch064-the-openagents-api.md) | VI | 48 | 64 | 4, 63 |
| [65. One schema, three carriages](ch065-one-schema-three-carriages.md) | VI | 49 | 65 | 62, 64 |
| [66. The compliance rule](ch066-the-compliance-rule.md) | VI | 52 | 66 | 65 |
| [67. Host it yourself](ch067-host-it-yourself.md) | VI | 53 | 67 | 55, 64 |
| [68. One gateway](ch068-one-gateway.md) | VI | 54 | 68 | 15 |
| [69. Models we serve](ch069-models-we-serve.md) | VI | 55 | 69 | 16, 68 |
| [70. Pylons](ch070-pylons.md) | VI | 56 | 70 | 58, 69 |
| [71. The Metaverse is a surface](ch071-the-verse-is-a-surface.md) | VII | 57 | 71 | 10, 12 |
| [72. Agents you can meet](ch072-agents-you-can-meet.md) | VII | 58 | 72 | 26, 71 |
| [73. Realms and makers](ch073-realms-and-makers.md) | VII | 59 | 73 | 58, 59, 71 |
| [74. A decision in the world](ch074-a-decision-in-the-world.md) | VII | 60 | 74 | 57, 70, 73 |
| [75. Cross-host conformance](ch075-cross-host-conformance.md) | VII | 61 | 75 | 6, 10, 37, 64, 71 |
| [76. Releases from one source](ch076-releases-from-one-source.md) | VII | 63 | 76 | 75 |
| [77. A Windows host](ch077-a-windows-host.md) | VII | 62 | 77 | 70, 75 |
| [78. What 1.0 means](ch078-what-one-point-zero-means.md) | VII | 64 | 78 | 52, 61, 67, 70, 74, 76 |
<!-- map:end -->

## Promise coverage

Every promise from the audit's
[section 1](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)
is covered by at least one chapter. The gap for each is in the audit's
[section 4](../../docs/audits/2026-11-10-episode-289-port-audit.md#4-promise-by-promise-the-gap-and-the-shape).
A chapter that covers a promise plans for it; it does not show that the
promise is met.

<!-- coverage:begin -->
| Promise | Chapters |
| --- | --- |
| P1 | 1–4, 6–7, 9–12, 30–31, 34–37, 71, 75 |
| P2 | 20–23 |
| P3 | 8, 13, 24–26, 29–30, 37 |
| P4 | 13, 18, 26, 38, 72 |
| P5 | 27, 38–41, 44–45, 52 |
| P6 | 19–20, 27, 52 |
| P7 | 14–16, 68–69 |
| P8 | 17, 28, 32–33 |
| P9 | 42 |
| P10 | 49, 53–55, 73 |
| P11 | 32, 53, 56–58, 61, 70, 73–74 |
| P12 | 4, 18, 53–54, 61–63, 65–68, 70, 74 |
| P13 | 4, 11, 29, 37, 64, 67, 71, 75 |
| P14 | 5, 17, 23, 32, 50, 59–60 |
| P15 | 40, 43–52 |
| P16 | 6, 10–11, 75–78 |
<!-- coverage:end -->

## Order of building

Each step ships something a person can use and is the base for the next.
Desktop (`apps/openagents-desktop`) leads as the primary host, sharing its
Coax logic in `apps/shared/` and `openagents/core/` with mobile and web:

| Step | What ships | Chapters | Targets |
| --- | --- | --- | --- |
| Step 1: the chat page | Shared GPU chat page (desktop primary + mobile consumer): text input, Send, transcript, streamed reply with Stop, login card (`cpu` verified) | 1–6 | `cpu`, `macos-arm64`, `ios-simulator`, `ios-device` |
| Step 2: sidebar, chips and settings | History and sidebar, environment and repository chips, settings (`cpu` verified) | 7–9 | as step 1 |
| Step 3: mobile and other native hosts | Full native host parity across iOS (`#14023`, `#14068`), Linux Vulkan, Windows Win32/Vulkan (`#14046`), and Android | 10, 75, 77 | `macos-arm64`, `ios-device`, `android-emulator`, `android-device` |
| Step 4: the web | Coax Web, Coax UI Core/Kit/Router, 0-KB-Wasm SSR books (`cpu` verified), plus migrating `/chat`, history and settings onto shared state | 11 | `cpu`, `web` |
| Step 5: one agent | Unified `openagents/core/` loop, `#authority` members, typed `RoutePlan` router, grants, records (`12–13, 15, 17` `cpu` verified), plus shared `NIP-KB` knowledge and memory (`#14058`) | 12–23 | `cpu`, `macos-arm64`, `ios-device` |
| Step 6: capabilities | Coder (`oa-worker`), opt-in external engines (`#14071`), visible subagents (`#14096`), CLI allowlist (`#14057`), files/browser/voice, Desktop+iOS Spark wallet & bypass-immune `WalletPay` (`26, 28, 32–33` `cpu` verified) | 24–33 | as step 5 |
| Step 7: continuity | Persistence, checkpoints (`#14026`), sync, resident `coder-host` background survival & Tailnet/LAN/Nostr failover (`#14027`, `#14030`, `#14032`) | 34–37 | as step 5 |
| Step 8: plugins, evidence and the market | OpenAgents Studio & plugin sandbox (`#14059`), Gym & evidence rules, signed registry (`54–55` `cpu` verified), 3-leg author royalties (`#13961`, `58` `cpu` verified), public `NIP-REG`/`EVAL`/`PAY` market (`#14060`) | 38–61 | as step 5 |
| Step 9: the protocol and compute | Protocol compliance, OpenAgents Cloud API & Usage HUD (`#14064`, `#14069`–`#14072`), local `$0` Psionic serving (`#14028`, `#14087`, `69` `cpu` verified), Pylons (`70` `cpu` verified) | 62–70 | as step 5 |
| Step 10: worlds and delivery | Embedded Metaverse & GameBuilder pane (`#14038`–`#14046`), cross-host conformance, releases from one source, Windows engine conformance, and 1.0 | 71–78 | every target |

**The desktop and environment service (reconciled with shipped `main`).**
[`apps/openagents-desktop`](../../apps/openagents-desktop) is the primary
unified desktop host across macOS Metal 4, Linux Vulkan, and Windows
Win32/Vulkan (`#14046`). It attaches to the independent resident `coder-host`
environment service (`#14027`, `#14032`) so closing the window never stops an
active agent turn, and reaches paired phones over Tailnet → LAN → Nostr relay
failover (`#14030`). See the
[canonical stages and gates](../../docs/protocol/environments-claude-and-world-editing.md#10-implementation-sequence-and-completion-gates).
Full Metaverse Engine Windows conformance (chapter 77) qualifies on its own
evidence and never blocks macOS, Linux, web, or mobile.

## What the new approach keeps from the first plan

The opening of 2026-11-13 (old chapters 1 to 11), its contract and its plan
held ideas the new iOS app should keep. Some it already has; some the old
plan did better, and the new app should adopt them.

**Keep, and where each applies now:**

1. **The four-part chapter check** (source, exact trace, refusal,
   independent check). It is what made the 2026-11-13 milestone credible.
   Every new opening chapter plans one ([CONTRACT.md](CONTRACT.md)).
2. **Common `coax drive` operations on every host**, with no private test
   language per host. The GPU login screen's driver answers `tree`, `tap`,
   `type` and `counts`; the old shared module also answered `state`,
   `clock` and `logs`. Chapter 6 needs the whole set. The old plan is ahead
   here.
3. **Driver input through the real input path.** The old rule was that
   `tap` and `type` never write state directly, and both old hosts refused a
   driver tap that no real press delivered. Today the login screen's driver
   `tap` presses the row's action directly and `type` sets the field's
   value, so hit testing and the editor path are not exercised. Adopt: a
   driver tap enters as a pointer down and up at the row's committed bounds,
   as the screen's own checks already do, and `type` enters where the
   editor's edits enter (chapters 1 and 6).
4. **Fake services with held answers and a controlled clock** (`clock data
   release NAME`, the pinned seed, date, locale and zone), so a trace is
   byte-exact with no network. The GPU login screen is checked today against
   a local fixture server and an XCUITest, which is slower and timed by the
   host. The old plan is better here; chapter 4 adopts it.
5. **A cpu host for cases.** The old case runner (`apps/shared/cases.coax`)
   played every case on the build host with no simulator and compared bytes.
   Keep a cpu host for the new cases, so the trace runs in seconds on a build
   host and the simulator run adds only what needs a simulator (chapter 6).
6. **Meaning before pictures.** Assert on state and the semantic tree first;
   a screenshot last. On the GPU kit everything is pixels, so the committed
   semantic rows are the anchor for checks, accessibility and the driver
   alike (chapters 1 and 6).
7. **Effect outbox semantics.** Taking a request means accepted or refused,
   not succeeded; states are pending, done, failed or unknown; an unknown
   outcome never retries on its own; an operation ID makes a retry safe
   (chapter 4).
8. **The late-answer fence.** Stop, logout and leaving a screen drop late
   answers; the old `logout-pending` and `reconnect` cases are the model for
   chapters 4 and 5.
9. **Typed routes, and a link is input, not permission,** with a return
   target through login. These arrive with the second place to go
   (chapter 7) and with URLs on the web (chapter 11).
10. **One execution owner per task.** A client presents a turn and never runs
    it again. It first shows in the app with the environment chip
    (chapter 8), and chapter 37 builds it.
11. **The three meanings of "one app"**: shared code, shared data, one
    execution owner. Chapter 10 keeps the old chapter 1's statement.
12. **The device evidence list.** The old milestone ran on a physical
    iPhone: the number pad on the code field, a Japanese input method
    sending `日本語` once, non-Latin text and an emoji, labels and the
    accessibility audit, a reply finishing once across backgrounding. The
    GPU login screen has run none of these yet. Chapter 6 runs the same list
    again.
13. **Who may drive a phone**: TCP on the phone's own network, a launch
    token, one driver at a time. Decided on 2026-11-13 and unchanged
    (chapter 6).
14. **The stale-build refusal.** The driver checks the app was built from
    the source under test. The web has it; the iOS driver still lacks it
    (chapter 6).
15. **One schema per record, the decision in Coax, and a versioned API.**
    With no over-the-air updates, an installed app can lag the service for
    weeks and a bad build is fixed only forward, so the version rule matters
    more than before (chapter 4).
16. **Tokens stay native.** No token or pending handle enters Coax, a trace
    or a log. The GPU login screen already keeps to this (chapter 5).
17. **Stable IDs from role and domain key, never an index,** and a duplicate
    refused, not shared. The G0 foundation enforces this in the keyed store
    and the surface commit (chapters 1 to 3).
18. **Show only what works, and every limit with its number** (chapter 9).
19. **A public page is a web page first,** with one source for the page and
    its Markdown twin (chapter 11).
20. **Large text.** The old shared screens were fixed to follow the text
    size after the device audit. The GPU login screen draws at a fixed scale
    today. The old app is ahead here; chapter 9 makes it a requirement and
    chapter 5 applies it to the login card.

**Dropped, and why:**

- **Shared DOM and UIKit presentation on iOS** (old chapters 3 and 11). The
  GPU kit draws every native component; the UIKit control projection is
  deleted.
- **Coax as Wasm or JavaScript in WebKit or JavaScriptCore, and the Coax
  Native starter as the base** (old chapter 1 and the README). The app is
  compiled Coax linked into the binary.
- **Over-the-air Updates** (the old README and chapter 76's rule for what may
  go over the air, and the signed previous package as a way back). Dropped
  by the owner. The cost is a slower fix path; chapter 4's version rule and
  chapter 76's fix-forward carry it.
- **The web and the iPhone as twin first targets** (old chapter 6). iOS is
  first; desktop and Android follow (chapter 10); the web is chapter 11.
  Cross-host byte equality moves to chapters 10, 11 and 75.
- **The public website in the opening** (old chapter 7). The iOS app does
  not need it; it moved to chapter 11.
- **A routes chapter before any screen needs a route** (old chapter 4).
  Routes arrive in chapter 7, with the second place to go.
- **The logged-in shell as one chapter** (old chapter 10). It split into the
  sidebar (chapter 7) and settings (chapter 9).
- **Standalone example programs for the opening chapters**
  (`app-state.coax`, `login-screen.coax`, `routes.coax`, `effects.coax`).
  They would copy the app's code into the book. The new checks run against
  the app's own modules and its `.ops` cases. The cost: `book.py --coax`
  runs none of the opening, so item 5's cpu host is what keeps those traces
  cheap.
- **Login as the first screen.** The code on `main` built the login screen
  first (stage G1). The book starts from the composer instead and reuses
  that screen as chapter 5's login card.

## Not written yet

- `PORT.md`: one line for each of the 229 crates at `coder-v1.0.0`.
- `PROTOCOL.md` (chapter 63) and `ONE-POINT-ZERO.md` (chapter 78).
- The drafts of chapters 35, 65 and 66, which are planned outlines.
- The capstones "A maker is paid" and "Coder takes a job from the queue",
  which chapters 73 and 61 point to.

## Open questions for the owner

The audit lists five decisions in its
[section 8](../../docs/audits/2026-11-10-episode-289-port-audit.md#8-decisions-for-the-owner).
Several major questions are now **settled on `main`**:

- **Directory & ownership (`decision 1`):** `openagents/book/` holds this book,
  and [`openagents/core/`](../../openagents/core) holds the unified 24-layer
  pure-Coax agent runtime.
- **Wallet & execution ownership:** the self-custodial Breez Spark wallet runs
  across Desktop, iOS, and CLI over `#authority` `pay/` (`#14057`, chapter 32),
  with bypass-immune `WalletPay` approval (chapter 33); every agent task has
  one execution owner via resident `coder-host` (`#14027`, `#14032`, chapter 37).
- **Authentication:** locked **WorkOS hybrid `acct_` spine** (`#14097`–`#14103`,
  [`docs/auth/2026-11-26-workos-coax-sdk.md`](../../docs/auth/2026-11-26-workos-coax-sdk.md))
  with Custom Coax UI, social OAuth/SSO at `L1`, and first-party Passkey/Nostr
  `L2`/`L2-fresh` step-up (chapters 5 and 59).
- **Default engines, gateway & local serving:** native `openagents/core/`
  (`oa-worker.coax`) + OpenAgents Cloud retail gateway (`#14069`–`#14072`) +
  `$0.00` local `psionic-inkling-serve` (`Inkling-2-Mini`, `#14028`, `#14087`),
  with external coding engines (Claude Code, Codex, Cursor, Antigravity)
  explicitly opt-in (`#14071`, chapters 25, 68, 69).
- **Executor author royalty cap:** 3-leg settlement (`Worker + Validator + Module Author`)
  capped at `<= 25%` (`2500 bps`) in `pay/executor-settlement.coax` (`#13961`,
  chapters 58 and 70).
- **Windows graphics (`decision 5`):** **Vulkan** is the shared desktop
  graphics API across Linux and Windows (`apps/openagents-desktop/windows/`,
  `#14046`, chapter 77).

The chapters still ask these genuinely open owner questions:

<!-- questions:begin -->
There are 78 of them.

| Chapter | Question |
| --- | --- |
| [1](ch001-a-text-input-on-the-gpu.md) | Does the first TestFlight build of the lean app ship with only this chapter's text input, or does it wait until chapter 4 gives it a reply? |
| [4](ch004-a-reply-from-the-service.md) | Which service answers in this chapter, before chapter 12: the existing `/chat` route with its live models, or only the fake service until our own loop exists? |
| [5](ch005-logging-in-when-a-send-needs-it.md) | Does the lean app's login card include "Use a Nostr key" in this chapter, or only email until the key login is rebuilt on the kit? |
| [5](ch005-logging-in-when-a-send-needs-it.md) | Do a member's grant (chapter 17) and an agent's grant become one record, or stay two? |
| [5](ch005-logging-in-when-a-send-needs-it.md) | Which scopes come after `profile:read` and `wallet:spend`? |
| [8](ch008-environment-and-repository-chips.md) | Which environments does the first build list: this account's hosted computer, your enrolled computers, or both? |
| [10](ch010-mobile-and-other-native-hosts.md) | The [sync spec](../../docs/sync/README.md) recommends syncing a conversation through our service, readable by the server at first (chapter 35). Confirm that, and say which collections get end-to-end encryption first. |
| [13](ch013-members.md) | What does the product call these? The essays say "members"; Episode 289 says tools, subagents and plugins. |
| [14](ch014-a-typed-plan.md) | The audit's plan carries a budget. Our rule is that we impose no limits on a run and show any limit the user sets. Should a plan carry only limits the user set? |
| [15](ch015-decide-before-you-generate.md) | Which of our own decision models backs up Gemini Decision 1 for the front at 1.0, and what result would move our own model to the first door? |
| [15](ch015-decide-before-you-generate.md) | What does the front do when no decision route answers: plain conversation with no routing, as the old tree did, or a visible error? |
| [16](ch016-measure-the-front.md) | Where do the labeled messages come from now? The old set is at the tag. New ones would come from real conversations, which needs a consent rule and must leave out our own testing. |
| [17](ch017-grants-and-approvals.md) | Which grants can a person give once and keep, and which ask every time? |
| [18](ch018-on-the-record.md) | Confirm the default: records are private, and publishing one is a separate, explicit act. |
| [19](ch019-conversation.md) | Which model answers general conversation by default, and may a person choose another? |
| [19](ch019-conversation.md) | Where does the written policy live, and who can change it? |
| [20](ch020-prepared-answers.md) | Who reviews a prepared answer before it is served: only us, or trusted contributors too? |
| [21](ch021-a-knowledge-base.md) | Which of the old tree's notes come back? Many were learned from benchmark tasks and need new evidence. |
| [21](ch021-a-knowledge-base.md) | Is every entry about OpenAgents public, or are some for logged-in people only? |
| [22](ch022-retrieval-by-meaning.md) | Which model makes the embeddings: a hosted one through the gateway, or one we serve with Psionic? |
| [23](ch023-memory-and-consent.md) | Where does memory live: on the device only, encrypted on a relay, or on our service? |
| [23](ch023-memory-and-consent.md) | Which sharing model do we offer first: none, opt-in publishing, or opt-in sale? |
| [27](ch027-programs.md) | Which of the twenty recipes come back, and are they members of the general agent or examples in this book? |
| [28](ch028-the-command-line-behind-the-agent.md) | Beyond the shipped 8-subcommand allowlist (`openagents/core/members.coax`), which additional CLI commands should remain permanently denied from chat? |
| [29](ch029-where-work-runs.md) | Does 1.0 offer hosted computers to everyone, and at what price? Episode 276 named one dollar an hour at the time. |
| [29](ch029-where-work-runs.md) | How does hosted compute fit inside the monthly cloud budget as use grows? |
| [31](ch031-voice.md) | Which voice reads replies aloud by default, and may a person choose another? |
| [32](ch032-a-wallet.md) | Do we offer a way to buy Bitcoin with a card, as Episode 290 shows, and through whom? |
| [33](ch033-spending-approval.md) | May a person give an agent a standing allowance in 1.0, or does every payment ask? |
| [35](ch035-sync-across-devices.md) | Confirm the sync spec's recommendations S1 to S6, including the 1 GB limit for each account. |
| [35](ch035-sync-across-devices.md) | Which collections get end-to-end encryption first? |
| [38](ch038-the-plugin-interface.md) | Do we read Claude Code's plugin and skill formats directly, or only our own package format? |
| [39](ch039-the-sandbox.md) | Wasm with grants, as the audit recommends, or native processes with an operating-system sandbox? This is the audit's decision 2. |
| [39](ch039-the-sandbox.md) | Which Wasm runtime runs plugins on native hosts: one we write in Coax, or an existing one we link? |
| [41](ch041-write-a-plugin.md) | Which of the nine do we port first after the repository map? |
| [42](ch042-the-authoring-interview.md) | Who pays for the compute an authoring run uses: the person, us for a first run, or the first paid use of the plugin? |
| [42](ch042-the-authoring-interview.md) | A plugin that files tickets needs the person's login to another service. Does 1.0 include a vault for such logins, or do first plugins avoid them? |
| [45](ch045-a-two-arm-run.md) | How many cases does a test set need before its result may be published? The first adoption used six. |
| [48](ch048-external-validation.md) | How do we get the first validation written by someone outside the company? |
| [49](ch049-adoption-is-policy.md) | What does the policy require before something becomes a default for everyone: how many confirmations, how many validations, and from whom? |
| [49](ch049-adoption-is-policy.md) | Is there one default set for the whole agent, or one per member, as Coder had? |
| [50](ch050-credit.md) | Should verification also earn sats, for example a bounty for a check or a validation, or does it stay points only? |
| [51](ch051-revalidation.md) | How much compute do we set aside each month for re-running old claims, inside the cloud budget? |
| [52](ch052-the-gym.md) | Who pays for a run on the hosted runner: the trainer in sats, us up to a free amount, or both? |
| [54](ch054-releases-as-signed-events.md) | Do we mirror releases from any relay by default, or only from a list we choose? An open mirror needs a plan for spam. |
| [55](ch055-everyone-gets-it.md) | Does a defaults update install on its own, or does each person confirm it? |
| [56](ch056-paid-calls.md) | What can an author price: a call, a unit of work, a subscription? |
| [56](ch056-paid-calls.md) | Do we offer card-funded credits as Episode 290 describes, with a fee and a Bitcoin discount, and what are the numbers? |
| [56](ch056-paid-calls.md) | Is there a free amount of paid calls for new people, and who funds it? |
| [57](ch057-metering-and-receipts.md) | How public is the ledger: every receipt, or totals per release with receipts shown only to the author and the payer? |
| [58](ch058-payouts-to-authors.md) | What is the general plugin payout schedule and minimum (beyond Executor Network 3-leg royalties capped at 25%)? |
| [58](ch058-payouts-to-authors.md) | When a plugin depends on another paid plugin, how is the price split between the two authors? |
| [58](ch058-payouts-to-authors.md) | A tip goes straight from the payer to the maker's own wallet (owner decision D6 in [docs/auth](../../docs/auth/decisions.md), built in #13210). A paid call with a split can't always do that. Who holds the money between such a call and the payout, and under what custody? |
| [59](ch059-one-identity.md) | When does a person who logged in by WorkOS email/OAuth get a Nostr key for signing public records, where is it made, and how do they take it with them? |
| [60](ch060-reputation-from-reproduced-claims.md) | What makes two keys distinct enough to count as independent confirmations? |
| [61](ch061-markets-for-work.md) | Who accepts a result: the buyer alone, or an independent checker when the buyer asks for one? |
| [61](ch061-markets-for-work.md) | Do the terms of the subscriptions people would lend allow this use? We need an answer per provider before we ship worker mode. |
| [63](ch063-every-step-is-an-event.md) | Which gaps do we close before 1.0, and which do we ship with and name? |
| [64](ch064-the-openagents-api.md) | Does the API include an endpoint in the shape other vendors' clients already speak, so existing tools can point at it? |
| [64](ch064-the-openagents-api.md) | What does API use cost a partner, and how does that sit with the author's share of paid calls? |
| [65](ch065-one-schema-three-carriages.md) | Which record is the worked example: a release, which is public, or a grant, which is private? |
| [65](ch065-one-schema-three-carriages.md) | Do chapters show schema text, or only link to `proto/`? |
| [66](ch066-the-compliance-rule.md) | Does a chapter reach "implemented" only when its records are compliant? |
| [66](ch066-the-compliance-rule.md) | Do we publish the fixtures as a kit that a second implementation can run? |
| [67](ch067-host-it-yourself.md) | How are authors paid when a self-hosted instance runs a paid plugin locally: by license terms, by plugins that call home for payment, or not at all? |
| [70](ch070-pylons.md) | As public operators join beyond our own machines, should per-job payouts stay fixed or move to open market bidding under daily caps? |
| [70](ch070-pylons.md) | What trust/reputation threshold graduates a new public Pylon from Tier-2 redundant replication to sampled validation? |
| [70](ch070-pylons.md) | Beyond our own coding agents and the Metaverse, who is the first outside buyer? |
| [72](ch072-agents-you-can-meet.md) | Do agents appear in shared public realms by default, or only in your own space until you choose to show them? |
| [73](ch073-realms-and-makers.md) | Is the maker's share the same as a plugin author's, and what does a fork's maker owe the original's? |
| [73](ch073-realms-and-makers.md) | Who curates the featured list, and by what written rule? |
| [74](ch074-a-decision-in-the-world.md) | Who pays for a guard's decisions: the realm's maker, the player, or us while the network starts? |
| [75](ch075-cross-host-conformance.md) | Does Android get the full OpenAgents app for 1.0, or does the Android game app come first and the agent follow? |
| [76](ch076-releases-from-one-source.md) | Which stores do we publish to at 1.0, and which platforms get a direct download only? |
| [76](ch076-releases-from-one-source.md) | Which kinds of update may install on their own, and which ask the person first? |
| [77](ch077-a-windows-host.md) | Do we need a Windows machine in the build fleet, or do we build from another platform and test on a rented one? |
| [78](ch078-what-one-point-zero-means.md) | Is 1.0 one release of everything, or does each surface reach 1.0 when its own chapters run? |
| [78](ch078-what-one-point-zero-means.md) | Which of the sixteen promises may ship marked "gated" at 1.0, if any? |
<!-- questions:end -->
