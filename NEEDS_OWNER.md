## Show cloud environment issue runs on openagents.com and the phone (#11228)

`openagents chat work` on `oa-dev-env-1` reports each issue run to the
account's agent list once it is signed in. Sign in once with `coder login` on
the environment, or store an app sign-in token as the Secret Manager secret
`dev-openagents-app-token` (project `openagentsgemini`), which
`scripts/cloud/dev-env-session.sh` exports as `OPENAGENTS_APP_TOKEN`. Then
start an issue run there and check that it shows on `/settings/agents` and on
the phone's Agents screen, and that Stop from each ends it.

## Qualify working-computer checkpoints on real Boat (#11007)

`crates/coder-working-computer` keeps a chat's computer between turns: it
checkpoints after each completed turn (on Boat, by stopping the sandbox and
recording its snapshot), restores on the next prompt, re-applies credentials,
and restarts declared services. Its tests use a fake provider only. Before
admitting it, run one computer on an isolated, separately funded Boat key:
two turns, an idle stop, and a deletion. Confirm that the snapshot recorded
after each turn is the one restored, that the sandbox does not report
`holdsCreatorLogins` (the provider refuses such a boot), that usage stops after
each stop, and that the deletion completes. No web or Coder surface uses this
owner yet; wiring it to chat prompts is a later, separately admitted step.

## Activate native Cloud runtimes on web staging (#10992)

The web source/runtime controls use the existing native operator review and job
owner. Staging has no configured native account service or enrolled resident
binding, so it exposes public source metadata and reports execution unavailable.
To activate actual repository work, provision the protected account configuration,
current account/workspace binding, reviewed host custody, operator source/profile
pins, and explicitly selected provider credentials described in
[`docs/cloud/README.md`](docs/cloud/README.md). Qualify the real provider lifecycle
on an isolated computer before admitting it. The integrated fixture uses a
synthetic backend and does not establish funded or persistent computer operation.
Native reviewed requests also require durable control-journal custody. The
current journal uses native files; the public chat's GCS adapter does not provide
shared native journal storage. Qualify a single persistent custodian or a shared
admitted journal before enabling native controls across Cloud Run replicas.
Production promotion requires a later request; this task publishes staging only.

## Verify native terminal display idle (#10909)

Run the updated native terminal with a screen lease, leave its default sheet
idle, and confirm WindowServer stops presenting unchanged frames and idle
CPU stays near zero. The automated one-hour soak uses the same native raster
pipeline and isolated PTY offscreen and passed on 2026-10-08:
zero idle submissions, 0.161% idle CPU, and 169.66 MiB peak RSS sampled every
30 seconds. The report is `docs/terminal/verification/2026-10-08-native-retention/one-hour.json`.
It cannot measure the physical display or reproduce the owner's lost
pre-reboot 20 GB process. If the display check finds a defect, open a new
issue with the process RSS and CPU sample.

## Ship a TestFlight build and open the Verse tab on the iPhone (#10928)

The build 53 crash — a main-thread stack overflow the moment the Verse tab
mounted — is fixed on main: the world now builds on a dedicated
`verse-create` thread with a large stack, and the scene returns to the main
thread boxed, so the creation path's frames stay under 32 KB each. The only
remaining acceptance step is on hardware: ship the next TestFlight build
(`scripts/release/testflight.sh start`, then `wait`), install it on the
iPhone 17 Pro Max that logged `OpenAgents-2026-10-07-222458.ips`, and open
the Verse tab — the Grid should render instead of crashing.

## The RTX 4080 pylon is running (#10921)

`coderos-4080` serves Qwen3.5 0.8B through Psionic on CUDA as the pylon
`npub1jk782ggcuyvls5khxaq7tay58r84a8wwfucc2kgszn9m0sc3aveq3acdyu`, on
`relay.openagents.com`, admitting only this Mac's buyer key
(`npub109epejnmy639a6evhwqzmmkz9mtcakm0ayh8vjtwsrmh9wk5hl0sug9pvd`). Two
transient user units run it, `pylon-psionic` and `pylon-provider`, from a
scratch checkout in `~/work/pylon-p1` on the box; no unit file is
installed, so a reboot also stops it. Try it with `openagents pylon ask
"..."`. To stop it:

```sh
ssh coderos-4080 'PYLON_DIR=~/work/pylon-p1/run PYLON_TARGET=~/work/pylon-p1/target ~/work/pylon-p1/openagents/scripts/pylon-psionic.sh stop'
```

To run your own from a checkout on the box, use `scripts/pylon-psionic.sh
setup` and `start --allow <your npub>` (`docs/compute/pylon.md`); delete
`~/work/pylon-p1` afterward (about 20 GB with build output). The owner
decided on 2026-10-07 that pylon traffic, test jobs included, uses the
production relay.

## Cloud BYOK credentials (#10917)

To delegate with an OpenAI API key, fund or replace the configured credential;
the live check reports exhausted quota. OpenRouter BYOK needs a valid
replacement credential; the configured keys fail authentication. Codex login
forwarding passes Boat's live integrated API check. Microcoder through the
OpenAgents cloud fallback passes the headless Boat and GCE lifecycles.

## Everglade's medieval kit at 60 frames per second (#10901)

