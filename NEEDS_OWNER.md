## Private Verse characters on a paired phone (#10797)

The phone path is code-complete and tested with fakes; no real phone,
key, or broker was used. To see it on a device: install a build with this
change on a phone paired with the Mac, restart the Mac's host so it runs
this commit, and open the OpenAgents app. On the Mac, run
`verse-private phones` (the host also logs the command the first time the
phone asks), then `verse-private grant NAME KEY` with the
phone's world key. In the app's Verse tab, walk through the EVERGLADE arch
to the owner's house; the seated character should be at the reception. Then run
`verse-private revoke NAME KEY`, leave and re-enter Everglade,
and check the character is gone.

## World place calibration (#10788)

Agents walk down the world tree with Jev
(`questions/world-place.json`), which has no live measurement yet. Run the
calibration in `docs/decision-models/measurements/2026-10-07-world-place.md`
with a live Jev key (about 60 labeled activities over
`crates/world-tree/data/everglade.json`) and record the numbers there. It
spends a little Jev money.

## Brainstorm public-read release qualification (#10841, #10842, #10843, #10844)

On release day, authorize a short public read-only smoke against the configured
Brainstorm HTTPS origin using scratch state and no persisted owner chat. Record
the deployed discovery document, current house identity, one public profile
search, and one exact-key rank lookup. Confirm score units, coverage uncertainty,
TTL, and separate identity attribution. Local HTTP fixtures verify the Rust
client contract. On the selected packaged Coder host, confirm private settings
survive restart, explicit search/rank and Escape work, and a following turn with
authorized local or OpenRouter model access sees the retained observation.
With authorized OpenRouter access, confirm the packaged terminal and headless
approval desk display the exact lookup input and recipient before a native
function reads, rejection prevents the read, and an admitted search can supply
only its returned keys to rank. These checks use explicitly public inputs.
Deployed availability and packaged interaction still need qualification.
This check does not authorize profile publication,
authentication signing, or private query disclosure.

For the optional REV-37 pilot, separately approve the publisher, immutable
guidance release, exact public profile and capability/release links, and private
recording consent and retention. Follow `plugins/brainstorm/PILOT.md`; record
real search visibility, exact-key coverage, consented referral/install, accepted
tasks, actual settlement, and same-buyer repeat evidence separately. The local
checker validates source pins and operator claims; it does not publish, verify
indexing or paid conversion independently, or activate another capability.
REV-05/REV-26 commercial joins apply only when their authorized lanes are used.

## Insight support calibration and a live reflection (#10789)

Alice's reflection checks each insight with Jev
(`questions/insight-support.json`) under provisional thresholds (supported
0.7, preference 0.5). Run the calibration in
`docs/decision-models/measurements/2026-10-07-insight-support.md` with a
live Jev key and record the numbers there. Then turn the job on for one
night (`openagents agent jobs alice add reflect`, then `on reflect`) and
read what it stored, proposed, and dropped with `openagents agent log
alice`; accept or reject any proposed preference at F2. A reflection spends
about $0.05 at Sol list prices.

## Memory importance calibration and a live interview (#10787)

Alice's briefing now scores importance with Jev
(`questions/memory-importance.json`) under a provisional level mapping.
Run the calibration in
`docs/decision-models/measurements/2026-10-06-memory-importance.md` on the
phase A fixture with a live Jev key, and record the numbers there. Then run
`coder interview --arm all --answerer live --store SCRATCH` to compare the
scored and word-overlap arms with a real answerer; the scripted answerer
can't tell them apart. Both spend a little Jev and model money.

## Disk cleanup 0.2.0 on the Mac (#10759)

The running Disk cleanup rule is the saved
`~/.openagents/background/rules/disk-cleanup.json` from plugin 0.1.0, so it
lacks Claude Code worktrees, kache, the 200 GB/15% start level, and the
one-minute check. Install `plugins/disk-cleanup` again, remove that file,
preview with `openagents background run disk-cleanup --dry-run`, then run
`openagents background resume disk-cleanup`. Also start a host build from
main: no background runner has been live on this Mac since 2026-10-06 10:43.

## Shared compute account views (#10719)

On a scratch Mac, use the same private compute configuration with
`openagents-terminal --compute-workbench CONFIG` and
`verse --compute-workbench CONFIG`. Confirm the shared account, offer, run,
and receipt identities and F2 refresh after revocation. Rust projections and
Products adapter fixtures pass; physical rendering and real-money qualification
remain unverified. This pane offers no spending or shell-approval control.

## Browser host workbench (#10686)

On a scratch host, open the same session in native and browser clients. Check
reload, typist changes, full-screen snapshots, IME, clipboard permission denial,
and exact proposal confirmation on WebGPU and WebGL2. Export
`host_terminal_receipt()` for physical timing. Rust fixtures and wasm compilation
pass; actual browser rendering and network performance remain unverified.

## Browser admitted terminal transport (#10685)

Verify an enrolled browser against a scratch resident host through direct and
relay routes, including route loss, grant revocation, and slow-relay replay.
The Rust transport fixtures and wasm build verify protocol behavior; physical
browser/network qualification remains unverified.

# Owner checks

## Delivery, cleanup, and support O1/O8 (REV-07, #10814)

Privately complete the [handoff kit](docs/sales/delivery-kit.json) with the real
customer's exact accepted result/runbook, dependencies, limits, retained
artifacts, and separately accepted support human/boundary. Verify only the
agreed credential/device/resource/data cleanup; retain pending or unknown
owner checks and their next actions. Approve exact reuse rights and privacy
review before transferring generic material, and authorize publication
separately. The synthetic review proves the manual package, not real removal,
customer acceptance, support responsibility, or permission to reuse their work.

## Public pilot publication and intake O1 (REV-05, #10812)

