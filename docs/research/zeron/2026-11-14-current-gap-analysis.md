# OpenAgents compared with Zeron: current implementation gaps

> [!NOTE]
> **Status (2026-11-22): Historical 2026-11-14 point-in-time snapshot (`097e11c86d`, iOS 1085).**
> OpenAgents has since moved Desktop and iOS to compiled Metal/Vulkan GPU (resolving the UIKit/JSC notes and the desktop op 14/15/16 host-contract mismatch), shipped Inky + Psionic Metal 4 / CUDA, Antigravity 1.5+, `coder-host` + Tailscale pairing, and the 29-flag `--minimal` release build. For newer mobile Zeron comparisons see [`docs/audits/2026-11-20-mobile-ui-gap-analysis-zeron.md`](../../audits/2026-11-20-mobile-ui-gap-analysis-zeron.md); for current T3 Code comparisons see [`docs/t3/README.md`](../../t3/README.md).

2026-11-14. OpenAgents source baseline: `097e11c86d39254390fbbb1316aa5728a074f04e`;
latest recorded iOS release: **1085**. Zeron reference: the clean local checkout
at `~/work/projects/repos/zeron`, upstream `zeronsh/zeron`, pinned to
[`fbf5199a699c48fbb4917e6caa882ec8dfceaa8f`](https://github.com/zeronsh/zeron/commit/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f)
(Cargo workspace version `0.3.4`). This is a source comparison, not certification
of the latest distributed Zeron app or its production services.

## Assessment

The most useful Zeron pattern for OpenAgents is the separation between a
persistent agent engine and any particular app window. Zeron combines that
with durable session documents, an explicit command plane, queued drafts,
workspace identity, optional device sync and a native mobile viewport onto the
same state. Its usage/account work is part of that larger engine model.

OpenAgents already has an independently running environment host, a native
phone client, bounded Claude history, managed Claude turns, isolated accounts
and a Usage screen. It also has native content Projects, playable streamed
worlds and domain-specific world editing. The remaining gap is not simply
adding another account meter: it is continuity of sessions, drafts, commands,
workspace context and provider state across devices and restarts.

Adopt the ownership and recovery disciplines in shared Coax. A Rust/GPUI port,
a second Markdown renderer, or automatically adding a CRDT is not required.
Choose synchronization machinery only after the protocol defines who may
write which state and how commands are admitted.

## Evidence limits

The comparison covers the actual `apps/zeron` entry point, Rust engine/document
and account modules, and `apps/ios/Zeron`. The separate `apps/zgx` interface is
not treated as the default released app. Zeron's architecture notes include
plans and milestones; claims below also identify concrete implementation
paths. No reference installer, script, binary, test suite or authenticated
provider was run. No credential store was inspected.

For OpenAgents, **present** means an implemented source path; **partial** means
a narrower flow or lifecycle; **library only** means no complete app integration;
**planned** means no implementation guarantee. The [1085 release record][release]
records TestFlight availability and prior checks, including eight native/UI
checks on a simulator. Those results were not rerun here and do not certify
every desktop, provider or physical-phone scenario. Absence findings concern
the compared app path, not every experimental module in the repository.

## Architecture and authority

| Concern | Zeron at the pinned revision | OpenAgents now | Gap or deliberate difference |
| --- | --- | --- | --- |
| Engine and UI | Headed or headless engine; GPUI desktop viewport; native UIKit app through Rust client/mobile bindings | Standalone `coder host serve`; Coax application modules; UIKit/AppKit/JavaScriptCore and Linux generated C/GTK hosts | The engine/window separation is already started. Recovery, platform effects and common app state are less complete. |
| Local and online profiles | Startup `WorkspaceScope` fixes Local, Synced or Development storage/transport scope independently of later auth changes | Local desktop environment access plus account-authorized remote environments and account-owned web/project state | Local access exists, but no equivalent unified session registry with explicit local/synced profile semantics is implemented in this app. |
| Session storage | Loro session documents and registry; SQLite persistence; optional Cloudflare room sync and attachment storage | Host managed JSONL/receipts, observed CLI files, server-saved account chats and separate project/world services | No common durable read/delivery model across these families. Different authorities must remain explicit. |
| Device access | Same-account trusted devices, typed relay-forwardable workspace RPC | Account → host discovery → exact device/grant/epoch/HOST admission | Retain OpenAgents' finer authorization boundary; do not import Zeron's trusted-peer assumptions wholesale. |
| Execution | Host owns provider harness and transcript outcomes; clients submit commands | Host owns managed Claude process; observed CLI history is inert | Similar basic direction. OpenAgents restart reconciliation and rich interactions remain bounded. |
| Mobile rendering | Shared Rust transcript/display-list engine, native CoreText rows and UIKit controls | Shared Coax Markdown/components and native UIKit adapters; Metal for playable worlds | Both are native approaches. Zeron's rendering/caching model offers concrete performance ideas, not a requirement to replace Coax. |

Sources: Zeron [entry point][z-entry], [architecture][z-architecture],
[command plane][z-commands], [document host][z-chat-host],
[native session source][z-session], and OpenAgents [runtime contract][runtime],
[provider account contract][accounts] and [native host][native-host].

Signing into OpenAgents, signing a computer into Claude, authorizing a phone to
control that computer, and choosing where a conversation is stored are four
different operations. Zeron's explicit separation between `WorkspaceScope`
and `AuthState` is particularly useful: refreshing a login must not silently
upload local history, switch an open store or redirect a pending command.

## Capability gap matrix

Priorities: **P0** protects the current multi-computer workflow; **P1** completes
the workbench; **P2** expands reach. They are proposed order, not estimates.

| Area | Zeron implementation evidence | OpenAgents status and exact gap | Priority |
| --- | --- | --- | --- |
| Engine lifetime | [Managed engine][z-managed] can outlive windows, reconnect and replace its runtime under defined conditions | **Present/partial:** environment service independently owns CLI processes. Four managed slots; after restart, saved histories become observe-only/unknown rather than a qualified resumed writer. | P0 |
| Session registry and sidebar | Spaces are device/folder pairs; registry contains devices, chats, checkout/repository identity and status; attention-sorted session list | **Partial:** drawer, computer groups, title/computer search and native content Projects exist. No equivalent durable cross-device registry, attention lifecycle, repo/checkout grouping or separation of local tabs from shared archival state. | P1 |
| Offline reading and scope isolation | Local documents; optional sync; startup-selected local/synced stores; existing local sessions are not automatically uploaded on login | **Partial:** local environment access and hosted account history are available, but phone state is not an equivalent persisted offline session mirror. Profile/account changes need explicit cache and pending-command ownership. | P0 |
| Commands and acknowledgement | Durable immutable per-device entries, host-only outcomes, cancellation/expiry/supersession and processed-command ledger in [commands][z-commands] | **Partial:** durable normalized host intent, command IDs, expected revision/generation and same-request recovery exist. No complete persisted multi-device command lifecycle in the app. A receipt is not a finished provider effect. | P0 |
| Editable send queue | [Queue document][z-queue] separates mutable unsent messages from commands; stable identities and attachment commitment; lost editor lease becomes review-required | **Missing equivalent app path:** in-memory draft slots and one active request are not an offline editable queue. Mid-turn steering, queued follow-ups and draft handoff across devices are not complete environment features. | P1 |
| Crash recovery and persistence | Document/cursor persistence, run journal and engine-owned recovery paths | **Partial:** saved history and receipts preserve evidence; full native process/session reconciliation is explicitly unfinished. No equivalent demonstrated recovery spanning app death, host death, transcript cursor and queued attachments. | P0 |
| Claude accounts | Account snapshots, live/saved account slots, authentication, per-chat pins and usage cache | **Present with a different credential strategy:** host-isolated `CLAUDE_CONFIG_DIR` profiles, CLI-driven phone-assisted login, usage, default selection and existing-chat pinning. No need to reimplement Claude accounts wholesale. | P1 |
| Usage presentation | [Account usage ring][z-usage-ui] shares the settings cache, targets the chat's device, follows model-specific pools where supported and probes with throttling | **Present/partial:** Settings → Usage reports Claude windows, remaining/reset/staleness and login state per connected host. No equivalent contextual composer account/usage affordance or cross-provider account catalog. | P1 |
| Codex and broader providers | Engine/harness implementations and provider-specific account/usage normalization, including [Codex windows][z-plan-usage] | **Library only for mobile Codex:** app-server client exists, but the environment service/app path remains Claude. Codex authentication, account selection, usage and full interactive turn path are not integrated. Do not infer uniform Zeron account/pin capabilities for every harness. | P1 / P2 |
| Rich transcript and questions | Native transcript rows, tools, questions, images, subagent companion and visuals; [session source][z-session] consumes typed core state | **Present/partial:** native Markdown, compact tools and model/effort controls. Environment item wire is text-oriented; no equivalent typed question/plan/subagent/attachment/visual model. Account/project-specific features do not fill that environment wire gap. | P1 |
| Long transcript performance | [RowModel][z-row] derives CoreText lines off the main thread and caches by row/version/width; dedicated transcript list | **Unqualified parity:** shared Coax/native rendering exists, but no equivalent measured large-history scroll/update budget is recorded here. Bounded 6,000-byte pages are not proof of smooth long-history behavior. | P1 |
| Files, checkout diffs and terminal | Workspace host/mutation services, diff sync, terminal engine and native terminal companion | **Missing complete mobile environment flow:** project uploads and world editing are present; repository browsing, interactive PTY and checkout-aware diff review are not equivalent app features. | P1 |
| Device setup and platforms | Headed/headless CLI and managed runtime; native iOS plus desktop engine integrations | **Partial:** Mac/Linux replacement sources and owner setup exist. GUI enrollment/workspace management remains limited; shared screens have mismatched/missing desktop effects. Windows is planned, not qualified. | P0 / P2 |
| Voice and remote visuals | Native voice/media and visual-source modules connect to the session's host | **Separate capabilities:** OpenAgents has shared voice and world rendering modules, but the environment chat composer does not expose a qualified comparable voice/remote-visual flow. Its microphone delegates to keyboard dictation. | P2 |
| Update transactions | [Updater transaction][z-update] stages binary promotion, readiness, watchdog/rollback and scoped database recovery | **Different/partial:** signed Coax Native Updates, compatible runtime selection and fresh-install dedup are present. They do not give provider-process reconciliation or transactional rollback of every database/file mutation. | P1 |

OpenAgents source anchors: [navigation][navigation], [composer][composer],
[environment UI][environment-app], [environment schema][schema],
[host][host], [Claude library][claude], [Codex library][codex],
[native Projects][projects] and [Updates specification][updates].

## Detailed findings

### 1. Account login and usage: preserve what is already implemented

The [provider accounts contract][accounts] implements much of the original
Zeron-inspired request. It distinguishes the system CLI account from isolated
additional profiles; limits inventory to eight accounts; starts login on the
selected computer through its CLI; lets the phone open the permitted Claude
login URL and return the one-use code; and prevents changing the profile of an
existing managed chat. This is phone-assisted CLI authentication, not a promise
of a generic OAuth device flow or passwordless automatic sign-in.

Usage reads are local to the computer and return observations. Five-hour,
weekly and model-specific windows remain distinct; stale/error/missing data is
not displayed as full remaining quota. The existing five-minute cache and
30-second forced-probe throttle are sensible and similar to Zeron's UI cadence.
The Claude endpoint is provider-internal, so expiry, rate limiting and response
drift need explicit failures and fixtures.

Zeron additionally connects usage to the active session/device and exposes
provider-specific account mechanisms. Its [pinning implementation][z-pins]
keeps chat account identity stable, while the broader account manager can
save/swap provider credential slots. OpenAgents should retain isolated CLI
profiles rather than copying credential stores between machines or replacing
the user's system login. The next additions are a small active-account/usage
entry point in the composer, durable profile identity through recovery, and
one complete Codex account/session integration. Avoid adding quotas across
computers when they may describe the same provider account.

### 2. Durable documents and commands solve different problems

Zeron has both a mutable queue and an immutable command ledger. Draft editing
does not mutate an already admitted Run command; outcomes belong to the host;
old interrupts can be superseded without discarding distinct prompts. Its
queue's review-required state after a lost editing lease is a useful model
for avoiding an accidental send after another device disappears.

OpenAgents already guards host commands and retries. The missing layer is
persistent client intent and read state across app termination, followed by
reconciliation against the host's receipt/session identity. Preserve unknown
outcomes rather than replaying new effects. A CRDT is one possible mechanism
for future multi-writer drafts, not a prerequisite for implementing a durable
single-writer outbox and server/host-owned timeline first.

If replicated documents are introduced, specify writers field by field:
clients may propose commands or edit unsent drafts, the execution owner records
outcomes and transcript state, and device presence remains ephemeral. Document
convergence cannot replace permission checks or prove exactly-once effects.
Zeron's processed-command ledger is also not an atomic transaction with every
external tool side effect.

Its [run journal][z-journal] illustrates the distinction: sequence-numbered
JSONL supports replay/tailing; an incomplete run can be marked aborted during
boot recovery, and persisted resume-attempt limits prevent revival loops.
Journal compaction is explicitly deferred there. This is a useful recovery
model with its own bounds, not evidence that Zeron can transparently resume
every interrupted provider process or retain unlimited history efficiently.

### 3. Workspace identity must not be conflated with Projects

Zeron's space is a device/folder pair, and its registry distinguishes repository
identity from a particular checkout. The current OpenAgents host is configured
with one canonical workspace and a separate history root; our content Projects
represent documents, drafts, publication and membership. Both models are useful,
but they solve different problems.

Introduce admitted workspace and checkout references alongside existing Project
IDs. A phone may associate a content Project or world with a selected checkout;
that association needs explicit authority, not an implicit filesystem path.
File access, diffs, mutations and terminals need separate bounded capabilities,
with workspace containment and command lifecycle rules.

Zeron's same-account device model permits trusted remote workspace operations;
some ignored-file access can intentionally include sensitive files. That is
not an appropriate blanket authorization rule for OpenAgents HOST grants.
Retain our per-device/grant admission and explicit handling of credentials,
path traversal, symlinks and stale checkout revisions.

### 4. Native UI parity requires state and performance work

Zeron's [iOS session source][z-session] exposes host capabilities, queue/send
state, branch/model/effort context and typed connection banners. Its transcript
uses Rust-derived display data with cached CoreText layout. The transferable
pattern is to derive expensive display work once per stable item/version/width,
keep the viewport attached to shared state, and avoid recomputing an entire
conversation for every token.

OpenAgents should extend its existing component system for typed tools,
questions, diffs, attachments and virtualized/windowed history. No second
Markdown parser, line wrapper or edit buffer. Measure append/update and
pagination behavior, scroll anchoring, keyboard dismissal, selection, Dynamic
Type and accessibility on a physical phone. Source similarity or a simulator
screenshot cannot establish those results.

### 5. Desktop and updates have distinct compatibility boundaries

The [desktop application projection][desktop-program] shares the iOS Coax
application, but the effect dispatchers lag it. iOS [NativeHost][native-host]
assigns operation 14 to document picking, 15 to upload and 16 to release.
[Mac][mac-host] and [Linux][linux-host] still assign 14 to local environment
RPC and lack the corresponding document operations; world opening is also
absent there. This is a concrete source-level compatibility gap, not a tested
desktop runtime failure. Shared UI must declare required effects and gate
unsupported operations until the hosts agree on their meaning.

Zeron's binary updater is a different layer from Coax Native resource Updates.
Its transaction code explicitly scopes rollback: SQLite can be backed up,
while journals, settings, attachments and saved logins are not all rolled back.
OpenAgents should document similarly precise boundaries. Signed delivery and
boot health do not undo a previously admitted tool call. New native effects
need a binary release; compatible Coax presentation/logic and world assets
can continue to use Updates/streamed packs.

## Recommended sequence and acceptance

| Order | Work | Acceptance evidence |
| --- | --- | --- |
| 1 | Fix host effect contracts and qualify phone → Mac/CoderOS ownership | Desktop document pick cannot dispatch a local RPC. The phone distinguishes computer offline, account expired, grant revoked and CLI signed out; incomplete discovery is never called empty. |
| 2 | Durable client store/outbox plus host restart reconciliation | Kill the app after acceptance but before acknowledgement; reopening queries the same command without a second turn. Restart the host mid-turn/permission wait; either reconcile the rightful process or retain unknown. Account/profile switch cannot attach old pending commands to a new identity. |
| 3 | Specify local/remote storage scope and session registry semantics | Local history stays local when signing in unless explicitly migrated. Offline cached reading is labelled stale. Tab closing, draft clearing and shared archival are different actions. Revocation stops remote access and prevents stale writers. |
| 4 | Typed interactions and durable queue using shared components | Two devices cannot answer the same question twice or send an expired draft-edit lease. Queue reordering/cancellation has defined outcomes; transcript IDs and scroll position survive streaming and pagination. |
| 5 | Complete Codex and contextual account usage | On each computer, phone can authenticate, select account/model/effort, run, inspect tools, approve/deny and stop Codex. Claude/Codex windows retain their actual duration/reset/error semantics. Existing chats retain account identity. |
| 6 | Workspace/checkout, attachment, file/diff/terminal capabilities | Phone chooses an admitted checkout, reviews changes, attaches a file and controls a PTY with explicit lifecycle. Traversal, symlink escape and stale revision cases are refused. Large payloads use bounded artifact transport. |
| 7 | Computer-owned world editing and platform/performance qualification | Claude on the selected computer produces a fenced world revision/artifact; phone previews the current Everglade or Splice Bots and can publish/undo through world authority. Qualify large histories, suspend/resume, Mac/Linux effects and update failure recovery before expanding to Windows/voice/cloud machines. |

Start wire changes in `proto/` and the protocol specification. New versions
preserve published contract identity; each wire/stored shape gets its registry
entry and one codec. Extend shared Coax modules and authority checks, leaving
platform hosts to network, storage, process and UI effects. The existing
[environment/world plan][environment-plan] remains the larger destination;
its future phases are not evidence that those acceptance scenarios pass today.

## Differences worth retaining

- Content Projects, Bitcoin services and playable worlds are OpenAgents
  product scope. They do not need equivalents in Zeron to justify their design.
- Private member-project conversations, account-saved chats, observed CLI
  history and managed environment chats have different authorities. Unify
  discovery and display without silently changing execution owners.
- Isolated Claude profiles and one-shot approval guards are deliberate.
  Improved switching must not copy provider secrets to the phone/gateway or
  rewrite a live chat's account.
- Optional synchronization should have explicit storage and writer semantics.
  Adopting Cloudflare rooms, Loro or a broadly trusted peer filesystem is not
  itself a product requirement.
- The installed world renderer and typed editor already exist. The missing
  environment-owned Claude artifact/preview binding is a separate integration,
  not a reason to rebuild world loading or import a second renderer.

## Source map

All Zeron links below are pinned to the inspected revision. In addition to
the linked modules, the review inspected `crates/engine/src/agent_accounts.rs`,
its `usage.rs`/`oauth.rs` helpers, `crates/doc/src/queue.rs`, and the native
iOS composer/transcript/companion file inventory. Test files were inventoried
as source evidence only. The archived [T3 comparison](../t3code/09-current-gap-analysis.md)
(and the current [`docs/t3/README.md`](../../t3/README.md) hub)
covers server orchestration, connection supervision and worktree restore in
greater detail.

[release]: ../../verification/2026-11-14-native-projects-1085.json
[runtime]: ../../protocol/environment-runtime-v1alpha1.md
[accounts]: ../../protocol/provider-accounts-v1.md
[projects]: ../../protocol/native-projects.md
[environment-plan]: ../../protocol/environments-claude-and-world-editing.md
[native-host]: ../../../apps/shared/native/NativeHost.swift
[desktop-program]: ../../../apps/openagents-desktop/native/program.coax
[mac-host]: ../../../apps/openagents-desktop/native/EnvironmentWindow.swift
[linux-host]: ../../../apps/openagents-desktop/native/linux-services.coax
[navigation]: ../../../apps/shared/app-navigation.coax
[composer]: ../../../apps/shared/native/NativeComposer.swift
[environment-app]: ../../../apps/shared/environment-app.coax
[schema]: ../../../proto/openagents/environment/access/v1alpha1/runtime.proto
[host]: ../../../coder/tui/host-environment.coax
[claude]: ../../../coax/lib/agent/claude/PARITY.md
[codex]: ../../../coax/lib/agent/codex/PARITY.md
[updates]: ../../../coax/native-updates.md
[z-entry]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/apps/zeron/src/main.rs
[z-architecture]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/ARCHITECTURE.md
[z-commands]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/doc/src/commands.rs
[z-queue]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/doc/src/queue.rs
[z-chat-host]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/engine/src/chat2_host.rs
[z-managed]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/engine/src/managed.rs
[z-journal]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/engine/src/run_journal.rs
[z-session]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/apps/ios/Zeron/Session/CoreSessionSource.swift
[z-row]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/apps/ios/Zeron/Transcript/RowModel.swift
[z-usage-ui]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/ui/src/account_usage.rs
[z-plan-usage]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/proto/src/plan_usage.rs
[z-pins]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/engine/src/agent_accounts/pins.rs
[z-update]: https://github.com/zeronsh/zeron/blob/fbf5199a699c48fbb4917e6caa882ec8dfceaa8f/crates/update/src/transaction.rs