After the openagents.com deploy that serves the kit pack, open
`https://openagents.com/everglade?frames` on the reference laptop and
confirm about 60 frames per second on Stoop Lane and Main Street. On a
phone, open Everglade and confirm the town draws the kit, not its grey
proxies, and holds its frame rate. P3 now serves a 21,467,658-byte kit with
60.5 MiB of decoded textures; B4 (#10908) still needs web and phone tiers.
Repeat the physical-device measurements after that tier work.

## Admit the villagers' fuller days (town clock)

The town clock now runs by default. On it, a daylight town hour is 4.25
real minutes, and the demo villagers stood still from 13:00 to 17:00: 17
real minutes with nobody moving. Branch `town-clock-routines` gives Mira,
Tobin, and Wren one or two short errands a half day, so some villager
walks in every five real minutes from 05:00 to 21:00, and stages their
proposals. Only you admit villagers. Check out the branch, run
`openagents verse town admit mira-baker --owner`, then `tobin-smith` and
`wren-bellringer`, run `cargo test -p verse-zone-everglade --lib
townsfolk`, commit `town.json` with the branch, and push to `main`. Until
then the roster leaves the changed definitions out, so don't merge the
branch without the admission.

## First paid workflow O2/O8 (REV-10/11/12, #10817/#10818/#10819)

Review the [meeting action-items package](plugins/meeting-action-items/README.md),
choose a real publisher, immutable version, per-call author fee, and supported
payout destination, and publish it through `openagents plugin publish`. On the
selected installed receiver, record its actual release/program/Wasm digests and
quote's separate endpoint and full author fee. Have a separately established
buyer supply consented notes, check the returned task text and source lines,
and accept the bounded result. Participant labels establish no independence.
Authorize one funded payment and record the settlement, exact author share,
payout wallet reference, reconciliation, and consented repeat-use result
separately. Keep the offer unavailable until this execution and payment path
is qualified. The retained fixture uses throwaway keys and fake settlement;
it establishes no deployed availability, independent adoption, ROI, or payout.

Qualify `plugin purchase` in the selected installed CLI with its separately
authenticated customer and explicit resident payer that checks the expected
node identity before payment. Review the exact invoice,
release, input digest, endpoint/author total, routing-fee ceiling, and expiry
before approval. Retain the actual result and original settlement. On the
qualified receiver and installed client, test loss of acknowledgment and
restart at payment, settlement, and invocation boundaries; privately recover
the original purchase and confirm one payment, at most one invocation, and
unchanged author shares. Confirm that another customer, a copied paid proof,
and revoked client rights cannot read its result. Protect and back up the
private buyer book and receiver's shared replay/outcome/ledger custody, and
retain unknown obligations without deleting their bindings. Older purchases
without a private recovery secret require support. Any reversal or new
purchase needs separate authorization; these isolated checks authorize no
payment, refund, deployment, or public offer. The remaining commercial gates
must pass before launch.

## Shared commercial funding (#10828)

Before offering a shared balance, approve the canonical customer references,
original conversion terms, adapter scopes, and protected controller policy.
Activate the intended empty resident wallet explicitly; possession of a node key
or a copied seed grants no shared spending authority. Qualify funding, concurrent
Gateway, compute, and Plugin purchases, refund recovery, and source-loss handling
on the intended deployment with private credentials and owner-controlled funds.
Retain a consistent backup of the canonical ledger and protected policy alongside
the wallet seed, store, custody manifest, required marker, and original handoff database. A restored
or moved deployment requires renewed custody qualification before it can spend.
Isolated tests establish neither funded production use nor customer consent.

## Commercial mapping activation O1 (REV-19, #10826)

Review the explicit native product sources, canonical customer and workspace,
protected operator policy, and independent current owner and member approvals.
Qualify the selected installed customer's Gateway and Plugin references on the
intended deployment. Check rotation, team conversion, revocation, and recovery
against the original quote, payment, execution receipt, and native payer.
Keep credentials and qualification evidence private. Mapping grants no access,
shared balance, spending, or payout right; activate each product's funding and
offer separately. Isolated fixtures establish no customer consent or funded
commercial use.

## Swimming in Everglade on a phone (#10775)

Everglade's ponds and Glade Run (`docs/verse/water.md`, W3) were tested in
code and captured on the desktop. On a physical phone, walk into Lantern
Pond from the commons: you should wade at half speed, then swim. Look down
past about 30 degrees while moving forward to dive, and check that the
breath bar shows over the hotbar under water and that you float back up when
you let go. Note the frame rate with the pond in view.

## Alice's spend records on a live request (#10805)

Spend records, budgets, owner reads, and relay publishing are tested with
scratch homes, file keys, recorded Coder turns, and a fake relay; no live
model call or relay write was made. On a scratch host with a funded model
provider (model spend only; she never pays anyone), ask Alice one terminal
request, then run `openagents agent show alice --owner-key FILE` and check
that `spend today` counts her plan, Coder's turn, and her report. With
relay sync on, check that `relay.openagents.com` serves her `kind:44200`
events only to the owner's key. Archive the Coder task the request makes.

## Water on a phone (#10774)

The shared water shader (`docs/verse/water.md`, W2) validates and translates
to GLSL ES 3.00 and Metal, and the Low tier was captured on the desktop with
no floating-point target. On a physical phone (iOS, and Android on OpenGL
ES 3.0 if one is at hand), walk through the plaza's WATER LAB arch and
confirm the sea, the river, and the falls draw, with foam at the shore and
the glint path at golden hour (`T` turns the hour). Note the frame rate in
the Lab next to the beach.

## Weather and rain on a phone (#10781)

Weather (`docs/verse/water.md`, W9) was captured on the desktop on every
tier (`bench/verse/2026-10-07/everglade-weather/`), and its shaders
translate to GLSL ES 3.00 and Metal. On a physical phone, in the Water Lab
press `U` until the weather reads Rain, then Storm, and confirm the rain
streaks, the rain ripples on the sea, the darker, glossier beach, and the
storm sea; note the frame rate in rain beside the beach. In Everglade, a
build run with `VERSE_WEATHER=rain` shows the same over the town.

## Water refraction and reflection on a phone (#10777)

Medium's scene copies and half-resolution planar mirror
(`docs/verse/water.md`, W5) were captured and timed on the desktop and in
desktop Chrome on WebGPU (`bench/verse/2026-10-07/water-screen/`). On a
physical phone, swim in Lantern Pond and confirm that the cafe and the
trees reflect in the water, that the bed and your legs bend through the
surface, and that foam rings the lily pads and the rowboat. Note the frame
rate with the pond in view, against the W2 note above.

## The clipmap ocean on a phone (#10782)

The Water Lab's sea now draws on the clipmap ocean over a streamed field
(`docs/verse/water.md`, W10), captured and timed on the desktop only
(`bench/verse/2026-10-07/water-w10/`). On a physical phone (Medium), walk
through the WATER LAB arch, look out to sea, and confirm the sea reaches the
horizon with no seams or holes where its rings meet, and that the foam and
the shallows along the beach stay put as you walk the length of the beach.
Note the frame rate. The two-client check of the shared sea ran as a test;
a two-device run waits for the coast zone (C1, #10885), whose clock is the
shared world tick. The Lab's clock still starts when you enter.

## Coast on physical devices (#10885, C6 #10890)

After a phone build includes C1, enter the COAST arch and return to the
saved plaza pose. Walk down to the bay, swim, look down to dive, and use
Jump to rise. Compare the tide on two devices at the same time. Record
seams, rendering faults, and frame rate for C6; C1's native captures and
shared-clock tests do not establish physical device performance.

## Live Gym interview round and judge marks (#10794)

The five-arm interview suite (`alice-interview-v2`) and the `interview-v1`
gate run offline with a scripted answerer and a scripted judge; no live
model or Jev call was made. Run one live round with every file in a scratch
directory, so nothing lands in `~/.openagents/gym`:

```sh
S=$(openagents scratch)/interview
coder interview round --partition development --blocks 3 \
  --answerer live --judge jev --max-usd 1 \
  --store "$S/rows.jsonl" --marks "$S/marks.jsonl" --out "$S/report.json"
```

The answerer goes through the capacity book's existing provider path; the
Claude Code path already passes `--no-session-persistence`, so it saves no
chat. The run stops asking at $1 of reported cost (about $3 for every arm on all
partitions, per the plan). Then mark the judge's readings:
`coder interview sample --store "$S/rows.jsonl" --n 20` lists 20 judged
answers, each with the `coder interview mark ...` line to run (supported and
embellished, yes or no), and `coder interview agreement --store
"$S/rows.jsonl" --marks "$S/marks.jsonl"` reports how often Jev read them
your way. Rerun the round with the same `--marks` and a new `--seed-base`
(for example 3) so the gate counts the judge, and record the numbers and
the gate's verdict in
`docs/decision-models/measurements/2026-10-07-interview-answer.md`.

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

## React-or-continue calibration and a live day plan (#10790)

Alice's day plan asks Jev whether she reacts to an event
(`questions/react-or-continue.json`), which has no live measurement yet.
Run the calibration in
`docs/decision-models/measurements/2026-10-07-react-or-continue.md` with a
live Jev key and record the numbers there. Then turn her morning plan on
(`openagents agent jobs alice add plan`, then `on plan`); the next 07:00
drafts the day. Check the plan board on the great room's west wall and her
F3 page, that every block names real work, and that a request you send
re-plans from the block under way. A day costs about $0.10 at Sol list
prices.

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

## Knowledge drafts from a live reflection (#10793)

After a live reflection stores insights, Alice judges each with Jev
(`questions/insight-share.json`, provisional thresholds: general 0.7, about
the owner 0.3) and drafts the general lessons as NIP-KB candidates in
`~/.openagents/host/agents/alice/kb-drafts/`. Read them at F2 or with
`openagents agent memory alice list`, check that none describes you, and
publish any you want with the printed `microcoder kb publish --dir DIR
--relay URL ID`. A draft costs about $0.01.

## Memory importance calibration and a live interview (#10787)

Alice's briefing now scores importance with Jev
(`questions/memory-importance.json`) under a provisional level mapping.
Run the calibration in
`docs/decision-models/measurements/2026-10-06-memory-importance.md` on the
phase A fixture with a live Jev key, and record the numbers there. Then run
`coder interview --arm all --answerer live --store SCRATCH` to compare the
scored and word-overlap arms with a real answerer; the scripted answerer
can't tell them apart. Both spend a little Jev and model money.

## Villager talk and rumor scores in Everglade (#10792)

The demo rumor `team-in-the-hall` was scored once by live Jev (0.62) and
added to `townsfolk/town.json` for the demo; review it as the owner's
admission in the commit, or remove it with `openagents verse town remove
team-in-the-hall --owner`. On the desktop, open Everglade at noon
(`verse --everglade --town-hour 12:30`), walk to the Market Hall, and press
`F` next to Mira, Tobin, or Wren: with a configured provider each answers
with one model reply in a bubble, under 20 replies a town day, and remembers
you the next time (`~/.openagents/verse/PROFILE-townsfolk.json`, or under
`VERSE_HOME`). Then run the calibration in
`docs/decision-models/measurements/2026-10-07-rumor-repeat.md`. A reply
costs about $0.0003 on Luna.

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

## Delivery, cleanup, support, and reusable templates O1/O8 (REV-07/REV-45, #10814/#10852)

Privately complete the [handoff kit](docs/sales/delivery-kit.json) with the real
customer's exact accepted result/runbook, dependencies, limits, retained
artifacts, and separately accepted support human/boundary. Verify only the
agreed credential/device/resource/data cleanup; retain pending or unknown
owner checks and their next actions. Approve exact reuse rights and privacy
review before transferring generic material, and authorize publication
separately. The synthetic review proves the manual package, not real removal,
customer acceptance, support responsibility, or permission to reuse their work.

Before using the [meeting follow-up template](plugins/meeting-followup/README.md)
with genuine customer notes, agree to that department's workflow, human owner,
input and recipient rights, protected checks, support, and retention. Prepare
and approve a fresh exact snapshot and current permission epoch for each task;
the local runner cannot discover revoked consent or verify operator declarations.
Retain separate customer acceptance and real usefulness evidence. Review exact
material rights and privacy before transferring genuine pilot learning, and
authorize a release/disclosure separately through the existing plugin flow.
The two-customer synthetic comparison is not genuine adoption, paid use, or ROI;
no publication, customer contact, or real-host qualification was performed.

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

## Paid bounded fulfillment activation O7 (REV-48, #10855)

Before a real order, privately qualify independently operated buyer/provider
identities, customer consent, accepted canonical partner assignment, exact
source/disclosure rights, current signed policy issuer and epoch, capacity,
protected checker custody, destinations, fixed BTC millisatoshi amount, routing
fee ceiling, ordered deadlines, support owner, and exact zero executable
revision/rework terms. Keep current policy, canonical credentials, journals,
checker, wallet, and ledger outside provider writes. Authorize a new accepted
order for additional work. Cancellation must retain delivered work, failed
attempts, support causes, costs, and any already accepted obligation.
Start a current receiver/resident build that supports node-bound invoice and
funding calls before admitting an order. Allow its bounded reply window within
the payment deadline and check the returned invoice's actual shorter expiry.

The fixed postacceptance lane leaves credit risk with the worker and provides no
escrow. Separately authorize the actual buyer payment through the existing wallet;
check its fee ceiling and receipt. Compare exact authenticated central inbound
funding with the signed accepted closure and one worker payable share. Qualify
current destinations and actual central payout/unknown-outcome reconciliation
under the existing payout owner. Retain full coordination, execution, protected
checking, failed-attempt, and rail costs, leaving unmetered items unknown. BTC
obligations do not imply USD cost or FX. Synthetic process/relay and fake funding
fixtures do not establish real independent supply, demand, collection, or payout.

## Partner assignment activation O1/O7 (REV-33, #10840)

Before entering a real assignment, privately verify the partner's identity,
exact consent and brief recipients, owner-approved proposal digest, scope,
acceptance, next action, support handoff, and retention. Authorize an actual
introduction or disclosure separately. For charged fulfillment, qualify the
same accepted service obligation and external bill/payment references; an
assignment creates no invoice, execution grant, or payment. Commission references
do not qualify attribution, earnings, or payouts without the owning agreement
and rails. Delete private source copies and exports when their consent expires.
Scratch fixtures establish record boundaries, not independent commercial supply.

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

## Consented activation and weekly review O1/O8 (REV-26, #10833)

Before enrolling real journeys, obtain separate tracking consent and optional
delayed count aggregation permission, agree to data recipients/retention, and
privately qualify immutable account/offer/cohort attribution. Record genuine
install/provider observations, independent task/customer acceptance, and current
settled/refunded payment evidence; fixtures and owner declarations establish no
commercial activation. Accept responsibility and a dated next action for failed
conversions. Qualify the selected paid product lane separately from manual
service evidence. Review actual bills, unknown costs, coverage gaps, and the
chosen seven-day scope before any contribution claim. Use an exact private
report review and explicit release date for count exports; qualify disclosure
rights before publication. Keep real names, messages, amounts, and deal dates
private. Agree to cleanup of separate evidence/export copies after consent
withdrawal or retention expiry; deleting the pipeline record removes no external
copy. These operations contact no customer and transfer no money.

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

## Customer connection O8 (REV-09, #10816)

Qualify the installed macOS arm64 CLI's [commercial customer commands](docs/cli/README.md#commercial-customer-openagents-customer)
against the chosen gateway's existing account/session and funded-decision
configuration. Supply real credentials privately, verify the intended customer,
workspace, payer, current rights, recovery, rotation, revocation, and retained
receipt attribution, and record external customer acceptance. Keep the selected
offer unavailable until its O5 backend, funding, price, result, and charge evidence
is qualified. Isolated fixtures establish client behavior; they do not establish
real funding, production deployment, customer acceptance, or another platform's
account connection. Device pairing and provider login confer no spend authority.

## Team adoption O8 (REV-38, #10845)

Qualify the selected installed CLI's [team commands](docs/cli/README.md#commercial-customer-openagents-customer)
with a genuine champion and colleague against the chosen gateway. Authorize the
private invitation handoff, verify the intended account, workspace, role, and
payer, then retain an independently useful admitted task and its original
settlement receipt. Check expiry, withdrawal, single use, role changes,
revocation across open sessions, personal and team switching, and recovery under
current rights. Recovery must not restore a removed membership or reveal
another member's credentials. Keep invitation files and customer records private.

The isolated two-client fixture uses synthetic balances and a fake decision
backend. It establishes installed client behavior and payer attribution, not
external adoption, actual funding, revenue, or production service qualification.
Team membership alone supplies no new budget or product spending grant.

## Native team budgets (REV-40, #10847)

Approve the chosen workspace's cumulative currency caps, alert thresholds,
reviewed native person/team roster, and the supported decision offers before
enabling `money.hierarchical_budgets`. Install the versioned policy through the
current owner's authenticated budget route. Qualify real concurrent calls,
revocation, lowered limits, and recovery against the O5-funded deployment,
retaining the actual policy, journal, and result evidence privately. Earlier
unattributed obligations count conservatively against child caps; review that
exposure without guessing a person or releasing unknown work. The isolated
fixtures establish enforcement and killed-process recovery with synthetic
funds, not actual team limits, real funding, deployment, or cross-product
budgets. Qualify the existing native quota marker recovery too: confirm its
exact writer has exited before removing its stale marker, and retain both
journals and every unknown monetary liability.

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

## Native prepaid card activation O5 (REV-22, #10829)

Before enabling `billing.prepaid`, select and qualify the processor and merchant,
accept the exact USD funding policy, fee payer and cap, purchase and spend
limits, refund and dispute responsibility, and loss treatment. Supply restricted
native credentials and webhook secret references outside the repository;
configure the pinned API version, live mode, HTTPS return origin, and public
webhook. Retain a genuine final collection, duplicate and restart reconciliation,
and refund or dispute evidence privately before commercial activation. The
optional Stripe adapter and isolated native HTTP fixtures establish code
behavior; they establish no live merchant, card sale, production fee, or wallet
liquidity. Common funding and BTC conversion remain unavailable in this profile.
The native dashboard and Rust SDK (REV-23, #10830) use this same profile.
Qualify a real hosted checkout, session-bound approval, read-only browser return,
and original-purchase recovery on the chosen deployment before offering it to
customers; isolated browser and provider fixtures establish these code paths.

## Native decision offer O5 (REV-16, #10823)

Before activating the [selected native `kev-0.6b` offer](docs/decision-models/service/monetary-accounting.md#selected-native-decision-offer),
accept its commercial price version, input-token rate, account currency,
funding policy, and customer responsibility. Qualify the retained artifact
family's actual loaded digest and all execution settings on the selected CPU
host; pin them in both the offer and dedicated registry binding. Verify one
concurrent call, the explicit request-rate limit, and agreement between the
backend's enforced token ceiling and the monetary hold. Supply restricted
deployment credentials and protect the existing ledgers from executor writes.
Retain genuine funding finality and one admitted typed result with its exact
payer, observed input count, charge, and sealed receipt; reconcile a duplicate
and an unknown attempt. Record measured provider and hosting expenses
separately, or keep them unknown. Local synthetic checks require no host
deployment and establish no production price, funded sale, or inference
qualification. Public terms continue to label qualification unknown.

## Decision funding and installed client O5/O8 (REV-17, #10824)

Select and approve the BTC-denominated decision offer and its native funding
policy before enabling the optional gateway adapter. Provision the customer's
monetary account and exact conversion; one BTC currency-millionth is 100,000
millisatoshis. Supply an explicitly admitted Lightning resident, pin its node
and network, and protect its private custody and the gateway's funding journal
from executors. Deploy matching identity-fenced resident and gateway builds.
Do not infer receiver liquidity or expense from invoice creation.

From an independently installed customer client, retain one actual quote,
explicit digest approval, invoice, confirmed receiver observation, and original
workspace credit. Then retain an admitted decision result and its pinned price,
observed usage, charge, and sealed receipt. Check duplicate settlement, a lost
reply across restart, an unknown usage hold, changed membership, and reconciliation
before another attempt. Funding is unused customer credit until verified usage
settles; keep actual payment/provider expenses separate and unknown when no
authoritative counter exists. Synthetic HTTP and ledger fixtures establish
implementation behavior and do not establish a funded sale or commercial launch.

## Sales outreach launch

REV-51's sales presets and native owner controls are code-qualified with
isolated fixtures. Use matching client and host builds; an older host
refuses the new operations. Before using a real model for Paul or a hire,
select and admit that member's provider, private assignment, and spend cap. The initial
charter permits supplied-request drafting only; its signed owner-recorded
verdicts do not certify claims, prove a Jev call, or authorize outreach.
Live helpers, certification, hiring, and sending still require their own
implemented adapters and explicit owner activation. These steps do not
block the native role and verdict implementation or human-led first revenue.


REV-53's private agent records are code-qualified with isolated native file-key
and CLI fixtures. Before admitting a real member, verify its native owner
attestation, exact job charter, private lead consent and `agent:PUBLIC_KEY`
recipient, policy version, US jurisdiction, business timezone, review caps,
execution budget, and expiry. Keep the issued scoped credential outside the
host root. Review proposed drafts and manual certification references privately;
these references are attributable owner records, not measured REV-57 certification.
The record route performs no model call or outbound action. Engram publication,
model helpers, measured certification, and sending need their own implemented
adapters and explicit activation. No customer material belongs in memory;
`agents memory` returns only current admitted random references and fixed fields.
These owner choices do not block the native records implementation.

REV-60's contact privacy controls are code-qualified with isolated native and
installed CLI fixtures. Before real use, verify the original customer/account ID,
US business source, actual requested contact or accepted introduction, exact
permission pins, linked aliases, human responsibility, and permitted recipients.
Review the default 90-day inactivity period or record a new owner policy version;
record engagement only from an actual customer reply or accepted introduction.
Use private export directories, and review any unavailable or truncated cleanup,
unmanaged captures, or historical signed records that need repair. Native copy
removal and local engram minimization do not prove physical or remote erasure.
The current sales profile admits no customer model disclosure or relay sync and
no sending; a future adapter needs separate recipient and legal qualification.
These activation and repair steps do not block the native privacy implementation
or human-led first revenue.

REV-61 qualifies the native mailbox adapter with isolated provider fixtures,
private file credentials, and an injected sealed source over an existing host
account authority key. It accepts actual bounded SMTP/OAuth/API credential bytes,
not a fabricated 32-byte provider token. Configure the dedicated sender and
monitored reply mailbox, current SPF/DKIM/DMARC alignment and authenticated TLS
evidence, exact permitted provider recipients, approved templates, accurate
sender and postal identity, and a working all-marketing unsubscribe mechanism
available for at least thirty days after a possible send. These records are
owner declarations, not independent DNS or legal verification. No mailbox was
contacted, no owner keychain item was created, and preparation grants no sending
authority. The [FTC business guide](https://www.ftc.gov/business-guidance/resources/can-spam-act-compliance-guide-business)
describes the commercial email requirements, including business recipients.
Actual provider qualification, O6 grant, and reply-injection acceptance remain
required before live sending; they do not block code completion.

The native reply handler supports explicitly untrusted owner imports and measured
scratch-injection safety results. Keep a human monitoring the dedicated inbox;
SMTP configuration does not select or qualify an automatic receiving adapter.
Imports and owner classifications do not prove provider delivery or model quality.
Run the current handler qualification after source changes, review safety pauses,
and approve each exact response. Original import files and unmanaged mailbox
history need their own retention; native minimization cannot erase them.

The [sales-floor decisions](docs/sales/agent-sales-floor.md#initial-operating-decisions)
start with permissioned US business email and individual approvals. Before
live outreach, configure the dedicated domain and monitored mailbox; verify
SPF, DKIM, DMARC alignment, TLS, commercial identification, and
unsubscribe/suppression handling; confirm
the recipient scope and footer with counsel; accept the certification sample;
and grant the exact campaign, model budget, and five-message daily pilot cap.
The native outbox and bounded SMTP transport have isolated scratch acceptance;
live sending still requires the current native qualifications and exact owner
activation. SMTP acceptance does not prove delivery. Keep qualified
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

- Useful contribution payments O7 (#10729; REV-49, #10856): Qualify one real
  protected Choice-accuracy checkpoint obligation through `contribution-service`.
  Admit independently controlled evaluator and acceptance keys, private evidence
  outside worker custody, current signed item/group/license permissions, exact
  baseline/artifact bytes, a frozen native policy/recipe, complete known bills,
  explicit reward/fee/budget/expiry, the central receiver, and the contributor's
  registered payout destination. Then separately authorize bounded real funding
  and the existing central payout path; retain exact receiver and payout lookup
  evidence, including unknown recovery. Synthetic protected/fake-wallet checks
  prove local admission and accounting only. Real independent usefulness,
  transferable improvement, licensed data, funded adoption, and every real
  contributor payout remain unverified. XP and activity authorize no sats. No
  owner host deploy is required for the standalone private operator adapter.

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
--funded --confirm DIGEST --bindings PATH --out /absolute/new/funded.json`
once, paying its printed top-up of at most 1,000 sats. Keep the receipt, the state directory's ledger and
journal, Boat usage, and artifacts; check the sandbox is deleted and
nothing stays held. Real readiness, latency, Boat billing, Lightning
top-ups, and the owner program (`owner-v1.sh`) on a real daily template
(its `git`, `setsid`, and `codex login --with-api-key`) are unverified. A
failed run opens a defect issue.

## Launch the retail cloud service O3/O4/O8 (#10724, REV-13/REV-14/REV-15 #10820/#10821/#10822)

The launch gate, monitoring, and operator runbook are on main
([`docs/cloud/retail-operations.md`](docs/cloud/retail-operations.md));
`retail-qualify advertise` reports `contract_unconfirmed` and advertises no
paid computer. To launch, in order: complete "Review the first retail cloud
contract" and "Funded retail cloud qualification" (including #10748's live
bindings); deploy the service from current main under a separate retail
Boat account and receiver wallet; configure the confirmed contract and native
funded evidence in the reviewed production host configuration; check
`retail-qualify health` is clean; and
publish the contract and prices to customers. REV-13 adds the authenticated
loopback transport and resident worker with synthetic HTTP, fault, and actual
process-restart checks. Its private credential-custody addendum also needs
commercial/data-policy approval. O3/O4 still require the dedicated retail
Boat key, resident receiver, exact template and plan/start allowance, real
funded qualification, and the supervised TLS deployment from current
main. No owner host deployment or live credential/fund check was performed
for REV-13. Use the [service configuration](docs/cloud/retail-operations.md#service-configuration)
and qualify REV-14's native `retail-client` with an explicitly selected HTTPS
origin, separate retail account/principal/bearer, current spend/execute/disclosure
rights, and customer-owned OpenAI key. Its actual binary/HTTP acceptance uses
fake funds/providers only. O3/O4/O8 still require installation and a real funded
quote/confirmation, changed-right refusal, cancel/reconnect/retained receipt,
measured billing, credential removal, and live incident handling. No live host
deployment, buyer credentials, or funds were used for REV-14.
REV-15 supplies the closed configuration/build package, bounded units and
ingress, exact running-identity approval, native qualification reconstruction,
private operator status, and stopped checkpoint/restore/rollback commands.
Its actual binary restart/recovery and signed receiver tests use isolated
synthetic providers and funds. No owner deployment, Caddy/systemd host
activation, public TLS qualification, real provider expense, or historical
binary compatibility is claimed. Complete the [deployment procedure](docs/cloud/retail-operations.md#deploying),
review the exact ingress and running identity, check actual resource limits,
backup deletion at its original artifact deadline, provider usage and costs,
then qualify the selected client at the real HTTPS origin. A production
approval cannot replace the native payment, checked delivery, or cleanup
proof; changing private source bytes closes paid admission. These owner
activation steps do not block code completion.

## Exact team capability qualification (O9, REV-39 #10846)

Use the selected Unix `openagents plugin team` client with an owner-selected
native workspace, separate real colleague credential, exact signed zero-fee
release and scoped evaluation review, and independently authorized input bytes.
Verify colleague discovery, explicit install/off, exact enablement, local Wasm
use, and fresh member removal, expiry, source withdrawal, and changed-scope
refusals. Confirm that publication and the input's rights permit this reuse.
Offline acceptance uses throwaway native accounts, signed public fixtures, and
an existing Wasm guest; it establishes no real team agreement or general plugin
qualification. No host deployment is needed for this local CLI lane. Other
clients need their separately admitted matching integration, and paid releases
must use their owning current quote and purchase path. Keep first founder-led
revenue independent of this qualification.

## Curated discovery (O9, REV-46 #10853)

Approve the bounded public publisher/service source set, its exact signed
releases or service heads, and attributed local review records before using
`openagents plugin discover` commercially. Maintain fresh publisher checkpoints,
public source provenance, exact evaluation scope, data and recipient requirements,
and explicit current fee pins. Retain each JSON snapshot for subsequent refresh
checks. Qualify actual provider availability and the selected paid lane separately,
then obtain and approve the owning customer's current total quote. Fixture-backed
discovery verifies signed metadata and bounded native support; it does not establish
independent delivery, capacity, a total purchase price, or spending authority.
No real publisher, service, customer, credentials, or funds were used for REV-46.

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

## Rotate, retire, and move a workshop agent (#10804)

Scratch-host tests cover rotation, retirement, moves, snapshots, and NIP-GS
signing with file keys, a fake keychain, and a fake relay. On your own
computers:

1. With the desktop app running, rotate Alice: `openagents agent rotate
   alice --reason "first rotation"`. The host uses the owner key it holds.
   Check `openagents agent memory alice engrams --owner-key FILE` still
   decrypts every engram, and that `agents/alice/lineage.jsonl` holds one
   row. With relay sync on, check that the relay took the `kind:9035`
   archive request for the old key.
1. To move Alice to another of your computers, copy `agents/alice/` there
   (with her key: `key`, or the `agent:alice` keychain item), then run
   `openagents agent move alice --to HOSTKEY` on the old one. Enrolling her
   key on the new host as a device with a delegated NIP-HOST grant of at
   most `operate` and `terminal` is not built yet; grant it yourself when a
   flow needs it.
1. To sign her merged worktree commits, run `openagents agent signing alice
   on`, merge one of her changes at the Merge station, and check the tip
   with a NIP-GS verifier.

## Referral source and attribution activation (REV-27/REV-28, #10834/#10835)

Review the public `/join?ref=TOKEN` wording and the explicit
`openagents.referral.consent.v1` presentation before distributing links. Qualify
signup and selected `openagents customer referral` commands against the intended
account deployment, and identify its canonical private account directory before
an assisted pipeline owner records an introduction. Synthetic checks establish
source custody, replay refusal, migration, and redaction; they establish no real
customer acquisition, permanent attribution terms, commission, or payout. Keep
OpenAgents sales-agent identities source-only.

Before promising a permanent relationship, agree on the exact attribution terms
and exception wording, then publish a new immutable policy with
`tenant-referrals publish --registry DIR --input FILE`. The private input is
`{ "version": "YOUR_VERSION", "terms": "YOUR_AGREED_TERMS" }`. The implemented
`consented-permanent-review-v1` rule requires separate customer consent; early
agreements, existing customers, and corrections require both parties' review.
Competing or missing evidence does not choose a winner. Legacy source records
without signup provenance remain unknown and retain their original evidence.
Keep the agreements identified by private evidence digests under the agreed
retention policy. Qualify actual customer consent, recovery, team ownership,
and referrer management succession on the intended account deployment before
distribution. A successor changes management; it does not transfer a payment
right. Later commission contracts decide eligible usage and earnings; synthetic
checks create no real referral, commercial agreement, or commission.

## Referral commission contract activation (REV-29, #10836)

Choose and agree on the commercial eligible products, rational share, earned
OpenAgents base, exact native unit, rounding, hold duration, payout minimum,
qualified destinations, refund and dispute liability, attribution conflict rule,
and permanence. The Rust contract supplies no commercial defaults. Its selected
profile uses the same exact unit throughout; it performs no FX conversion.
The existing Spark and Lightning destination rules accept only BTC denominated
terms (sats, msats, or BTC millionths). Declare whole-satoshi payout precision,
retention of unpaid msat remainders, and a minimum that converts exactly to
whole sats within the native ledger's integer bound. No terms agreement
qualifies a real payout destination. Broader currency or rail offers require an
explicitly reviewed conversion and an actual settlement adapter.
Keep signed author fees outside the commission base, exclude unused funding,
promotional or free credit, self-referral, recycled funding, unknown costs, and
unresolved attribution, and retain paid reversal obligations separately.

Prepare a bounded private `Terms` JSON file from the native contract in
`crates/tenancy/src/accounts/referrals/commission.rs`. Use `tenant-referrals
commission-check --input FILE` to derive and review the digest without publishing.
After commercial review, retain that sealed file and explicitly publish with
`commission-publish --registry DIR --input FILE --approve DIGEST --expected
DIGEST|none` against the intended canonical account directory. Changing an
existing version is refused. The registry directory must be owned and have mode
`0700`; the bounded private input file must have mode `0600`.
Qualify the intended Gateway, SDK, and installed
customer consent and historical-read path before distributing the published
terms. Both native parties must accept the exact terms and accepted attribution
decision. Publication, consent, and the inert supplied-fact preview enable no
accrual or payout. REV-30 must independently verify earned settlements, actual
costs, attribution, reversals, and central-ledger liabilities; the isolated
synthetic checks establish no real commercial agreement, buyer, or commission.

## Remote placement for gates and benchmarks (#10767)

`openagents lease run --class CLASS` places release gates and benchmarks on
another computer over SSH, but no computer is configured by default, so they
run here under `quiet` until you name one. On this Mac, check that `ssh
coderos-4080 true` succeeds without a prompt, then run `openagents settings
set coder.placement computer=coderos-4080`. Make the first real remote run
yourself, for example `openagents lease run --class release-gate --place
remote:coderos-4080 -- ./scripts/verify-rust.sh --release`, and check that it
checks out the pushed commit under `~/.openagents/remote-runs/openagents/`
there and that its receipt lands in `~/.openagents/leases/placements/` here.
Tests used only a stand-in `ssh`; no real computer was reached.

## Team policy activation (REV-41, #10848)

Choose actual team data classifications, exact input provenance, permitted models
and releases, recipients, local placements, and expiry before activating
`team_policy` on an owner deployment. Qualify native backend identity and
disclosure rights separately. Only the isolated local Gateway SystemOne lane is
code-qualified; cloud, customer-host, plugin, and fallback execution remain
unavailable under this policy. No fixture uses owner hosts or funds, and policy
review does not create spending or source grants.

## Rowboats and wakes on a phone and with two players (#10778)

Everglade's rowboats, wakes, and splashes were checked in tests and in
offscreen captures only. On a phone, swim across Lantern Pond and check
that the wake and foam trail show and the frame rate holds; then board
the rowboat from the jetty with F, row across and back, and step out onto
the jetty. With a second player in the same world, have them board as the
passenger, and check that both see the same boat as it moves.

## Sales cohort activation O1/O6/O8 (REV-52, #10859)

The selected Unix host's owner-only crew controls are qualified with isolated
native members and a fake external outbox. Before real sales activity, qualify
stop/pause and explicit resume on the chosen deployment, including the exact
REV-62 approval, durable handoff, suppression, and delivery-recovery adapter.
The default adapter is disabled. Fixtures authorize no campaign, recipient,
funds, or real contact; an interrupted or unknown external effect remains
unknown. Human-led first revenue needs none of this activation.

## Spells on water in the Water Lab on a phone (#10780)

The spell rules on water were checked in scenario tests and offscreen
captures only. On a phone, open the Water Lab, cast Sleet Storm over the
bay and swim into the slush (it holds no one), cast Control Water through
its modes out past 50 m (nearer the beach the whirlpool refuses), and
strike the sea beside a dummy with the Thunderbolt; check that the frame
rate holds while the ice, trench, and whirlpool draw.

## Team report qualification O5/O8 (REV-42, #10849)

Qualify the selected native decision gateway's private team reports with a real
champion and colleague. Protect the canonical evidence root outside executor
write custody, retain the original member, task, ATIF, checker, acceptance, and
statement references, and verify useful owner/admin and member views under current
rights. Review source removal, expired credentials, revocation, restart, refunds,
unknown expense, and measured reservation-to-dispatch waits before agreeing to a
service-level target or publishing aggregates. The isolated fixtures establish
native report and browser/export behavior with fake provider responses and
synthetic balances; they establish no actual funding, external adoption,
production performance, independent remote attestation, or earned cash revenue.

## Underwater on a phone and in the browser (#10779)

Diving, the split waterline, caustics, sun shafts, and motes were checked
in offscreen desktop captures only (`bench/verse/2026-10-07/water-w7/`). On
a phone (Medium), dive into Lantern Pond and check that the view turns
blue-green, the waterline splits the screen while surfacing, caustics move
on the bed and on your character, and the frame rate holds; do the same in
`everglade-web` under WebGL2 (Low). Native underwater sound is wired in
the mixer but no zone drives it yet.

## Native plugin commission activation (REV-30, #10837)

Use the existing authoritative private merchant ledger, protected original
customer purchase, native Registry/Accounts grants, and original REV-29 bilateral
BTC terms. Qualify the explicit private `pay commission` configuration, receiver
node, all six cost classes, accepted payout destination and minimum, and actual
resident collection, payout, and refund evidence before enabling the profile.
Synthetic ratios, costs, invoices, and fake-rail checks establish no commercial
rate, paid customer, or real-money qualification. Missing costs retain a hold;
declared costs remain declarations. Cross-registry shared-wallet commission
activation is refused until an adapter binds the original native buyer and
merchant grants to canonical bilateral terms. Wallet possession supplies no
account authority. Keep shared-wallet refunds disabled until the separately
authorized REV-20 expense-refund adapter is available. USD,
conversion, and other product commission sources require their own native
adapters. Unknown invoice creation and payout outcomes keep their original
references; qualify recovery and any funding loss without reminting an invoice,
reexecuting a purchase, or rewriting author shares. Gateway referrer statements
require explicit commission activation, current native referrer management,
current workspace membership, and qualified rail evidence.

## Paper Mono on the phones and CoderOS (#10904)

Every surface now names Paper Mono (`crates/paper-mono`), but no phone or
CoderOS build ran after the change, because builds were held to the build
lease after the 14:50 crash. Build and look at each:

- iOS: run XcodeGen and build `bins/openagents-ios` and `bins/coder-ios` for
  a simulator or device. Each `project.yml` bundles the four static faces
  from `crates/paper-mono/fonts/` and each `Info.plist` lists them under
  `UIAppFonts`; check that text draws in Paper Mono, not the system face, and
  that nothing clips. The mockup (`bins/openagents-mockup-ios`) built and
  rendered Paper Mono on a simulator before the crash.
- Android: build `bins/openagents-android` and `bins/coder-android`
  (`scripts/build-openagents-android.sh apk`). The `paperMonoFonts` Gradle
  task copies the faces into a generated `res/font`; this machine has no
  Android SDK, so neither the task nor the Kotlin compiled here.
- CoderOS: rebuild a host with `coderos.desktop.enable` and check that
  `fc-match monospace`, `fc-match sans`, and `fc-match serif` print Paper
  Mono, and that foot draws Coder's braille spinner.

## Referral abuse review activation (REV-32, #10839)

Review the fixed `original-commission-abuse-review-v1` rules, named native
merchant reviewer, bounded review interval, evidence sources, and appeal
procedure before enabling live referral payouts. `pay commission abuse-review`
requires a private input and explicit approval of its rules digest. Unknown,
model, and reputation evidence retain a scoped hold; expiry requests review and
never releases funds. Operator review is a declared finding, not independent
attestation. Qualify recycled-funding and promotional-credit evidence against
the original funding source before declaring it absent or present. The native
plugin lane refuses unqualified cross-registry funding. Rejected claims retain
their original liability and history; an appeal requires a separately reviewed
remediation, not a new invoice, duplicate commission, or payout retry. Synthetic
checks establish no real customer identity, fraud finding, or payment authority.

## Sales model expense activation (#10861)

Approve an exact private `sales models` policy with reviewed source and price
revisions, finite full-input, output, retry, and deadline caps, and narrower
agent and request limits under the fixed $5 Chicago-day floor. Each real
provider adapter must enforce those caps and reserve every helper or hidden
downstream call before execution. General sales model paths remain unavailable;
synthetic fixtures and deterministic local zero-cost helpers establish no
provider capacity, billing, successful result, qualification, or contact grant.
Reconcile interrupted usage from actual provider evidence while preserving the
original receipt, unknown liability, and distinct estimate and billed amounts.
Migrate the complete private canonical sales book with native expense identity;
an imported agent alone cannot reset floor or agent costs. Synthetic training
uses the same expense floor and owner-approved persona/run/source pins without
customer records or outbound authority. Customer material requires a separate
typed field-scoped disclosure adapter; arbitrary expense input refuses retained
protected identifiers and credentials.

## Sales evidence and written practice activation (REV-55/REV-56)

Review the exact playbook, maintained claim sources, full price terms, and
source/review digests before publishing helper inputs. Approve the helper or
training source in the canonical `sales models` policy, with its input,
output, retry, and deadline bounds. Local deterministic helpers and
`training run-scripted` have zero model cost and establish no provider
availability, customer proof, passing grade, certification, or contact grant.
Select and qualify a bounded real model adapter before using practice quality
as commercial evidence. Retain the original partial transcript and expense
when execution fails or is interrupted; a new controller or schedule cannot
reset unknown liability. Supply owner-marked examples and original reviews
for REV-57 calibration and locked evaluation before enabling prospect drafts.
Use the qualification owner's exact review controls with ten passing written
practices across five situations and twenty distinct reviewed samples, including
opt-out and ambiguous consent. The bounded host adapter must prove its original
model identity, full input/output/retry/deadline caps, and actual price source;
there is no default live-provider adapter or zero-cost subscription inference.
A measured certificate never grants sending authority. Correct an attributed
complaint, changed identity/playbook/source, or two failed real drafts in one
Chicago week before collecting fresh practice and review evidence for
recertification; reusing an old sample or changing the certificate ID cannot
reset the suspension.

## REV-59 meeting and human handoff activation

Publish finite available slots under the pipeline owner's current credential,
review the private brief against the checked claims and current pilot kit, and
confirm the exact requested proposal. The named human must separately accept
before the meeting has an agreed responsible person. These records do not read
or change a calendar, send invitations, agree to prices or commercial terms,
create product credit, or establish earned revenue. Keep real-contact permission,
source and recipient boundaries, baseline unknowns, pilot scope, acceptance
criteria, next action, and review date current. Changed lead or proposal scope
requires a fresh confirmation; missing briefs, stale slots, declines, and absent
acceptance remain pending. Human-led R0/R1 remains independent of this activation.
## Joined original financial statement qualification (REV-24)

Before enabling joined statements for an owner account, review its exact native
and canonical member snapshots, original source bindings, expiry, and separate
payee read scope in the protected shared-custody controller configuration.
Compare a private statement with the original receiver, native price and receipt
journal, and actual payout reference. Qualify historical reads separately after
linkage retirement. Real payment and device qualification remain owner steps;
synthetic conservation and current authority checks do not attest those outcomes.

## Joined team operating qualification (REV-43)

Qualify one real team on its selected installed client and enabled native routes.
Review invitations, recovery delivery, current roles, exact capability releases,
data recipients, policy narrowing, limits, and private exports with the team's
owner. Confirm that recovery does not restore removed membership or old device
access. Device, real payment, and external-team observations do not follow from
the isolated native qualification; unsupported routes remain unavailable.

## REV54 — activate Paul's sales controls

Before using the controls with real assignments, approve the exact Paul binding digest, current private sales-owner credential, native identity and charter, requester IDs, assignment credentials, and local helper sources. Keep requester authority separate from Studio or phone observation grants. The default host leaves model work unavailable. A paid adapter requires actual current provider, served-model, list-price, full-context, output, retry, deadline, private-data, and billing custody; a subscription or source declaration does not establish capacity. Use current measured qualification before real prospect drafting, and retain unresolved expense receipts after interrupted calls. Scratch fixtures do not certify a real campaign or authorize customer messages.


## Private Agora board activation (REV-69)

Configure the two explicit private paths in the [Agora observation guide](docs/sales/agent-sales-floor.md#private-agora-observations-rev-69) only for the sales owner. Qualify the original Paul binding, assignment credentials, native key custody, current certificates, and pending proposal references on that computer. Missing or changed custody keeps the boards unavailable. These reads authorize no mailbox, approval, booking, payment, campaign, or shared publication; real model and outreach qualification remains in the existing owner steps.

## Sales hiring activation (REV-64, #10871)

The hiring book, caps, and owner decision path are code-complete and
exercised by fixtures over scratch hosts. Owner-only steps remain:

- Confirm the floor caps the code enforces: Paul plus at most three active
  hires and a USD 5 daily model ceiling for the floor. A wider ceiling needs
  a new owner grant, not a code change.
- Confirm the first real hire on your own host: `openagents agent hire list`,
  then `agent hire confirm ID --expected SHA256 --workspace DIR` with the
  host's owner key present. The hire starts in training and cannot draft
  for a real person until certified.
- Deferred, as answered on 2026-10-04: pilot price (proposal USD 250),
  provider budget per pilot, promotional credit (default none), card
  provider and unit conversion, commission base and share, support owner
  and hours, pipeline owner, sales domain and legal review, first-team
  controls, and store release steps.

## Sales floor report activation (REV-65, #10872)

The report, escalations, and weekly draft work with no revenue, no financial
adapter, and no delivery telemetry; absent inputs are labeled unknown or
listed under `gaps`. Owner steps:

- Run `openagents sales floor escalations --root DIR --credential FILE` on
  the real sales root whenever the outbox is active; complaints and pauses
  appear as `immediate`. No scheduler or notifier is installed by this code.
- Decide whether a weekly draft is published; `sales floor weekly-draft`
  never publishes, and `sales review` remains the exact owner decision.
- Name the pipeline owner (deferred decision 7) before relying on the report
  for the weekly review.

## Sales bodies in the Agora (REV-70, #10877)

`openagents sales town bodies` places Paul and confirmed hires from the
hiring book and the canonical sales books on any host; it reads only. Owner
steps:

- Confirm a first hire (`openagents agent hire confirm`) before any body
  other than Paul stands on the real floor; fixture bodies come from
  `--member` and are not admitted members.
- The Everglade client's rendering of these bodies through Bob's shared
  seat figures is owner-reviewed pack work; until then the projection is
  the CLI's JSON and the private Agora boards.


## REV-71 earned sales (#10878)

- The bell rings only from verified `paid` settlements with reconciled
  delivery recorded through `sales service`; the owner records both.
- Publishing a shared aggregate anywhere outside the owner's board (the
  website, a weekly update) is an owner step after
  `openagents sales earned approve`; nothing ships it automatically.

## REV-72 partner and affiliate desks (#10879)

Code is complete: Arthur and Vanna bind to their own crew anchors and read
the canonical pipeline with every money limit reported as unavailable.
Owner steps:

- Approve each desk binding against its exact digest (`configure_desk`) once
  the native `arthur` and `vanna` agents are admitted through the hiring book.
- Publish referral and partner terms, commission base and share, hold
  period, and payout rail (O7). Until then `Limits` stays all-false and no
  brief or draft may promise earnings.
- Name the growth owner recorded in each binding.

## REV-66 reviewed batches (#10873)

Code is complete: `grant_batch`, `revoke_batch`, `raise_batch`, and
`openagents sales outbox batch-qualification`. Owner steps:

- Run four real clean weeks at level 0 with at least 100 delivered live
  messages across 25 permissioned contacts; fixture history never counts
  toward a live grant.
- Read every item of the first five batches and grant each batch against
  the exact qualification digest. Nothing promotes on its own.

## REV-73 standing follow-ups (#10880)

Code is complete and disabled: `grant_standing`, `revoke_standing`, and
`openagents sales outbox standing-qualification`. Owner steps:

- Operate at least one fully delivered reviewed batch (REV-66) on live
  history before any policy can qualify.
- Choose the invited-thread cohort and grant each policy separately against
  the exact qualification digest, naming the reviewed inviting reply. No
  batch grant, measurement, or elapsed time enables a policy on its own.

## Configure the Cloud account connection (#10949)

Before enabling `/cloud/app`, qualify an explicit native account service and
HTTPS origin. Pass `--cloud-config PRIVATE_JSON` with schema
`openagents.cloud.web-config.v1`, `public_origin`, `account_service`, and an
absolute `csrf_secret` path; both files must be private, owned, regular files
under a private directory. The secret contains 32 random bytes. Build
`scripts/build-coder-cloud-web.sh OUTPUT` and pass `--cloud-build OUTPUT`.
Account creation, recovery-token issuance, and other-session revocation remain
with the native account owner. Qualify native recovery and revoked membership
on the deployed origin before enabling sign-in. This connection grants no
computer, execution, sales, custody, or spending rights.

## REV-44 browser purchase surface (#10851)

The owner chose the browser on 2026-10-04. Code lands a read-only purchase
browser (`openagents-web --customer DIRECTORY`, `/app/purchases`) over the
same customer store the installed client writes. Owner steps:

- Qualify the surface on a real install: run `openagents-web --customer
  ROOT` beside an installed client, quote and approve one purchase on the
  client, and confirm the browser shows the same payer, quote digest,
  approval digest, and receipt through a restart of both. Retain the actual
  outcome and limitations under `docs/payments/`.
- Decide whether approval or cancellation should ever move into the
  browser. Today they stay on the installed client, which holds the resident
  wallet and the private purchase authorization; moving them is a separate
  issue with its own authority design.

## REV-47 supplied-snapshot plugin step (#10854)

The owner left the richer operation to the agent on 2026-10-04. Code lands
one bounded profile: a paid `snapshot-read` step whose release names the
files it reads, and whose buyer supplies exactly those files as UTF-8 text
with the request (`openagents plugin purchase quote --file NAME=PATH`). The
guest reads nothing else; the packet, quote, approval, and receipt bind the
supplied snapshot's digest. Owner steps:

- Qualify one paid supplied-snapshot purchase against a real provider and
  wallet (`plugins/meeting-followup` with its `examples/meeting.md`), and
  retain the receipt under `docs/payments/`.
- Decide whether any paying buyer needs an effectful operation (a patch, a
  fetch, a delivery). None is admitted; a release that declares one is
  refused by the paid route and needs its own authority design.


## REV-74 public reply channel: GitHub Issues (#10881)

Code state: `coder::task::sales::outbox::public_reply` posts GitHub Issue
comments. A `Mode::Fixture` grant posts only through `FakeTransport`; a
`Mode::Live` grant posts through `GithubTransport` from
`Store::public_reply_dispatch_live`, which loads the account's token from
the host credential broker and refuses unless its digest equals
`Grant::credential_sha256`. Before the owner records a live grant:

1. Name the invited OpenAgentsInc issue threads where a maintainer or
   prospect asked for an answer, and record the invitation evidence digest
   as `Grant::invitation_sha256`.
2. Create a dedicated GitHub account for the agent whose profile states
   that it is an AI agent operated by OpenAgents, and record its login as
   `Grant::account` and the label (login plus "AI") as `Grant::label`.
3. Issue a fine-grained token scoped to those repositories with
   `issues: write` only, hold it in the host credential broker under the
   account name, and record its digest as `Grant::credential_sha256`.
   Never place the token in the store, a request record, a log, or a
   fixture.
4. Post the first live reply with the grant's daily cap at one, read the
   comment back on GitHub, and reconcile any `Unknown` attempt before any
   further reply in that thread.

## Qualify resident Cloud observation (#10950)

Before exposing resident task records on the deployed origin, provision an
explicit account/workspace/membership-epoch mapping to a separately issued
Observe grant and private device key. Pass `--cloud-hosts PRIVATE_JSON` with
the protected binding schema in the web crate README and an exact authenticated
host route and generation. Qualify current revocation, workspace removal, host
restart, source changes, and HTTPS browser cleanup using the deployed native
owners. The isolated account/host fixture supplies synthetic evidence only;
it enrolls no owner's browser, starts no engine, and activates no retail,
execution, terminal, sales, custody, or spending lane.

## Qualify reviewed Cloud controls (#10951)

Before enabling controls on the deployed origin, separately approve server
custody for each explicitly mapped native device and configure the private
control journal described in the web crate README. Qualify HTTPS enrollment,
current membership and grant revocation, native lost-reply recovery, and
candidate changes on that origin. Review resident auto-start and publication
policy separately. Synthetic checks enroll only an isolated fixture device;
they activate no owner's executor, publication, wallet, or commercial lane.

## Qualify project and operator Cloud web views (#10952)

Configure explicit protected resident project-observer and Cloud-operator
policies for the selected native device and workspace. Approve source commits,
pools, executor/model policy, credential-name allowlists, and server custody
before using real providers. Qualify HTTPS policy removal, source changes,
revocation, duplicate confirmation, original-job recovery, cancellation, and
cleanup on the deployed origin. Scratch provider fixtures establish no real
capacity, retail authority, delivery, publication, or commercial activation.

## REV-50 enterprise sign-in: Google Workspace OIDC (#10857)

Code state: `tenancy::accounts::sso` admits one OpenID Connect provider per
organization workspace (issuer, `hd` tenant, audience, reviewed RSA keys,
linking rule, reviewed audit fields). The gateway's
`/v1/workspaces/{ws}/sso*` routes record terms (owner), link subjects
(admin), sign in with an ID token (RS256 against the reviewed keys only,
no JWKS fetch), and export the workspace's audit rows. Forged, expired,
replayed, cross-tenant, and unlinked tokens refuse; removed members and
revoked sessions stay refused; a provider outage refuses sign-in and
grants nothing. Break glass is the owner's existing `oak_` key sign-in.

Owner steps before a real tenant uses it:

1. Create the Google Cloud OAuth client for the tenant and record its
   client id as `audience`, `https://accounts.google.com` as `issuer`, and
   the Workspace domain as `tenant`.
2. Copy the current Google signing keys from
   `https://www.googleapis.com/oauth2/v3/certs` into `terms.keys` and
   re-record the terms when Google rotates them (the book never fetches).
3. Name the reviewer account and the audit fields the customer requires.
4. Link each member's Google subject, or set `linking` to
   `verified_email_domain` and label accounts with their work email.

## REV-76 second jurisdiction: Canada business email under CASL (#10883)

Code state: international contact stays disabled by default. A lead outside
`US` is contacted only with positive evidence (`details.scope`) that names an
enabled, versioned, reviewed scope (`openagents.sales-jurisdiction-scope.v1`,
recorded through `sales privacy apply` with `kind: scope`, revoked with
`kind: revoke_scope`) whose jurisdiction, business category, email channel,
consent basis, and rule version match exactly; admission pins the evidence
digest, dispatch rechecks scope, revocation, expiry, and suppression, and a
restart cannot restore a revoked or expired scope. The reviewed Canadian
scope admits business recipients on express consent or an existing business
relationship only, never a published address.

Owner steps before the scope is enabled:

1. Confirm the buyer cohort that needs Canada and record it as
   `cohort_reference`.
2. Obtain the CASL review (CRTC consent guidance, identification, and
   unsubscribe requirements) from the named reviewer and record its
   reference and expiry in the scope.
3. Add `CA` to the sales policy's `jurisdictions` after `US`.
4. Record each Canadian lead's consent or relationship evidence with its
   recorded date, expiry, and reviewer before any draft.

## REV-75: consented booked voice participation (#10882)

Voice stays disabled until the owner records a `sales voice` authority. The
code admits only web meetings the recipient requested, over an accepted
meeting, under the accepting human's start, mute, takeover, and end controls;
it builds no outbound call. Before activation:

- Confirm written selling has proven useful and keep that evidence reference.
- Obtain a legal review of AI voice participation in web meetings for the
  actual medium (telephone is refused in code; FCC 24-17 covers AI telephone
  voices) and record its reference and reviewer.
- Name the qualified human supervisor and brief them on the controls.
- Leave recording off, or record a separate consent reference, recipient
  list, and retention before enabling it.
- Wire the chosen meeting medium's native audio glue to `voice_turn`; the
  session stores speech and transcripts as untrusted data only.
## Qualify granted Cloud browser terminals (#10953)

Provision an explicit HTTPS account binding, secure native relay/direct routes,
and a separately issued browser host invitation before enabling real terminals.
Review the native host-wide Terminal right and independent Observe right for
retained thread reads; a selected account workspace does not narrow those
rights. Qualify revocation, grant expiry, host restart, hidden-page cleanup,
fresh enrollment after route loss, and PTY survival after detach on the deployed
origin. Check physical IME, gesture clipboard denial, keyboard accessories,
glyph coverage, and WebGPU/WebGL2 behavior on supported devices. Scratch hosts,
synthetic grants, and retained fixture threads activate no owner's executor,
retail shell, commercial lane, or wallet.

## Everglade destruction: 20 destroyed buildings on desktop (#10941)

The code freezes evicted buildings and turns timed regrowth off in
`dev-destruction` builds (PR #10989); the desktop checks need a real GPU:

1. Run the desktop Verse build with `--features dev-destruction` at High
   quality and destroy 20 buildings in a row. All 20 stay destroyed.
2. Walk 100 m away and wait 2 minutes. Nothing regrows. Press `R`; the
   town is whole again.
3. Record the frame time before and after the 20 destroyed buildings
   (the raised caps are 24 buildings and 1500 pieces). If the frame time
   drops under 60 fps, open an issue with both numbers.

## Water W11 physical devices (#10783)

On the supported iPhone and Android devices, record the device, OS, build,
quality tier, drawing-buffer size, and water residency. Use Water Lab and
Everglade pond views above water, at the waterline, and underwater; include
rain and the storm sea. Retain frame timing and thermal behavior after warmup.
Verify that sustained overruns reduce optional optics or update frequency
while water, swimming, buoyancy, and underwater visibility remain correct.
Browser elapsed time is not thread CPU time; unavailable GPU timestamps
remain unknown. Desktop and browser measurements do not qualify phone
performance. Open a new issue for any device defect.

## Rain occlusion cost per tier (#10939)

The rain occlusion map (a straight-down depth pass, 256² over 64 m on Low,
256² over 80 m on Medium, 512² over 96 m on High) is pass 4 in the water
timer. Record its GPU cost per tier against the W11 budgets (#10783) on a
real GPU: run `cargo run --release -p verse --example everglade_weather_capture`
and read `pass_ms[4]` from the timing output. This box has only lavapipe,
whose numbers say nothing about hardware.