Review the [frozen offer](docs/sales/README.md#first-workflow-offer-v1) and
public copy before publishing or activating real intake. The default `/pilot`
page shows proposed terms and unavailable intake. Code completion uses isolated
Rust and browser fixtures and needs no production host deployment; publishing
the reviewed page or enabling real contact intake is this separate owner step.

Privately name the responsible pipeline owner and public support email; accept
standing review responsibility, request-only email consent, its version,
one through 30 days of intake retention, a review duration within seven days,
and a lifetime lead cap from 1 through 32. Provision a distinct create-only
credential with [the intake procedure](crates/openagents-web/README.md#permissioned-pilot-intake).
Select an exact HTTPS origin beside the canonical durable private pipeline,
and ensure deployment logs omit bodies, cookies, and request queries. A site
deployment without that root keeps intake unavailable; no remote pipeline
transport or replica-local contact store is provided. Human review must verify
self-asserted contact permission before real follow-up; O6 separately governs
outbound automation.

The selected pilot installer builds `coder` and `openagents` from a recorded
clean commit on macOS arm64 through `scripts/install-coder.sh`. General native
release downloads do not qualify this path. Qualify that exact installation and
accept the private buyer agreement before work. Fixtures establish neither
commercial activation nor customer acceptance.

## Private pipeline O1/O6 (REV-04, #10811)

Before entering real leads, initialize the [private pipeline](docs/sales/README.md#private-sales-pipeline)
on the selected host and privately verify each named human, credential,
reader grant, consent source/date/expiry, jurisdiction, workflow, next action,
and `human:ID` recipient boundary. Agree to content retention and the minimum
suppression retained after deletion. Qualify a collaborator handoff with
that human's acceptance; do not treat a proposed handoff as accountable
ownership. Scratch fixtures establish persistence and authorization, not
customer permission. The CLI does not activate messaging; O6 still separately
authorizes real outbound. Keep customer content and credentials out of git.

## Assisted pilot agreement O1 (REV-06, #10813)

Before using the [pilot kit](docs/sales/README.md#assisted-pilot-kit), approve
its exact offer version, client/install qualification, named buyer and delivery
humans, input/disclosure rights, independent checks, price and provider budget,
external payment route, review date, and retention privately. Freeze the
agreement and retain separate buyer/owner acceptance of its digest. Confirm
actual results and acceptance before invoicing; qualify each extension with a
new accepted agreement. Record shared-content deletion when due. Synthetic
private-record walkthroughs establish field mapping, not customer consent,
real delivery, payment, install qualification, or automatic cap enforcement.

## Service invoice evidence O1 (REV-18, #10825)

Before claiming a real service collection, privately qualify the exact accepted
pilot agreement, customer result/runbook acknowledgment, support acknowledgment,
external invoice and payment route, and independently checked comparison.
Verify the actual collection, refunds, disputes, and any separately priced
fulfillment bill/payment with the external source, then record their references
through the owner credential. Keep bank/card details and credentials out of
records and git. Agree to invoice/consent retention and cleanup of the separate
source directory and private exports; pipeline deletion does not remove those
owner-controlled copies. Synthetic recording, replay, and report tests establish
the code path, not a real payment or remote attestation. No product funding or
entitlement is created by these service records.

## Operating cost and revenue reports (#10832)

Retain genuine customer/account attribution, contract terms, payment and
invoice evidence, independently accepted delivery, actual bills, refunds,
support cases, and complete source inventories privately. Check each declared
bill allocation, payer, excluded baseline cost, and no-cost assumption before
using `gym sales-finance`. Qualify real selected-lane inputs; synthetic ledger
and task evidence proves the projection, not actual revenue or delivery. The
owner reviews the exact private report and customer rights before exporting
or publishing an aggregate or margin claim. Future commission enrichment
remains unavailable until its authoritative obligation owner is integrated.

## Claims and price publication O1/O6 (REV-08, #10815)

Review the factual wording, full evidence inventory, scope, disclosure rights,
limits, payer, and current authoritative price source before using the claims
register with real buyers. Ordinary review does not activate proposed prices
or qualify a launch. Record a separate exact-source commercial or funded
qualification and its expiry before an available price/launch claim; the
synthetic fixtures establish no owner approval, customer agreement, payment,
or product availability. O6 still governs real outreach. Founder CLI use needs
no host deployment; Paul/outbox integration remains separate REV-55/REV-62 work.

## Pilot evidence O1/O8 (REV-03, #10810)

Before using [pilot comparison evidence](docs/sales/evidence.md) in a sales
claim, privately freeze a complete attempt inventory and obtain baseline/data
permission, independent check records, customer acceptance, and actual billed
evidence where available. Review the exact report and disclosure rights before
publishing its aggregate projection. Synthetic tests establish the adapter,
not a real customer result, measured savings, or a deployed routing improvement.

## Selected install and first task O8 (REV-02, #10809)

Use the [pinned macOS arm64 source-install path](docs/sales/README.md#selected-installation-and-first-task-rev-02)
with a genuine external buyer's supported Codex login, authorized public
repository commit, approved recipients/payers, and frozen independent checks.
Record actual setup time, installed binary digests, private runbook/support
references, accepted candidate, failures/repairs, and full costs. Disable hosted
cloud and default hosted decisions; disable or separately admit any existing
decision-provider configuration before buyer work. The isolated offline fixture
proves code behavior, not a real provider, buyer acceptance, measured savings,
or another release/platform/store qualification. Publication and paid-service
activation still require O1 and the private agreement.

## First workflow offer O1 (REV-01, #10808)

Before selling [Coder pilot v1](docs/sales/README.md#first-workflow-offer-v1),
confirm the proposed USD 250 service fee on accepted delivery, seven-day
invoice terms, zero promotional credits, one 30-minute free discovery call,
one change plus one repair, seven-day review, and three operator hours.
Changing these defaults creates a new offer version. No buyer agreement or
commercial activation is recorded by this documentation change.

Privately name the buyer/workflow owner, exact public repository and commit,
behavior/checks, installed Coder revision, approved provider recipients and
customer budget, delivery person, monitored support contact/business hours,
and review date. Approve the data policy, 30-day deletion of shared content,
invoice/consent retention period, provider terms, cancellation/defect terms,
and external invoice payment route. Keep secrets and customer records out of
the repository. Qualify the selected install before work; retain private
buyer acceptance and confirmed invoice payment before claiming service revenue.
This service does not activate retail compute, paid plugins, or product credit.

## Private earnings and payouts (#10838)

After deploying the earnings-enabled gateway beside the existing receiver
ledger, verify each `earnings.grants` account/payee/workspace binding. Declare
only supported payout rails with qualification evidence in `earnings.rails`.
Qualify a destination and one small funded payout through the existing
worker; compare the private statement's wallet reference, exact rail amount,
routing fee, and rounding with the real wallet. Check a destination change
while an attempt is reserved: it must keep the original destination and
resolve the same reference. Isolated fake-rail and authenticated dashboard
tests do not establish real-money qualification. Keep commissions and
reversals unavailable until their authoritative obligation owners are wired.

## Product funding policy O1/O5 (REV-21, #10827)

Before enabling converted or promotional product funding, accept the exact
`openagents.money.funding-policy.v1` document: monetary units, configured
rate source/version/validity, rounding and uncredited dust, verified fee payer
and cap, required payment finality, external refund/dispute terms, operator
loss liability for reversed spent credit, and promotion origin/caps/expiry/use
and reversal rules. Supply the actual payment adapter's restricted credentials
and retain funded qualification and reconciliation privately. Synthetic policy
tests establish accounting behavior, not launch prices, FX, processor fees,
real finality, or wallet liquidity. Existing fixed-unit Lightning products
keep their own contracts; Coder pilot v1 remains a separate service invoice
with zero product credits.

## Sales outreach launch

The [sales-floor decisions](docs/sales/agent-sales-floor.md#initial-operating-decisions)
start with permissioned US business email and individual approvals. Before
live outreach, configure the dedicated domain and monitored mailbox; verify
SPF, DKIM, DMARC alignment, TLS, commercial identification, and
unsubscribe/suppression handling; confirm
the recipient scope and footer with counsel; accept the certification sample;
and grant the exact campaign, model budget, and five-message daily pilot cap.
The host sending path still needs implementation and scratch qualification.
These documentation decisions don't activate outreach. Keep qualified
handoffs with the owner until a collaborator accepts a private agreement.

## Reviewed knowledge admission (#10667)

Before live use, retain a separately authorized independent evaluator cohort and explicit owner review under the fixed-pair profile; verify the exact candidate and actual runtime/configuration receipts, full charges, instruction/disclosure grant, expiry, and retirement. Synthetic signed fixtures validate the boundary; they do not establish real-world transfer or XP. On a scratch Mac session, inspect the pane with `--knowledge-review EVIDENCE --knowledge-operator PUBKEY --knowledge-evaluator PUBKEY`; physical rendering remains unverified.

## Knowledge candidate pane (#10666)

On a scratch Mac session, open a retained candidate with `openagents-terminal --knowledge-workbench SESSION` and confirm that its source count and candidate status remain visible; use `kb workbench inspect SESSION` for exact candidate bytes and trial costs. The fake-proposer harvest and Rust pane tests pass; physical rendering and a paid proposer run remain unverified.

## Phone glyph grid (#10684)

Compile the thin iOS and Android hosts with their native SDKs, then use a scratch host to check CJK and combining text, full-screen output, IME commits, SELECT/drag/COPY, horizontal pan, proposal controls, background recovery, and surface recreation on supported Metal/Vulkan devices; retain output, frame-time, and memory measurements. Unsupported GPU paths retain the native terminal screen; this work does not add an on-device shell. The grid remains bounded at 80 × 240 cells and 500 scrollback rows; each frame prepares at most 4,096 glyph characters in 1,024 reserved atlas rows. Native builds, measured frame/memory budgets, and physical recovery remain unverified.

## Plain TTY physical sessions (#10687)

Run `openagents terminal shell` in a scratch directory on the Mac and through your usual SSH/tmux setup; confirm native editing, full-screen controls, and the existing thread view remain usable. Scratch bash, zsh, and fish fixtures verify exact edited proposals and one result without a real engine; physical SSH/tmux and a signed-in provider remain unverified.

## Durable native and Verse terminal mounts (#10657)

On a scratch Mac host, check physical full-screen keys, selection, clipboard, and resize in both shared mounts. Close each mount and attach the other to its exact retained generation and terminal; confirm the process continues. The automated scratch acceptance covers projection, reattachment, route changes, refused offline input, and restart loss. Device rendering remains unverified.

## Retained capability workflow pane (#10665)

On a scratch Mac build from current main, open a retained flow with `openagents-terminal --root SCRATCH --capability-flow FLOW` and `verse --capability-flow FLOW`. Check that the shared product pane shows its original task, exact release, comparison costs, and failed/unknown actions after reopening. Physical display/input and publication through an owner-selected real signing key remain unverified; offline acceptance uses a temporary signer and registry.

## Remote shell proposal controls (#10745)

On scratch state in a Mac build from current main, create a desktop shell proposal and open the same terminal from the phone. Confirm that the proposal shows its exact command and revision, confirmation runs once, and its command block appears. Check that a watch screen has no enabled approval control. The Rust scratch-terminal and phone-projection tests cover these behaviors; physical controls remain unverified.

## Shared studio native and real-engine qualification (#10650)

The [scripted receipt](docs/verse/verification/2026-10-06-studio-workbench/README.md)
passes through the shared sheet and scratch host. On the Mac, confirm physical
T/workshop entry names the same resources, confirmation sends once, and exact
review/merge remains visible after reopening. Retain a separate bounded
real-engine run with source/app/host/engine IDs, request and review revisions,
trace and artifact hashes, actual checks, spend, failures, and cleanup.
Follow the [task admission](docs/coder/guides/tasks.md) and
[host auto-start](docs/coder/runtime/host-autostart.md) contracts. Use an
explicit temporary HOME, root, task store, and repository with separately
granted credentials; never use the owner's normal host or chat lists. Set
wall, output, concurrency, and spend limits before starting, and archive every
created task. Historical live audits do not qualify this new sheet. Open a
new issue if qualification finds a defect.

## Smart terminal Mac verification (#10642–#10644)

The sheet refactor you asked for is implemented: Retina text, one smart input
line, and the [design principles](docs/terminal/design-principles.md). Open the
local build named in the coordinator's message and approve it or list what to
change. Publication waits for that approval. After it, run the release from the
[receipt](docs/verse/verification/2026-10-05-smart-terminal/README.md) (the
credentials are on this Mac), install from the public URL with Verse absent, and
run the stress workload and repeated startup samples in a visible window when
the Mac is free. Pressing ENTER on a live proposal with a physical key, and a
recorded video if you want one, remain yours.

## Shared Everglade on two devices (#10553)

On the Mac that runs the Coder host, grant two devices `world` (and one of
them `observe`), start `openagents chamber host host.json` with
`"transport": {"type": "reach"}` and `"profile": "everglade"`, and join from
two computers with `verse --join FILE` (a build with
`--features remote-chamber`). Confirm both see the studio's seats at the same
places and each other's avatars, that `W`, `S`, `A`, `D`, `Q`, and `E` walk
and turn the avatar in the expected directions, and that the `world`-only
device opens no studio panel. The binary-level two-client check passes over
REACH with a scripted studio; the facing and keyboard mapping need a person
at a window.

## Desktop composer (#10004)

In a Mac build from current `main`, select the Japanese input source using the
system input menu. Start a new chat, compose a Japanese word, change a candidate,
and commit it. Confirm that Enter confirms the candidate without sending the
message; a later Enter sends it. Confirm that clicking away cancels marked text
without replacing the previously committed draft.

Scripted Japanese preedit, commit, Enter suppression, and focus-loss checks pass.
This remaining check exercises macOS's actual input method and input menu.

## Linux transcript performance (#10005)

On Linux with a native Wayland or X11 session, run the matrix in
`docs/desktop/verification/2026-09-30-transcript/verification.md` and retain the
result directory. Confirm transcript scrolling, tool expansion, and code copy.
The eight Mac cases pass; Linux hardware is unavailable in this worktree.
Open a follow-up issue if the Linux run finds a defect or exceeds 8.3 ms.

## Desktop image input (#10011)

On Linux in a native Wayland or X11 session, attach a generated PNG or JPEG
through **Attach image**, clipboard paste, and file drop. Confirm that each
preview appears, **Remove** preserves the caption, and **Send** preserves the
draft while explaining the current hosted text-only limit. A missing portal or
clipboard facility must report a reason. Repeat image clipboard paste on macOS.
The real macOS picker and scripted decoder, clipboard pixel, drop, removal,
and refusal checks pass. Open a follow-up issue if a native adapter check fails.

## Installed local Coder broker, handoff, and task chat (#10014–#10016)

After installing the desktop bundle and its Coder built from current `main`,
exercise the **Run Coder** flow added by #10015 against this computer. Confirm
that the normal OS key source remains in the resident host, saved device
pairings remain available after restart, and the task can be read and stopped.
With the configured engine signed in, verify live tool steps, Stop, the
phase-appropriate steering choice, queued follow-ups, and answers to a question
and an approval in the same task on desktop and phone. The scratch host checks
cover command admission, durable queue editing, exact retries, steering, Stop,
and archive. Shared state tests cover question and approval answers, stale
responses, split ATIF records, and draft acknowledgment. Native painter checks
cover all three task modes at normal and minimum window sizes.
Archive every verification task afterward. The scratch same-user socket,
portable client, durable inbox, restart, history, admission, and cancellation
checks pass without accessing the owner’s computer state.

## Installed saved sessions (#10017)

After installing a desktop and resident Coder built from current `main`, open
**Saved sessions** and inspect a known Codex session and Claude Code session.
Confirm their titles and times against the original tools. Choose a configured
project and continue one with the engine signed in and the existing auto-start
policy enabled. Confirm that its recent context reaches Coder, tool steps
appear, and the original saved session remains unchanged. Stop and archive
every verification task afterward. Scratch tests cover both source formats,
the native reader, real broker admission, exact prompt bytes, acknowledgment
retries, restart identity, cancellation, and archive without owner state.

## Desktop playable Grid (#10038)

Install a desktop bundle built from current `main` on macOS and on Linux
Wayland/X11. Choose **The Grid → Play**. Verify right-drag look, left-drag
orbit, simultaneous WASD, wheel entry and exit from first person, Escape,
Tab, Alt-Tab/Cmd-Tab, minimization, display-scale changes, and returning to
Watch or chat. A failed cursor grab must leave the pointer usable and explain
the keyboard fallback. Confirm the original chat draft survives.

Verify the separate world identity and Gym connection in the macOS login
Keychain or Linux Secret Service. Relaunch Play and confirm the public key
stays the same; deny a key read and confirm Play stays offline without
replacing it. Pairing and the resident host must retain their existing keys.
With a grant created for that world public key, inspect Gym runs, review a
recipe's budget, and explicitly confirm a permitted launch. Walking and
public boards must never launch it. Check desktop/phone movement together
against the chosen relay without creating test chats or tasks.

The isolated relay, shared controller, native input and layout, GPU captures,
and Linux binary checks are recorded in
`docs/desktop/verification/2026-09-30-playable-grid/verification.md`.
Native OS cursor grabs, protected-store prompts, and the signed Mac bundle
need device checks. Windows Verse remains tracked by #10027. Open a new issue
if any installed-device check finds a defect; these checks do not keep #10038 open.

## Cloud artifact signing (#10227)

Configure a private GCS artifact bucket with lifecycle retention and an identity
with object-create and URL-signing permissions. Run a non-sensitive Boat or GCE
command using the artifact publisher described in `scripts/cloud/README.md`.
Verify that all four signed links open and expire after 24 hours. Mocked tests
cover upload and comment behavior; a live signing check has not run.

## GCE retirement decision (#10223)

Review `docs/cloud/2026-10-02-orphan-retirement-review.md` and confirm a current
owner and retain-or-stop decision for each of the nine running candidates.
No machines were stopped or deleted. The issue explicitly requires this decision;
its operational work is not complete.

## Raw versus OpenAgents study (#10162)

The standing study still needs implementation and new measured trials for raw
Claude Code, raw Codex, the shipped default path, and a matched arm, including
repository tasks and Terminal-Bench tasks. Existing matched Opus evidence is
not evidence for the current shipped default path. This delegated run cannot
launch another coding engine as a subprocess, so it did not execute raw-engine
trials. Do not report a savings percentage or close the issue based on the
historical pilot alone.

## Boat paid lifecycle (#10218)

Run the ignored `paid_lifecycle` test in `crates/boat/tests/live.rs` with a
scoped, expiring Boat key and its ID. Follow `crates/boat/README.md` for the
command and required variables. The offline suite and cleanup failure-path
check pass; the paid check has not run. Confirm that deletion completes and
final usage is non-running and below $0.01. The check detects cost overruns;
it does not impose a provider-side spending limit. Do not use an unrestricted
key or publish the credential in logs.

## Everglade studio on phones (#10476, #10485, #10486)

Build the Coder iOS app from current `main` and run it in a simulator, then on
a device. Walk through the Everglade arch on the plaza, wait for the pack to
download, and confirm the glade renders textured (grass, trees with cut-out
leaves, the workshop) on Metal. At the notice board, a desk, the podium, and
the merge station, tap **Interact** and confirm that the console, the seat
panel, decisions, and diff review open, and that the close control returns to
the world. Run `testEvergladePortalEntersGladeAndReturns`; it now needs the
pack download and still names its screenshot "Everglade greybox glade".

Repeat on the Android emulator and a device, on Vulkan or GLES: the same four
stations, the back button closing the panel, and the TalkBack Interact action.
Until the live host source lands (#10492), every panel says the studio has not
loaded. The Rust panel and scene tests pass. Open a follow-up issue for any
rendering or mounting defect.

## Agent Studio mixed-engine run (#10477)

Run the studio once with real engines, on a scratch host, from current
`main`. The code paths are covered by unit tests and by the scratch-host
acceptance test (`cargo test -p verse --test studio_host`), but no real engine
has driven them; the repository's velocity rules leave live engine runs to
the owner. The [Agent Studio audit](docs/verse/agent-studio-audit.md) lists
what a scratch host already showed and the defects to expect. The simulated
team is a read-only replay inside Verse and can't stand in for this run
(#10572).

1. Build `openagents`, `coder`, and `verse` from current `main` into the
   agent slot's target directory and call them by path. The `openagents` on
   `PATH` is an older program with no `studio` command.
2. Start a scratch host with a temporary `HOME` and `--state`, `--root`, and
   `--tasks` under a temporary directory, admitting a scratch Git repository
   as a workspace (`coder host init --workspace scratch=PATH`). The
   repository needs a clean checkout and no remote: a merge fast-forwards the
   checked-out branch locally and pushes nothing.
3. Add seats with `openagents studio seat set`: a Codex lead
   (`--role lead --route codex:MODEL`) and Claude Code (`claude:MODEL`),
   OpenCode (`opencode:PROVIDER/MODEL`), and Codex (`codex:MODEL`) workers.
   Turn on auto-start for the workspace with `full` access and exactly those
   routes. Microcoder isn't a route: a `codex` seat runs Microcoder's loop by
   default (`coder.codex` is `loop`), and a `claude` seat runs a Claude Code
   session (`coder.claude` is `session`). These settings apply to the whole
   host until #10568 lets each seat choose its engine.
4. Open Verse with `--everglade --studio-socket PATH`, pointing at the
   scratch host's control socket, and submit a two-task goal from the notice
   board's console.
5. Confirm that at least two tasks run in parallel, seats walk to the
   stations their work implies, one question and one approval are answered at
   the podium, one change is requested at the merge station with a line
   comment and fixed, and one task is merged from the review. Only
   Microcoder's loop raises questions and approvals; Claude Code and Codex
   sessions run with permission bypass flags. Answering, reviewing, and
   merging need Verse until #10566 adds them to `openagents studio`. Merge
   only finished tasks, because the host doesn't refuse an unfinished one
   yet (#10567).
6. Quit Verse mid-run and reopen it, then restart the host mid-run, and
   confirm that no state is lost.
7. Archive every task the run created (`coder task archive TASK_ID --reason
   "studio trial"` against the scratch task store) before deleting the
   scratch host.

Open a follow-up issue for any defect, with the seat routes and the step it
failed at.

## Everglade from the Grid, as the ranger (#10530, #10534)

On TestFlight build 46 (iPhone) and an Android device: on the Grid, walk
through the arch lettered EVERGLADE. The first visit shows download progress
with Cancel; cancel once, then Retry with the network off and on. In
Everglade, confirm you play the hooded ranger, it idles, walks, runs, and
jumps with the stick, and no spade follows you. Return by the arch lettered
THE GRID and by **The Grid**, and check you land beside the EVERGLADE arch
with other players visible again. A second visit opens from the cache.

## Phone studio panels on Android and a real phone (#10579)

The iOS host builds and, in the simulator, enters Everglade and reports
that no computer is online for the studio. To finish checking:

1. Build the Android app (`bins/openagents-android/build.sh`): this Mac has
   no Android SDK, so the Kotlin wiring (`VerseStudio.kt`, the JNI entries
   in `crates/openagents-mobile/src/android.rs`) was never compiled.
2. On a phone paired with a computer that runs the studio, walk into
   Everglade, stand at the podium, tap Interact, and answer a decision;
   then merge a finished task at the merge station.

## Display names over heads on real devices (#10583)

#10598 adds a **Display name** field to Account on iOS and Android and
sends the name in NIP-MV states. This box has no Xcode and no Gradle
run of the Kotlin host, so the Swift and Kotlin glue was not compiled.
To finish checking:

1. Build both hosts (`scripts/release/testflight.sh start --validate-only`,
   `bins/openagents-android/build.sh`).
2. Set a name under **Account > Display name** on one phone, open the
   Grid on a second device or run `openagents --json verse who`, and read
   the name over the first phone's avatar.
3. Run `openagents verse walkers 20` and check that `walker-0` to
   `walker-19` are readable over heads at a steady frame rate.

## Two players meet in Everglade (#10584)

#10604 re-keys presence to a zone's shared NIP-MV world
(`verse-everglade`, `verse-lagrange-1`) through the arch. Loopback tests
cover the mobile and desktop world switch; no two-device run happened
here. To finish checking:

1. On two phones (or a phone and the desktop), walk through the Everglade
   arch from the Grid and confirm both see each other's avatar, name tag,
   and collide in the glade; walk back and confirm both reappear on the
   Grid.
2. Standing in Everglade, run `openagents verse walkers 5 --world everglade`
   and `openagents --json verse who --world everglade`; the walkers must be
   visible in the glade and absent from the Grid.

## Two desktops in a public chamber through RITUAL (#10585)

The guest chamber host and the Grid's RITUAL arch were checked on one
machine: four guest keys joined a local host, a fifth was refused, and a
reconnect kept its seat. The two-desktop play test needs a second
contributor machine and a desktop display, which this box has not. To finish
checking:

1. Run a host with a `guests` policy on a machine two desktops can reach
   (`openagents chamber host CONFIG.json`, or `chamber service install`),
   and copy its DER certificate to both desktops.
2. Write `~/.verse/ritual.json` on both (address, instance, `trust_der`,
   `pack`, `scene`, `dir`) and start `verse` built with
   `--features remote-chamber,imported-desktop`.
3. Walk both players through the `RITUAL` arch, fight cultists in the same
   chamber window, read `openagents --json chamber status` for
   `population.players: 2`, then close the windows and read that both
   players stand before the arch again with Grid presence restored.

## The phone Grid on the engine renderer, on an iPhone (#10616)

The Grid draws through `verse-engine` on every client: the engine opens on
an iOS Metal layer, an Android window, and a browser canvas, and the Android
package and the browser module were built here. This box has no macOS, so
the iOS build and the Metal surface were not run. To finish checking:

1. Build the OpenAgents iOS app from `main` and open the Grid; the plaza,
   arches, line figures, and name tags must draw as on the desktop, in the
   neutral phone palette, with the stick and HUD unchanged.
2. Rotate the phone and background and foreground the app; the frame must
   follow the new size and resume after the layer is reattached.
3. Walk through the Everglade arch and back; the Everglade draws on the
   legacy renderer and the Grid returns on the engine.
4. With `openagents verse walkers 10` running, confirm ten walkers with
   their names move smoothly on the phone's Grid.

## The desktop app's Grid on the engine renderer (#10606)

Play and the Watch backdrop in the OpenAgents desktop app now draw the Grid
through the engine into the window's own texture. The offline GPU fixture
(`grid_fixtures.rs`) passed on an Apple M5 Max; a live window was not
opened. To finish checking, build the desktop app from `main`, open the
Verse page, and confirm that Watch shows the plaza from above and that Play
shows the line figure, the name tag, and the boards. Resize the window,
enter and leave full screen, and switch between Watch and Play; the world
must follow each change without a black frame that persists.

## The phone in the chamber (#10586)

The phone joins the chamber through the RITUAL arch, and the session,
suspend and resume, host loss, and respawn pass against an in-process
chamber host. The frame-time run on a phone did not happen: the build
machine's disk filled during the Android release build. To finish checking:

1. Host a chamber on a scratch directory with a temporary `HOME`:
   `openagents chamber pack DIR/assets`, `openagents chamber tls DIR`, and
   `openagents chamber host DIR/host.json` with
   `"guests": {"cap": 16, "ring": [0, 0, -22], "radius": 3}` and the scene
   `assets/verse/original/ritual.json`. Join five more guests with
   `openagents chamber move` under five scratch profiles, so the chamber
   holds 20 actors with the phone.
2. Write `ritual.json` (address, instance, `trust_der`, `pack`, `scene`,
   `dir`, and `server_name`) and copy it, the certificate, the pack, the
   scene, and the asset directory into the app's zone cache directory: on
   Android, `cache/VerseZones` of `com.openagents.app` (`adb push`, then
   `adb shell run-as com.openagents.app cp ...`); on iOS, `VerseZones` in
   the app's caches directory. An emulator reaches the Mac's host at
   `10.0.2.2`.
3. On an Android phone and an iPhone built from `main` (release Rust), walk
   through the `RITUAL` arch on the Grid, fight for two minutes, die and
   respawn, background and foreground the app, then tap **Leave**. Confirm
   the player returns before the arch with Grid presence restored.
4. Copy `chamber-frames.json` from beside `ritual.json` into
   `docs/verse/verification/2026-10-05-phone-chamber/` for each device.
5. Fight a desktop player in the same chamber (`verse` built with
   `--features remote-chamber,imported-desktop`, through its own RITUAL
   arch): each must see the other's character move, cast, and take damage.

## The world population cap on the public relay (#10588)

The relay now refuses a 21st key's NIP-MV frames in one world
(`NOSTR_RELAY_WORLD_POPULATION_CAP`, default 20) and bounds a world's
frames a second (`NOSTR_RELAY_WORLD_POSE_PER_SEC`, default 300). Unit
tests cover the cap and the budget; the public relay doesn't run it until
it's redeployed. To finish checking:

1. Deploy the relay from `main` (`docs/deployment/runbook-cloud-run.md`).
2. With the Grid empty, run `openagents verse walkers 21 --wait 30`; the
   `done` line must count refusals (`rate-limited: world is full`), and
   `openagents verse who --world verse-bare` must show 20 live.
3. Raise `NOSTR_RELAY_WORLD_POPULATION_CAP` before the 20-player soak
   (#10589), which also brings a phone, a desktop, and a browser.
4. Block a walker with `openagents verse block KEY`, relaunch the Verse
   app on the same computer, and check that the walker stays hidden.

## The 20-player Grid soak on real devices (#10589)

The simulated soak passed on 2026-10-06
(`bench/verse/2026-10-06/grid-soak/review.json`): 17 walkers and simulated
phone, desktop, and browser clients for 30 minutes on a scratch relay, with
frame-time p95 near 6 ms, no refusals, no frame older than 136 ms, and a
21st player refused by the cap. Real devices haven't run it, and phones and
the desktop app need a build from `main` after 6879f68cbf, which stops pose
frames from stalling for most of each minute. To finish checking:

1. Deploy the relay from `main` if it predates the population cap (#10588).
2. Run `openagents verse walkers 17 --world verse-bare --wait 1900` and,
   beside it, `openagents verse load --world verse-bare --players 20
   --max-age-ms 1000 --wait 1800 --json`.
3. Join with a phone build, `verse --frame-times` on the reference desktop,
   and the browser Grid, standing with the Gym in view for 30 minutes.
4. Check frame-time p95 under 16.7 ms on the desktop and 33.3 ms on the
   phone and browser, no `rate-limited:` refusals, and `load` exit 0. Retain
   the output under `bench/verse/<date>/grid-soak/`. Open a new issue for
   anything that fails.

## Block a player from the Grid on a phone (#10638)

Tapping a player's name tag on the Grid opens a card with **Block**,
**Mute**, and **Close**; the desktop Grid opens it with a click. A
loopback test drives the scene; no phone ran it. On an iPhone and an
Android phone built from `main`, start `openagents verse walkers 3`, tap
a walker's tag, and tap **Block**: the walker disappears and stays gone
after the app is relaunched.

## The Grid in the browser at `/grid` (#10587, #10626)

The browser Grid joins the other players over the browser's WebSocket and
draws on WebGL2 as well as WebGPU; headless Chrome on a scratch relay saw
20 walkers with names on both (`docs/verse/verification/2026-10-05-grid-browser/`).
openagents.com was not redeployed. To finish:

1. Deploy `openagents-web` from `main` as the `/druid` deploy did
   (`docs/deployment/openagents-web.md`). On the `new` tag, `/grid` must
   answer 200 with `connect-src 'self' wss://relay.openagents.com` in its
   policy.
2. Open `/grid?name=YOURNAME` in Chrome, in Chrome with WebGPU off
   (`chrome://flags`, or `/grid?gl`), and in Safari. Each must draw the
   Grid with `ONLINE · N HERE` at the top left and your name over your
   head; walk with `W`.
3. With the OpenAgents app on a phone in the Grid, the phone and the
   browser must each see the other move, with names.
4. Click another player's name tag and **Block**: they disappear, and stay
   gone after a reload.
5. Optional: on the Android emulator with `-gpu swiftshader_indirect`, the
   OpenAgents app's Grid must draw instead of the renderer's error card.

## Upgrade Verse hosts and clients together after V18 (#10637)

The V18 wire-29 milestone adds applied movement confirmations to owned response
controls. Subsequent game services and safety advance the current wire version;
see the [generated runtime contract](docs/verse/runtime-contract.json). Deploy
matching current builds of `verse-host` and each chamber client before using
this protocol on a real host. The V18 capacity checks use scratch TLS
hosts and offscreen rendering; they do not deploy or exercise owner devices.

## Review the first retail cloud contract (#10704)

[`docs/cloud/retail-contract.md`](docs/cloud/retail-contract.md) freezes the
first paid cloud class for implementation: Boat `large` sandboxes, one per
task (not the GCE pool); a public-GitHub-repository change returned as a
patch with customer-declared checks and no publication; Codex on the
customer's own OpenAI key, with hosted Vertex fallback off; at most 4
retail sandboxes at once and 60 minutes a task. Confirm these choices
before #10705 to #10711 build on them. A change is a new contract version
(`openagents.cloud.retail.v2`), not an edit of v1. Paid availability stays
off until the funded qualification passes.

Also confirm the prices in [`docs/cloud/retail-prices.md`](docs/cloud/retail-prices.md)
(#10706): one credit is one sat; 40 millisatoshis a second of compute (144
sats an hour) and 100 sats of coordination per started task; nothing
charged before the executor starts; and no payout of a purchased balance
in v1. A different rate or refund policy is a new book version.

## One terminal on two devices (#10653, #10655, #10675)

Hosts now answer terminal queries themselves, join devices by snapshot, and
let one device type at a time. After the Mac app and any headless hosts run
a build with these changes, open one terminal from two phones (or a phone
and a second phone build):

- Run a program that asks for the cursor position (`vim` or `htop` does) and
  confirm it draws normally on both with no stray `R` or `c` characters.
- Join the second device while the first prints continuously, and confirm
  its screen matches without a gap note.
- Type on the first device, confirm the second shows **Type here** and
  draws at the first device's size, then tap it and confirm the second can
  type and the first now shows **Type here**.

The PTY-level checks run on scratch hosts in `coder-vt`'s
`tests/authority.rs` and `tests/join.rs` and in `coder-pty`'s
`tests/typist.rs`.

## Verify native Verse audio on physical output after V23 (#10739)

After installing the matching native chamber build, listen to a crowded fight,
remove and reconnect the selected output device, change focus, and suspend and
resume the application. Confirm that critical cues remain audible, music resumes
at its retained position, and sound remains clean. With no output device, confirm
that captions and master/music controls (F9–F12) still work. The committed V23
checks use a headless callback and controlled PCM production; they do not open an
owner device or prove driver scheduling. V24 mounts browser and phone captions;
device output audio remains unimplemented.
Open a new issue for a defect found in this device run.

## Verify authoritative platform clients after V24 (#10742)

Use an isolated scratch chamber, original admitted content, and three enrolled
world keys. Follow [platform client configuration](docs/verse/platform-clients.md).
Join desktop, a physical phone, and a supported browser to the same instance.
Compare damage, death, and respawn; interrupt network and focus, suspend/resume,
and verify the same character returns without stuck movement or repeated casts.
Measure declared device frame, memory, and thermal budgets. Check touch targets,
text at device pixel ratios, keyboard focus, screen reader status, and gamepad
remapping. Browser and phone have captions but no mounted audio output adapter;
record that limitation rather than treating silent output as audio parity.
The V24 receipts cover Rust tests, Wasm linking, and isolated headless DOM/
software WebGL2 checks. The 1000-by-800 CSS viewport
passes authority movement and stopped input after adaptive graphics resolution;
full-resolution software rendering failed that budget before the fix. They do not cover these physical
checks. Retain per-device evidence and file a new issue for any defect.

## Everglade workbench opening (#10647)

In a current desktop build with the studio source connected, stand at a
studio desk and open `T`, then hide it and use `Shift+F`. Confirm both show
the same goal, seat, and task in the sheet status. Shift-click a seat to
select it; ordinary clicks and `F` still open studio panels. Opening must
not start a goal or task. Confirm that leaving Everglade clears the context.
The isolated adapter and fake-transport checks cover identity, denied
observation, restart, and repeated opening; this visible-window check remains.

## Studio workbench projection (#10648)

On the Mac, open the shared studio page with `F13` in the standalone window
and at an Everglade station. Check goal and task rows, seat engines and
spend, log tails, and memory against the existing studio panels. Confirm
one scratch seat command with Enter twice, inspect the host receipt, and
check that revoking observation clears the page. Archive any scratch tasks.

## Studio workbench approvals and reviews (#10649)

On a Mac, open the studio sheet from Everglade and the standalone window.
Verify the visible question, approval command, directory, risk, and standing
rule before confirming. Read a completed scratch task with `/review TASK`,
then verify that its three revisions and diff remain legible before deciding.
Focused adapter and host tests use scratch data; native rendering and a
physical owner-device run remain unverified here.
## Phone terminal sessions, commands, and watching (#10683)

On an iPhone and an Android phone enrolled with `terminal` on a host built
from current `main`, open **Terminal** on that host. Run a command, send the
app to the background, and return: the screen shows the same shell, not a new
one. Tap **Commands** and confirm the list (the shell needs the host's
integration hooks to record commands). On the desktop, save a session that
holds the terminal and a thread; on the phone, tap **Sessions**, open it, and
tap another live terminal to switch to it. Rotate the phone, paste, and use
the soft keyboard in each. Restart the host and confirm the screen says the
terminal was lost. Approving pending proposals from the phone is #10745.

- Later independent-worker qualification (#10725): Run
  `scripts/qualification/later-worker.sh NEW_OUTPUT_DIRECTORY` first. Its
  buyer/provider roles share one operator; independent operation is unverified.
  Have two separately controlled scratch operators confirm the pinned
  MKT/LAB no-spend order, protected checker, disclosure, source, and grants;
  retain duplicate, crash, relay replacement, delivery, check, and acceptance
  records with measured all-in costs. Before a paid rehearsal, separately
  authorize one fixed invoice, payer, worker destination, fee cap, and spend
  limit; retain central receive/share/payout and both wallet receipts, including
  unknown-outcome recovery. No funded worker scenario has run.

## Terminal sharing controls (#10681)

On the Mac standalone terminal and the Verse terminal, connect to an enrolled
resident host and press F17. Issue future-output watch and drive shares to two
devices, privately deliver the copied sealed authorization, and confirm the
viewer and typist markers. Pause and confirm both recipient panes blank;
resume and confirm a gap without paused output; revoke and confirm both detach.
The scratch host checks the sharing protocol and privacy cuts. Physical native
presentation, clipboard delivery, and device interaction remain unverified.

## Standalone Linux and Windows qualification (#10689, #10690)

On isolated Linux x86-64 and Windows x86-64 machines, build the matching
standalone app and helpers with the platform release script. Retain native
startup, fullscreen, Unicode/IME, clipboard, resize, process cleanup, and frame
workload observations with executable hashes, then publish and verify the
public installer readback. The tooling refuses qualification marked not-run.
Neither platform has a qualified public artifact from this work.
On Windows, include PowerShell filesystem/provider changes and the explicit
`# ...` request, exact pending proposal, and single acknowledged command result;
portable PowerShell fixtures and Windows cross-checks do not qualify that native path.


- Negotiated bids (#10726): After the independent no-spend operator rehearsal
  above, ask two eligible separately controlled providers for private quotes;
  retain quote timestamps and expiry, exact terms, current capacity, buyer
  selection and acceptance, nonwinner disposition, and measured incremental
  coordination cost. The synthetic `bid-qualification` example measures only
  local comparison latency; provider quote latency and all-in coordination
  cost remain unverified. Do not adopt funded bidding before that study.

- Commercial custody qualification (#10727): Production escrow is unavailable.
  Run the `custody-qualification` example for fake conservation evidence. Before
  funding a scenario, admit a specific legal custodian and enforceable rail,
  pin the resolver and release/refund authority, fee limits, milestones,
  deadlines, and insolvency liability. Separately authorize one bounded
  deposit and retain real deposit, release, refund, fees, unknown lookup, and
  dispute records. Never infer escrow or refund support from exact Lightning
  payments. All funded custody and dispute cases remain unverified.

- Training market qualification (#10728): Admit independent scratch operators,
  signed dataset/checkpoint rights, the exact corpus partitions, frozen recipe,
  seed, budget, worker class, artifact contract, and protected evaluator. Run a
  no-spend training fixture and retain exact checkpoint/checker/restart records
  plus all failed-training, checking, and search costs. Then separately fund
  and authorize compute, data-license, and accepted-improvement obligations;
  retain central payment receipts and an independently verified improvement.
  The current fixture uses synthetic checkpoint bytes and fake costs; real
  training, transferable improvement, and all funded cases are unverified.

- Useful contribution payments (#10729): Freeze one optimization-bounty class,
  source/evaluation group separation, license, attribution, beneficiary,
  protected evaluator, acceptance/funding authorities, reward, and expiry.
  Retain an independently controlled protected evaluation of the exact artifact
  away from its sources, then authorize a bounded funded central receipt and
  payout destination. Verify conservation, duplicate refusal, wallet lookup,
  and unknown payout recovery. The synthetic contribution fixture uses fake
  receipts; independent usefulness and every real contributor payment remain
  unverified. XP and token/activity counts do not authorize sats.

## Terminal agent handoff (#10682)

On a standalone and Verse terminal attached to a resident host from current
main, use F17 and `/agent AGENT_KEY THREAD_ID RUN_ID` to confirm one handoff.
Check the private agent badge and press an owner key to reclaim control.
The scoped host-local producer and scratch PTY/relay tests verify admission,
attribution, revocation, replay refusal, and private thread/run evidence.
Physical native keyboard and badge presentation remain unverified. A handoff
admits at most 256 distinct inputs and grants no screen observation; consent
separately to individual block attachments. No live model producer was run.

- #10658: On a paired Mac host, visually check the standalone and Verse task
  shell titles and directory display. Scratch relay fixtures cover admission
  and task isolation; a physical Mac session remains unverified.

## Metered Lightning session qualification (#10721)

[`docs/cloud/mpp-sessions.md`](docs/cloud/mpp-sessions.md) defines the
session profile `openagents.mpp.lightning-session.v1`; mock-rail tests cover
deposit, debits, disconnect, expiry, closure, remainder return, and unknown
refunds. No route offers a session. Before any route does, separately
authorize one funded session: a deposit of at most 1,000 sats from a scratch
payer, two debits under one admission, closure, and the remainder refunded
to the payer's own BOLT12 offer. Retain the deposit and refund payment
hashes and the ledger's session summary. A real deposit, a real refund, and
third-party MPP client compatibility are unverified.

## Funded retail cloud qualification (#10722, #10723, #10748)

The fake-payment acceptance run and the fake qualification of the
checked-in plan pass, and the live bindings (the resident receiver wallet's
socket client and `retail_cloud::boat::BoatAdapter`) qualify against a
simulated Lightning network and a loopback fake Boat API
([`docs/cloud/retail-qualification.md`](docs/cloud/retail-qualification.md),
receipts in `docs/cloud/evidence/2026-10-06-retail-fake-*.json` and
`2026-10-06-retail-simulated-qualification.json`). After "Review the first
retail cloud contract", follow the owner runbook there: run the resident
receiver wallet on mainnet under its own home, create a separate retail
Boat account with its own key (`OPENAGENTS_RETAIL_BOAT_API_KEY`, never
`BOAT_API_KEY`), give the test customer its own OpenAI key
(`OPENAGENTS_RETAIL_CUSTOMER_MODEL_KEY`), write a bindings file, review the
plan digest (`retail-qualify plan`), and run `retail-qualify qualify
--funded --confirm DIGEST --bindings PATH` once, paying its printed top-up
of at most 1,000 sats. Keep the receipt, the state directory's ledger and
journal, Boat usage, and artifacts; check the sandbox is deleted and
nothing stays held. Real readiness, latency, Boat billing, Lightning
top-ups, and the owner program (`owner-v1.sh`) on a real daily template
(its `git`, `setsid`, and `codex login --with-api-key`) are unverified. A
failed run opens a defect issue.

## Launch the retail cloud service (#10724)

The launch gate, monitoring, and operator runbook are on main
([`docs/cloud/retail-operations.md`](docs/cloud/retail-operations.md));
`retail-qualify advertise` reports `contract_unconfirmed` and advertises no
paid computer. To launch, in order: complete "Review the first retail cloud
contract" and "Funded retail cloud qualification" (including #10748's live
bindings); deploy the service from current main under a separate retail
Boat account and receiver wallet; pass `--contract-confirmed` and the
funded receipt to the gate; check `retail-qualify health` is clean; and
publish the contract and prices to customers. Isolated install and customer
onboarding on the native clients, real capacity behavior, and incident
handling on a live service are unverified.

## Alice in the owner's house

Deploy Everglade pack `4cfbbe2bfe74bba084436b5a5fd8dc09ec0c770fcdd613f9ef92986749950abb`
so Verse can download the house with Alice's workstation. Then:

1. Run Verse (`verse --owners-house`), or the OpenAgents desktop app.
1. Walk into your house, up to Alice's workstation, and press F.
1. Follow her: Enter continues, Enter starts the host if she says none is
   running, pick a workspace from her list (or type a path), and Enter
   confirms. Then send the request she suggests.

[Talk to Alice in your house](docs/verse/workshop-agent.md#talk-to-alice-in-your-house)
has each step. A scratch-host run verified each step with keystrokes into
Verse, but not your host, the desktop app's keychain-held owner key
attesting her, your Codex login's limits, or another person's device being
refused.
