# Owner actions

## GPU device loss and trap recovery: check on real GPUs (#12651, 2026-11-02)

`coax/web/host.js` now replaces a lost GPU device and keeps the realm's
state, and saves a realm whose engine traps so the reload puts it back. The
check, `coax/web/gpu-recovery.coax`, passed on coderos-4080 against the Grid
only on SwiftShader (headless Chromium's GPU process still dies there, see
"Coax Nostr queue" below), so the 3-second budget is not yet measured on a
real GPU. On the Mac, with a served build (`coax/web/serve.sh
apps/openagents-web`), run `coax/web/tool.sh coax/web/gpu-recovery.coax
http://127.0.0.1:8800/realms/the-grid/` and then the same against Everglade
and the fixture realm, and add the results to issue #12651. Not blocking.

## Realm seats show in made realms: release to staging, then production (2026-11-02, #12655)

Slice 4 of `docs/verse/realm-seat.md` is in `main`: migration
`0108_realm_seat_presence.sql`, `coax/web/api/room.c` (a signed presence is
an agent's seat; a private realm's room is 404 to others), the made-realm page
(`myrealm-room.coax`: who else is here, an agent's seat tagged AGENT) and the
CLI holder posting the seat's presence. Nothing is deployed. To see it live,
release to staging, then run `openagents realm play village-green` with a
profile that has a key while a browser has the realm open: the seat shows as
a bronze figure with the account's name and AGENT. Production after that.
`realm use build THING` still needs a staging realm with BUILD on; no listed
staging realm has it, and switching one on is an edit to someone's realm.

## Verse OS V0: the Verse on tty3 of coderos-4080 (2026-11-02)

The boot session is built (docs/os/2026-11-02-verse-os.md, "Status"):
`os/verse-session.coax`, `vk-realm --leave-to`, `verse/gpu/realm.sh local
install`, and the NixOS module `os/nix/modules/verse-os.nix`. Turning it on
changes the machine's configuration, so it waits for you:

1. Build the realms (no screen change): `verse/gpu/realm.sh local install
   ~/.local/share/verse-os/current` from an openagents checkout.
2. In `~/coder/os`, add the input `verse-os.url =
   "git+file:///home/christopherdavid/openagents?dir=os/nix"`, import
   `verse-os.nixosModules.verse-os` in `hosts/coderos-4080.nix`, and set
   `verseos.session = { enable = true; user = "christopherdavid"; tty = 3;
   dir = "/home/christopherdavid/.local/share/verse-os/current"; stateDir =
   "/home/christopherdavid/.local/state/verse"; };`. Build, compare with
   `nix store diff-closures`, switch. The module adds you to `input`, so log
   in again once (or reboot) before the session can read the keyboard.
3. At the desk: Ctrl+Alt+F3. The Grid fills the screen. Escape held a second
   returns to tty1; Ctrl+Alt+F3 resumes. Say whether it should become tty1
   (booting straight into the Verse), which is V0's last step.

## Background jobs stopped after desktop lockups (2026-11-02)

At the owner's request, the open-issues chat stopped its eight mutation
workers, an 18-worker compiler mutation that had run for over two hours,
and overlapping bootstrap/render builds. It temporarily unloaded these user
LaunchAgents to prevent automatic restart; their plist files were not edited:

- `com.openagents.coax-publish`
- `com.openagents.disk-janitor`
- `com.openagents.spotlight-skip-targets`

CPU recovered to about 66% idle with 52 GB unused RAM. Our remaining local
checks run one at a time, at low priority, with one mutation worker and a
15-minute command limit. Interrupted runs do not count as passing evidence.

Before resuming the background jobs, bound their concurrency and runtime so
they cannot accumulate overlapping work again. To restore a service after
that review, run `launchctl bootstrap gui/$(id -u)
~/Library/LaunchAgents/LABEL.plist`, replacing `LABEL` with the names above.
The compiler publisher's release work remains incomplete while it is stopped.

## Psionic chapter 74: run gpt-oss-120b on the Mac (2026-11-02)

The chapter 74 planner (`psionic/lib/gptoss/device-choice.coax`) admits 120b
resident on the 128 GB M5 Max (65,574,023,296 bytes at a 4,096-token cache,
within the 107.5 GiB working set) and picks it over the RTX 4080 (24.6 vs a
worst-case 9.5 tokens/s). No agent has documented access to the Mac from
coderos-4080. Once chapter 72's Metal harness takes the 120b sizes, run it on
the Mac with the original 120b shards and keep the log. The run needs
allocation headroom, logits and routes against chapter 73's references,
equal greedy IDs, and decode and prefill rates. Separately, decide whether
coderos-4080 should get the 65.2 GB of 120b shards for the 4080
expert-streaming path. Its disk has about 35 GB free.

## Psionic chapter 196: working with Mesh-LLM (2026-11-01)

`psionic/lib/peer/meshllm.coax` maps Mesh-LLM peers to capability claims and
prices their usage by Mesh-LLM's bill rules, against a simulated peer in
process only. Before any live interop: decide whether to join a Mesh-LLM mesh
(a private test mesh first, or a public one), whether to pay Mesh-LLM peers
(their 1 sat deposit and final invoices) through our wallet, and whether to
accept payment from Mesh-LLM buyers.

## Psionic chapter 238: paying verified work (2026-11-01)

`psionic/lib/peer/pay.coax` now emits payment intents (msat, keyed by the
receipt digest) for work chapter 195's checker verified; it pays nothing.
Before any real payment: choose the period cap and the payout cap, decide
whether unsampled work is trusted, and decide whether intents may be executed
through the Breez Spark wallet. Each job held for review (unverifiable, not
sampled, or above the payout cap) also waits for your pay or withhold.

## ARWES Coax HUD extension: visual review (2026-11-01)

At `http://127.0.0.1:8815/`, press **6** for the ARWES collection. Its four
groups cover nine frames, motion/text, four backgrounds plus pointer lighting,
and styles/theme/silent cues. Click a card for its enlarged inspector. Use
REPLAY and ENTER/EXIT to compare transitions; ASSEMBLY/FAMILY selects the
shared or per-family frame timelines; EASING and MANAGER cycle the
motion models; RESEED and DOT TYPE change the fields; CATEGORY/CUE/LOOP inspect
the silent cue model. M retains the monochrome/cyan comparison.

Review the new primitives alongside the existing Grid scenes before choosing
which ones should become production defaults. Source, license and exact port
boundaries: `docs/verse/2026-11-01-arwes-coax.md` and
`aethyr/ui/arwes/SOURCES.md`. The full upstream MIT notice is retained.

## Aethyr HUD component library: visual review (2026-11-01)

Review the running local demo at `http://127.0.0.1:8815/` (the isolated
`aethyr-hud-library` worktree). Keys 1–5 choose the component library, Grid,
flight, systems and communications; M toggles monochrome/cyan, Space freezes
motion, Tab/Enter operate controls. Click a component to inspect it; Escape
closes it. Scroll for all 18 components and the stacked phone scenes.

Assess the monochrome hierarchy, type size, instrument density and how the
four compositions extend the current Grid. Contacts, telemetry and events are
synthetic demo fixtures. This review does not require a payment, sign-in or
production deployment. Research and reuse contract:
`docs/verse/2026-11-01-hud-library.md`; restart instructions:
`apps/openagents-web/demos/hud/README.md`.

## Nostr book follow-ups that need the owner (2026-11-01)

Details: `docs/verification/2026-11-01-nostr-book-chapters-11-104.md`.

1. **x402 settlement expiry (money path).** The live route settles in SQL
   (`inference.topup_settle`) with no invoice expiry check at settlement.
   `coax/lib/x402/settle.coax` (`#authority`, zero survivors) has the check
   but isn't wired in. Decide whether to wire it in or add the check in SQL.
2. **Plan rewrites.** Confirm the rewritten "Done means" lines for chapters
   101 (offline presence, with an assumed latency) and 102 (offline agent
   path; the move of the hosted runner to staging isn't built).
3. **NIP-KEYS pin.** Review `nostr/nips/openagents/NIP-KEYS.md` (from
   `b01de252f7`), then re-pin the OpenAgents NIP folder digest in
   `coax/tests/nostr/profiles.py` (now `fb7563e7…`, pinned `2fb1f51f…`).
   Its pin test fails until then.
4. **May the Coax relay hold a signing key?** NIP-43 (13534, 8000, 8001)
   and NIP-29's relay-signed group state (39000 to 39003) need the relay
   to sign with its own key, loaded from a file. This is the same question
   as "Real Pylon keys: your sign-off" below: the signer audit is not
   approval to hold a real key, and safe key-file reading isn't built.
   Until you decide, the relay has no key, signs nothing, and refuses
   every 39000 to 39005 event. Say yes or no here; nothing is built until
   then.

## Nostr chapter 103: export, shadow period, cutover and rollback rehearsal for relay.openagents.com (2026-11-01)

The importer (`coax/lib/nostr/relay_import.coax`, `relay_import_run.coax`),
the event-set equality check (`relay_equal.coax`) and the shadow comparer
(`relay_shadow.coax`) are built and checked offline; nothing touched
production. The runbook is in `nostr/book/ch103-replace-relay-openagents-com.md`
("The runbook"). These steps need your sign-off, in order:

1. **Export snapshot.** Approve restoring the newest `nostr-relay` backup unit
   into a throwaway Postgres off the production host and exporting
   `nostr_event` read-only, in `ingest_seq` order, as JSON lines (the query is
   in the chapter). Production's database is never read directly.
2. **Store capacity and required wiring first.** `--store-capacity` now sizes
   host-allocated slots and bytes; 2,048 events/4 MiB are defaults, not a hard
   production ceiling (`6f4c722640`). Qualify the chosen limits against the
   restored export, tombstones, memory, persistence and restart behavior before
   treating an import as complete. Index/delete/rate/COUNT/search/protection
   and optional proof-of-work wiring have landed. Decide which remaining
   advertised features must be wired before cutover: NIP-77, NIP-86, NIP-43
   and NIP-29 (the chapter tracks the current table).
3. **Shadow period.** Approve a proxy that sends every write to both relays
   (clients get the Rust relay's answers) and the replay of readers' REQs to
   both, and set its length (the runbook suggests at least a week).
4. **Rollback rehearsal.** Approve rehearsing on a staging name: point it at
   the Coax relay, then back at the Rust relay, checking reads and writes each
   time.
5. **DNS cutover.** Only after the final equality check is equal, the shadow
   period has no unexplained answer, and you sign off here: point
   relay.openagents.com at the Coax relay, keeping the Rust relay read-only as
   the fallback.

Any refused line in the import (a forged or malformed event in the export)
also comes to you before the cutover continues.

## Psionic chapter 59: book search on openagents.com (2026-11-01)

Chapter 59's browser search runs locally (`psionic/web/book-search/check.sh`:
Wasm in headless Chrome, results equal bit for bit to the native tool on the
36 held-out queries, stale and wrong-encoder bundles refused). Putting it on
openagents.com is a production deploy, which agents don't run. To ship it:
decide the URL (for example `/search`), add the page and its three bundle
files (`coax/web/build/book-search-web/`: `search.wasm`, `bundle/manifest.txt`,
`bundle/encoder.bin`, `bundle/index.bin`, about 8.6 MB uncompressed) to the
site's release, and rebuild the bundle on every deploy so the manifest names
the books being served. Until then the chapter stays partly implemented.

## Psionic chapter 177: machines for a live multi-replica run (2026-11-01)

The router, bounded queues and autoscaler (`psionic/lib/serve-route.coax`,
`#authority`) run only in a simulation of replicas in one process; nothing
was provisioned. The chapter's last "Done means" item, a live run that
reproduces the simulated percentiles within a stated tolerance, needs your
choice of hosts (two or more replicas of one qualified model), the replica
budget the autoscaler may spend up to, and the spending limit for the run.

## Psionic chapter 34: approve a packaged llama.cpp build (2026-11-01)

Chapter 34 compares Psionic with llama.cpp only through an owner-approved
packaged build, never the clone in `~/code/llama.cpp`. nixpkgs has
`llama-cpp` version 9190 (CPU build). A CUDA build needs the unfree CUDA
packages (`cudaSupport = true`) and a source build that runs longer than the
15-minute command limit. To approve one, add it to `os/hosts/coderos-4080.nix`
in `~/coder` (or name the exact nixpkgs attribute to use ephemerally) and say
so here. The Psionic side of a same-file comparison, a GPU engine that runs a
whole GGUF file, is not built yet either.

## Coax 0.46 publication: local cloud reauthentication (2026-10-31)

Run `gcloud auth login` locally on this Mac, then confirm it finished. Do not
send credentials in chat or commit them. The installed CLI has an active
account, but token acquisition exits 1 with reauthentication required. The
read-only publication preflight stopped before its first GCS request; no
staging or production bytes changed. Compiler verification and the remaining
ordered language work continue independently.

Diagnostic receipt: `/tmp/coax-return-borrows-A1/publication-v2-credential-diagnostic/diagnostic.json`,
SHA-256 `674f1704ea8cd4aa0d09a1c6e5f13949ffd42cfd9325a3cfc28f351a66c87a9f`.

Resolved: the owner confirmed local reauthentication. A fresh GET-only
preflight completed 17 requests with HTTP 200 across production and staging,
including a generation-pinned release marker whose size and MD5 match its
metadata. No credentials, headers or authentication error bodies were
retained, and no site bytes changed. The cloud preflight is restored.
Publication will use the compiler and site package after their required checks
pass.

Receipt: `/tmp/coax-return-borrows-A1/publication-v2-readonly-api-preflight-after-reauth/observation.json`,
SHA-256 `db0ecdf266d4a76218a7d93bb7a795aa83ce27329dfd1794c7ea0dfcdbad5582`.

## GR00T chapter168: gated backbone tokenizer/processor (2026-10-31)

The selected robotics artifact is NVIDIA
`GR00T-N1.7-LIBERO@2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/libero_10`.
Its public weights/config do not include the required Cosmos-Reason2-2B
tokenizer, backbone configuration and processor. The official dependency's
configuration GET returns HTTP401; no Hugging Face token environment or normal
local token file is present. Source and checkpoint metadata have been read;
no third-party repository scripts were executed.

Please request/accept access at
[NVIDIA Cosmos-Reason2-2B](https://huggingface.co/nvidia/Cosmos-Reason2-2B)
and sign in locally using an approved account. Do not put a token in chat or
this repository. If approved credentials already exist elsewhere locally,
provide their location without their contents. Exact pinned asset hashes will
be recorded after authorized retrieval. This blocks full text/image processor
and original VLA parity; original-weight numerical components can proceed.

Update: the owner accepted the model terms. A subsequent configuration GET
still returned HTTP 401 and no local token was present. The installed
`hf` command is available: run `hf auth login` locally using that approved
account. Account terms acceptance and the local CLI credential are separate.

Resolved: the owner accepted terms and signed in locally. Authorized retrieval
now succeeds for pinned Cosmos revision
`9ce19a195e423419c349abfc86fd07178b230561`; tokenizer/image processor assets
are available in the normal Hugging Face cache. No credential was printed or
committed. Original learned-network qualification is recorded in chapter 169; simulator integration remains separate.

## Made-realm golden after the incoming renderer change (2026-10-31)

Resolved visual review: the original author reviewed and updated the `built`
golden in main commit `21dc5da9ca` ("looked at"). No further owner sign-off is
pending. The targeted check against that approved image passed (0 changed blocks)
in `/tmp/coax-native-baseline-approved-built-golden.log`; this audit did not
approve or change a snapshot.

The earlier check of incoming `59347994ba` drew without errors but differed
from its old golden by 4,127 blocks (6 allowed). Its failed result remains
historical evidence: `/tmp/coax-library-final-parent-built-golden.log` and
`/tmp/coax-golden-bRJX2B/built.png`. It is not reported as a pass.

## Coax header composition: existing visual baseline failures (2026-10-30)

The `r1a-header-composition` issue (`#9bdc`) has a prepared compiler fix:
15 tooling tests, six real iOS Bridge checks, the full bootstrap and three
affected Wasm builds pass. Before/after Wasm binaries also match exactly for
every application behind the eight pages flagged by the broader golden walk.

The mandatory `coax/web/golden-check.sh --commit
e6e6acabe75b6fbe191ee5fa0a325b9f718f2d74` still failed nine image checks.
Confirmed causes include Haunt/Sandbox goldens predating the existing GALAXY
button, live staging planet data, and Fairy Forest's unfrozen wing phase.
No goldens were changed or failures reported as passing.

Resolved 2026-10-30: the owner explicitly chose "Record baseline failures
and continue" in this chat. The header fix may be pushed and the ordered
compiler work may continue with these demonstrated baseline-only failures
recorded separately. No golden snapshot was approved or changed, and the
failed global walk is not reported as passing. This resolves the header-fix
sign-off only; it does not waive new regressions or the later golden repairs.

## R0 adoption gate

The benchmark/source-debugging implementation is usable. Closing R0 still
needs a representative physical iPhone run, agreement on device median/frame
tail budgets, and an independent review of the frozen native-reference f32
reader/full-body-mask adaptation and tolerance. The updated Mac pose comparison
passes the unchanged proposed 5% median gate in all three profiles, with raw
tails/outliers retained. That is measured median evidence, not agreement on
device/frame-tail budgets or an independent adoption sign-off.

Independent invariant/oracle review and its enforcement on owned
infrastructure remain separate roadmap work. This entry records those future
owner/reviewer decisions; it does not block building or using this slice.

## Coax Nostr queue: site releases from the Linux worker

`claude-nostr` signs queue events from the Linux host, but it cannot run
`coax/web/release.sh` there. The page build uses the Mac's `/opt/homebrew`
Wasm toolchain, and the release goldens are Mac renders. The relay is current;
only the page lags. Either run `coax/web/release.sh` from the Mac now and
then, or decide whether the Linux host may release. That would need Linux
goldens or a policy for them, and `COAXIAL_*` set to
`~/.cache/coax-wasm-toolchain`.


Update 2026-10-29 (claude-nip01): the Linux build now works
(`coax/web/toolchain.sh`, `serve.sh`, `chrome.ts`, `goldens/linux/`), and
`coderos.desktop.browser` is switched on for coderos-4080 (the grant file is
there). Headless WebGPU is the remaining blocker. Chromium 152 finds the
RTX 4080 (`--enable-features=Vulkan --use-angle=vulkan --ignore-gpu-blocklist
--disable-vulkan-surface` reports an nvidia/lovelace adapter), but its GPU
process dies once a page configures a canvas ("Failed to send
GpuControl.CreateCommandBuffer"). With ANGLE left at its default it falls
back to SwiftShader, which draws but captures blank. Until that's solved,
release from the Mac. The switch also started `coder-claim.service`, which
fails because `~/coder/target/release/coder-runner` isn't built.

## GPU profiling counters for Psionic CUDA work

Nsight Compute (`~/cuda-13.2/bin/ncu`) refuses this user with
`ERR_NVGPUCTRPERM`: the NVIDIA driver restricts GPU performance counters to
administrators. Kernel tuning for Psionic's CUDA chapters (37 and 44) works
without them, from event timings, but per-kernel occupancy, memory and
instruction counters need them. Allowing them is a system change: set the
`nvidia` module option `NVreg_RestrictProfilingToAdminUsers=0` (on NixOS,
`boot.extraModprobeConfig = "options nvidia NVreg_RestrictProfilingToAdminUsers=0";`)
and reload the driver or reboot. Decide whether to allow it; nothing is
blocked if not.

## Review Claude's first blog post

`docs/blog/2026-10-29-attention-that-cannot-miss.md` is a draft post by Claude
about Psionic chapters 226 and 227 (exact memory by attention, and traces as
receipts). Read it and approve, edit or reject it. Publishing it needs a blog
route on openagents.com; the steps are in `docs/blog/README.md`.

## Psionic chapter 24: the Pylon's key, relays and TLS (2026-10-29)

Chapter 19 runs a Pylon that answers Elephant decisions over Nostr
(psionic/book/PLAN.md, "Before chapter 24"). The code is being built now;
these four choices are yours:

1. ~~**Where the Pylon key lives.**~~ Decided 2026-10-29: a 0600 file in
   `~/work/.secrets/` on the owner's Mac, generated once and never printed.
2. ~~**Your `auth` signature.**~~ Delegated 2026-10-30: the agent producing
   the real Pylon key signs the beacon's `auth` tag with the owner key when it
   is needed, after the #4828 audit and review gate clear signing for real
   keys. Neither key is printed or logged.
3. ~~**Which relays.**~~ Decided 2026-10-30: relay.openagents.com (the current
   Rust relay) and every Coax relay we run (coax/nostr.md section 17) as it
   comes up. Both go in the Pylon's NIP-65 list.
4. ~~**TLS for `wss://`.**~~ Decided 2026-10-29: the C host may link a TLS
   library (AGENTS.md, Rules). Linux and Apple are now implemented; macOS
   TLS/relay operation and iOS compilation are checked in the
   [Apple TLS record](docs/verification/2026-10-30-apple-tls.md). The Pylon's
   reusable connection passes its macOS wss end-to-end check as well.

Signing with a real key also waits on the #4828 secret audit and the review
gate in coax/nostr.md.

## Coax relay: which NIPs survive, and the cutover (2026-10-29)

coax/nostr.md section 17 plans a Coax relay to replace relay.openagents.com
(the Rust relay, removed from the tree at the Nuke). Phase R1 runs only on the
tailnet. Left for you: approval of the data migration and the DNS cutover (R3). Nothing touches the production relay before that.

Decided 2026-10-29: the old Rust relay may be built and run inside a sandbox
for the comparison; the Coax relay aims for full NIP support, starting with
what the website uses. Still yours: the R3 migration and DNS cutover.

Open 2026-10-30: **a rollback-resistant checkpoint for the relay's store
(#966b).** Crash recovery is done (`092c3e44a4`,
[record](docs/verification/2026-10-30-nostr-store-recovery.md)). A torn tail
is cut, and any other damage stops the relay with the file unchanged. But an
older copy of the whole file, or a deleted file, still replays as a valid
older or empty store. Closing that needs a checkpoint kept outside the file
that can't be rolled back: for example a TPM monotonic counter, a signed
checkpoint published to a second machine, or a cloud object with versioning
and a retention lock. Which provider to use is your call. It doesn't block R1
on the tailnet.

Two questions from the sandboxed Rust differential (2026-10-30,
[record](docs/verification/2026-10-30-coax-relay-r1-rust-differential.md)).
Each answer is yes or no:

1. **Kind-specific structure checks.** The Rust relay refuses NIP-34 issues
   without a `30617` `a` tag and NIP-22 comments on an event without a `P` tag
   ("invalid: invalid event: ..."). The Coax relay checks only NIP-01 (id,
   signature, time, tags, size) and stores both. The website's own events pass
   both checks. Should R1 copy the Rust relay's per-kind checks?
2. **Multi-filter REQs.** The Rust relay merges every filter's results into
   one list, newest first, and shares 127 events across the filters (127/n
   each, rounded up). The Coax relay answers one filter after another, and
   each filter has its own `limit` of at most 127. A single-filter REQ (all
   five on the project page) is the same on both. Should the Coax relay copy
   the Rust relay's merged order and shared budget?

## Psionic Part III: public ingress and a sustained serving window (2026-10-29)

The original Elephant service was installed and checked privately on
`coderos-4080`, `127.0.0.1:8798`, as the user unit
`psionic-systemone.service`. Readiness, the retained golden answer, SIGKILL
recovery with re-warm, and clean stop passed. It is now stopped, installed
without boot enablement, so it does not reserve the shared card between uses.
Start it with `systemctl --user start psionic-systemone.service` for a serving
window. Its versioned artifacts and tokenizer persist in the user's data
directory; its private usage journal lives in the user's state directory.

For continuous or public serving, choose the host/GPU allocation, memory and
spending budget, domain, TLS ingress and access policy. This work grants no
public listener, DNS cutover or billing. The existing GPT-OSS service was left
running; idle Elephant instances were stopped only under the owner's earlier
authorization. Evidence: [Part III verification](docs/verification/psionic-partIII-2026-10-29.md).

## Real Pylon keys: your sign-off (2026-10-30)

The Coax signer has passed the #4828 secret-source audit:
- `docs/verification/2026-10-30-signer-source-audit.md`;
- `docs/issue4828.md`;
- final mutation gates clean at `6c6dab1054`.

Passing the audit is **not** approval for production keys. The Pylon refuses `--key-file` (`psionic/lib/pylon/run.coax:167-168`) until you approve here. Two things are needed:

1. **Your decision:** may the Coax signer hold real Pylon keys? The stated limits:
   - the audit covers the signer's source, not host copies of the key or memory wiping;
   - target probes cover release builds only, and checked builds rely on the source proof;
   - there is no hardware timing evidence and no iOS run.
2. **After a yes:** new code that reads a key file safely (an owned buffer, wiped after use, never logged). Then the real Pylon key is created in a `0600` file in `~/work/.secrets/` on your Mac, and your `auth` signature is made over it (already decided above).

Until then, demo Pylons use fresh random throwaway keys (`docs/Oct30MVP.md`, §3).

## B1 qualification: task time-limit decision — resolved (2026-11-01)

The resumed B1 task began at 06:38:49 UTC; the AGENTS.md two-hour limit ends
at 08:38:49 UTC. The analysis cost is fixed within unchanged limits, but the
first full checker mutation shard found 29 survivors. The async question asks
whether to continue beyond the two-hour limit until B1 is fully qualified,
pushed and closed, or stop at the limit and record the remaining work.
No extension is assumed while the owner decision is pending. No credentials
or local UI action are needed.

Resolved: the owner stated that the two-hour task limit was never approved
and directed its removal on 2026-11-01. Commit `099d3575b0` removes it from
`AGENTS.md`. B1 implementation and qualification continue, followed by the
next Coax book chapter. No owner action is pending for this item.
