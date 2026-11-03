# Owner actions

## Verse OS: optional visible WoW trial (#12724–#12727, 2026-11-03)

- [ ] Try the installed Verse window using [the gaming commands](docs/os/verse-gaming.md).
      Battle.net, entering Classic Era, bounded movement, camera turns and
      cancellation passed programmatically in a private headless session.
      This optional play-through checks the visible desktop experience and
      sustained performance. The default desktop/session has not changed.

## The agent runner: redeploy (2026-11-03)

Redeploy the agent runner to pick up file-tool hardening
(`coax/web/agent/deploy.sh`, staging first). Until then the running image
has the old engines.

## Your own Coder builds your realm: the first real try (#12640, 2026-11-03)

Checked locally with test keys, a local relay and a stand-in `claude`.
Your Claude or Codex login and relay.openagents.com were not used. Try it
for real when you can. It takes about five minutes:

1. On your Mac, from an up-to-date checkout: `coder/tui/coder.sh host`
   (the first run builds for about 90 seconds). It prints your computer's
   key as an `npub` and connects to wss://relay.openagents.com. Leave it
   running.
2. In Chrome, open one of your made realms on openagents.com after the next
   release. Open EDIT > COMPUTER, paste the `npub` and press PAIR.
3. In the terminal, `coder host list` shows the browser waiting. Allow it
   with `coder host allow ID`, using the ID that list prints.
4. In the pane, type a request ("add a crate by the pines") and SEND. The
   job runs on your Claude Code login with read access and the realm tools
   only. Its steps stream in the pane, and the realm reloads by itself when
   the edit lands. A refused shell command or file write shows as a step.
5. Send a longer request and press STOP while it runs. Then press FORGET
   THIS COMPUTER, or run `coder host forget ID`.

Say whether each step did what it says. If one didn't, say which and what
you saw. The job needs the openagents CLI built (`apps/openagents-cli/build.sh`)
for the realm tools, and your CLI login (`openagents login`) for the realm.

## Coax relay cutover: parity, the checkpoint's home, the export and the rehearsals (#12631, 2026-11-03)

Built and checked offline, with test keys and synthetic data only: the
relay's `--import` (every line verified again, deletions and vanish
applied, one sync, a freshness checkpoint written last), `--check-store`
(a restart that checks the checkpoint, replays and prints the served set's
digest), and `--checkpoint FILE` (the store's and policy journal's length
and SHA-256, checked on start, rewritten after every commit). Parity table:
`docs/verification/2026-11-03-coax-relay-parity.md`. Runbook:
`nostr/book/ch103-replace-relay-openagents-com.md`. Nothing here touched
relay.openagents.com, its database, DNS or a secret. Yours, each yes or no
unless it says otherwise:

1. **Private reads: ported whole (2026-11-03), three questions left.** The
   Coax relay now serves gift wraps and OpenAgents' private kinds exactly as
   the Rust relay does (`coax/lib/nostr/relay_private.coax`; all 33
   private-read observations in the Rust differential match). Yours:
   a. **Private and hidden NIP-29 groups** are not ported: the Rust relay
      reads `relay_group` and `relay_group_member`, which the Coax relay
      doesn't hold without a relay key. If production has any private or
      hidden group (`SELECT count(*) FROM relay_group WHERE private OR
      hidden` on the restored copy), its events would be served to anyone,
      so this blocks cutover until you decide it with the relay key
      question.
   b. **The Rust relay's stored and live rules disagree**, and the Coax relay
      copies both as they are: a gift wrap needs one distinct `p` value in
      history but exactly one `p` tag live; kinds 24200, 30622 and 44200
      need one distinct `p` in history but only the first `p` live; 30175
      and 30178 count any tag holding "shared" and "true" in history but
      only exactly `["shared","true"]` live; 39604 to 39613, 39620 and 39650
      are never served from history but are delivered live. Keep them, or
      align them (say which way)?
   c. **Writes of the private kinds** keep the Coax relay's rules: the Rust
      relay also refuses kind 78 and 30078 from a connection not
      authenticated as their author, the relay-only Block kinds, malformed
      run, artifact and private-policy envelopes, and a 3188 whose content
      isn't NIP-44 v2 or whose `p` isn't a point. Port those write checks
      too? Reads never depend on them.
2. **Advertisement.** Rust advertises 17, 59, 65, 67, 94 and 98, which the
   Coax relay doesn't. For each: wire it before cutover, or drop it from
   the advertisement. Nothing was changed.
3. **Where the checkpoint lives.** `--checkpoint` refuses an older copy, a
   forked copy or a deleted store, but only if the checkpoint itself isn't
   restored with them. Pick its home: a second machine, a TPM counter, or a
   versioned object with a retention lock (replaces the 2026-10-30 question
   under "Coax relay: which NIPs survive").
4. **What an event export can't carry.** Rust tombstones whose deletion
   event NIP-40 already removed, and the NIP-86 policy tables
   (`relay_blocked_*`, `relay_allowed_*`, `relay_member_pubkey`), have no
   signature to verify again. Carry them (event tombstones as NIP-86 bans in
   `FILE.nip86`; address tombstones have no carrier yet), or drop them.
   Count them on the restored copy first (runbook step 6).
5. **Export a restored backup (runbook steps 4 to 7).** Approve restoring the
   newest backup unit into a throwaway Postgres off the production host and
   exporting it, then `relay --import` with `--checkpoint`. Every refused
   line, and every kind 62 the import applies, comes back to you.
6. **Shadow period (steps 10 to 12).** Approve the proxy that sends every
   write to both relays and replays readers' REQs, and set its length.
7. **Rollback rehearsal (steps 17 to 20).** Approve the rehearsal on a
   staging name, including the export of post-cutover writes back to the
   Rust relay.
8. **Cutover (steps 13 to 16).** A separate sign-off, only after 1 to 7.

## Dictation in Coder's terminal composer: a real microphone (#12668, 2026-11-03)

Coder's terminal dictation passes 14 recorded-audio cases in a headless pty,
including real Parakeet on the GPU
(`docs/verification/coder-terminal-dictation-2026-11-03/README.md`). No
microphone was opened. To accept it:

- [ ] Build both: `apps/openagents-cli/build.sh`, then start
      `coder/tui/coder.sh --dictation` in your own terminal.
- [ ] Type a few words, press **Ctrl+Space**, speak, press **Ctrl+Space**
      again. The words appear after the draft and nothing is sent. Press
      **Ctrl+Z**: they go away.
- [ ] Press Ctrl+Space, then Esc: "Dictation cancelled". Press Ctrl+Space,
      then Enter: cancelled, and the draft is not sent.
- [ ] If your terminal reports focus, switch away while listening: the
      request is cancelled. Note whether it did; Coder can't tell when a
      terminal doesn't report focus.
- [ ] If Ctrl+Space doesn't reach Coder in your terminal (some send nothing
      for it), try `--dictation-key ctrl-g` and note which terminal it was.
- [ ] On the Mac this also needs the `dictate-mac.m` build in the
      `openagents dictate` item below.
## Tensor IR kernels: requalify Metal on the Mac (#12698, 2026-11-03)

`tik_run` in `psionic/lib/tensor-ir-kernels.coax` and `registered_squared`
in `psionic/examples/operators/squared-relu.coax` changed so that no mutant
can survive: loops run to the rank instead of to 4 with a guard, matmul reads
rank-two addresses, and flags that the final non-finite test already caught
are gone. On Linux the outputs are bit-identical: the 12 graph cases match
word for word on CUDA (RTX 4080) and the CPU, in debug and release, plain and
ASan/UBSan; the registered-operator run on CUDA prints the same weight and
loss; and 4,000 random programs with infinities, NaNs and the largest floats
give identical words, flags and outputs from the old and new kernels.

Metal was not run. The release Metal emission is now
`3c74be010b0f4fd74e1f092e5bc52220d2a662b0e4d46d37dba99088bf228677`
(squared-relu: `8af2b3f1bfb91ddd8dd66237d16e5dce515f09d91bdd3b5f9cf642d5d12ffc14`).
Metal compiles with fast math by default, so the non-finite cases matter
most there. On the Mac, run:

    psionic/tests/check_tensor_ir.sh --metal
    psionic/tests/check_operator_registry.sh --metal

Both should pass unchanged. If either fails, comment on #12698.

## Dictation vocabulary and code text in Coder for Mac (#12670, 2026-11-03)

`openagents dictate --terms FILE --format code` works on Linux. Its record
is `docs/verification/dictation-vocab-2026-11-03/README.md`. Coder for Mac
does not use either part yet, and its files were not changed. To adopt them:

1. Call `psionic_parakeet_vocabulary` once after the model opens. Pass the
   user's terms first, then `apps/shared/dictation-terms.tsv`.
2. Pass the final transcript through `dtx_draft`
   (`apps/shared/dictation-text.coax`) before the guarded insertion, behind a
   default-off "Code formatting" setting. Insertion, Undo and ticket checks
   stay as they are, and a refusal inserts the raw text.
3. Measure rescoring on the Mac. Its decoder runs on the CPU, so rescoring
   there uses the exact CPU rows and has not been timed.
4. Dictate a few sentences with this repository's terms on the Mac and record
   term recall. All measurements so far are synthetic speech.

## Psionic oracles: run check_decoder.sh and check_muon.sh on the Mac (#12695, 2026-11-03)

The decoder and muon oracles now use correctly rounded sin, cos, exp, log
and pow (`psionic/tests/cr_math.py`), and their references were rerecorded
from them on Linux, where both checks pass. On the Mac, run
`psionic/tests/check_decoder.sh` and `psionic/tests/check_muon.sh`. They
should pass unchanged: the oracles no longer touch libm, and the Coax side
compares within 1e-12. Both oracles need Python 3.12 or later. If either
fails, comment on #12695.

## MetaHuman packers: port them, or name a Firm reason (#12693, 2026-11-03)

`verse/tools/humans/metahuman.py` (453 lines) and `pack_human.py` (301)
have no `Firm reason:` line, so `coax/web/tool.sh tools-check` reports them.
None of the six reasons holds: they are packers with real logic (OBJ and
skin parsing, joint folding, A-pose to T-pose posing, vertex packing, and
JPEG and PNG layers through Pillow), not a thin wrapper or a Python-only
SDK. They can't go on `coax/tools-port.txt` either: tools-check refuses a
path that wasn't tracked at the list's base commit. Porting them means
porting what they import too (`verse/tools/people.py`'s glTF loader and
skeleton, `pack_realm.py`'s container) plus a JPEG encoder in Coax, which is
larger than a release-blocker fix. Decide: port them (and with them
`people.py`), or accept them as an exception some other way.
Since then, two more MetaHuman Lab tools without a `Firm reason:` line have
landed, and tools-check reports them too: `verse/tools/humans/face_rig.py`
(234 lines, 58a1c8741d, the face rig's tables and a Python evaluation after
OpenRigLogic's) and `pack_checks.py` (181, 1487a7c576, headless frame checks
of a pack). They belong in the same decision (noted by #12702).

## 5E MMO's hotbar art tool: port it, or name a Firm reason (#12702, 2026-11-03)

`verse/tools/fivemmo_hotbar.py` (209 lines, 088fd5f0af) is the same case and
tools-check reports it beside the MetaHuman packers. It draws 5E MMO's
hotbar icons, key tabs and cards and writes `fivemmo-hotbar.coax`. None of
the six reasons holds: it parses `aethyr/rules/srd/spells.coax` with
regular expressions, cuts the SRD text to whole sentences and writes a Coax
table, and it draws through `verse/tools/everglade_hotbar.py`. That file is
on `coax/tools-port.txt` and uses Pillow to set text in Paper Mono, tint and
composite, and encode JPEG, with `rsvg-convert` to rasterize the SVG icons.
It can't go on the port list (it wasn't tracked at the list's base commit).
A Coax port has to port `everglade_hotbar.py`'s drawing as well: text,
SVG coverage and a JPEG encoder. That is larger than #12702. Decide with the
MetaHuman packers: port the hotbar tools together, or allow them an
exception.

## Drop's Mac golden in golden mode (#12693, 2026-11-03)

Drop now holds its ember pulse, lamp flicker and star twinkle in golden
mode (`droprealm.coax`). The Linux golden matched three walks in a row with
0 blocks changed and 0 moving. On the Mac, an agent with goldens.ts should
walk Drop once (`bun coax/web/goldens.ts BASE --only drop`) and, if it
differs, refresh `coax/web/goldens/drop.png` with `--update`. Mac-only:
coderos-4080 can't run it.

## Ice Cream Man's camera on Aethyr's follow: approve the look (#12609, 2026-11-02)

Ice Cream Man's boom camera now runs on `aethyr/follow.coax`, one step per
rules step. The same bot round (`#seed=4&bot`) was drawn before and after,
frame for frame:
`docs/verification/data/icecream-boom-2026-11-02/side-by-side/` (before on
the left), with the full frames in `before/` and `after/` and what changed
in its `README.md`. In tight rooms and doorways the camera sits a little
further back, the kid trails slightly while walking, and the camera backs
out faster after a wall. Look at the pairs. If the look is right, refresh
`ice-cream-man.png` with `bun coax/web/goldens.ts BASE --update` against
staging. If not, say what to tune: distance, spring or ease.


## iOS Metal host onto the render admission ledger (#12628, 2026-11-02)

Move the iOS Metal host (`apps/openagents-ios` `HomeRenderer.swift`) onto
the shared render admission ledger (`aethyr/render/registry.coax`). This
needs an agent on the Mac with Xcode; coderos-4080 can't build the app
(no `xcodegen`, `xcodebuild` or iOS simulator SDK). It is the last open item
of #12628; everything else in it is done on Linux.

What the Mac agent does, following the native Vulkan host
(`verse/native/render-host.c`, 491fbeb7e4), which uses the same exports:

1. Add the ledger's C exports (`aethyr/render/host.coax`, over
   `registry.coax`) to `apps/openagents-ios/generate.sh`; they aren't
   compiled into the app today. Swift then calls
   `aethyr_render_registry_storage`, `_open`, `_acquire`, `_release`,
   `_live` and `_advance`.
2. Acquire a handle before every `makeBuffer` and `makeTexture` in
   `HomeRenderer.swift`: the Cast buffers and texture sheets, the Grid props'
   vertex, face, line and identity buffers, the palette, the dot and line
   buffers, the depth target and the offscreen target; and in
   `HauntAssets.swift`, the house's vertex and index buffers, texture sheets
   and per-frame instance and light buffers. Create nothing when
   the ledger refuses. Release the handle when the object is freed.
3. Each frame: advance the epoch, then refuse the frame before encoding
   anything if a buffer or texture it uses isn't live.
4. Check with `apps/openagents-ios/build.sh` and the simulator tests in
   `apps/openagents-ios/README.md`, then record in #12628 the counts of live
   handles while drawing and after close, as the Vulkan record does.

## Realm seats: release oa-api, the site and the runner (2026-11-02, #12655)

`docs/verse/realm-seat.md` slices 4 and 5 are in `main`, none deployed:
migrations `0108_realm_seat_presence.sql` and `0110_realm_seats.sql`,
`coax/web/api/room.c` and `seats.c` (with `seat-policy.coax`, generated by
`coax/web/api/deploy.sh`), the made-realm page (`myrealm-room.coax`), the
CLI (`realm play` as the account, `--hosted`, `realm shot --as-me`) and the
runner's hosted seats (`coax/web/agent/seat-runner.c`, `seat.coax`; its
image now installs Chromium, `coax/web/agent/Dockerfile`).

1. Release oa-api and the site to staging (`coax/web/release.sh`), then
   `coax/web/agent/deploy.sh --staging` for the runner. The runner's image
   grows by Chromium (about 300 MB); hosted seats draw WebGPU on SwiftShader
   there, which no one has run in Cloud Run yet: check the first one opens.
2. On staging, with a profile that has a key: `openagents realm play
   village-green` while a browser has the realm open (the seat shows as a
   bronze figure with the account's name and AGENT), `openagents realm shot
   SLUG --as-me` on a private realm of yours, and `openagents realm play
   village-green --hosted` then `realm look village-green --turn 90`.
3. Production after that.

Run locally on coderos-4080 (docs/verse/realm-seat.md, "Local runs"): the
agent-pane page check passes, and `realm look --pitch` and `realm use build`
worked against local made realms. The "WORLD FAILED TO LOAD" seen from here
under SwiftShader is this machine's headless Chromium 152, which can't present
a WebGPU canvas (no shared image for its swap chain), not staging. One risk for
step 1: if the runner image's Chromium has the same gap, a hosted seat's page
loses its device on its first frame and never lays out (the offscreen fallback
works only on a localhost page). Check that the first hosted seat says what it
sees. (On this machine's GPU, the default since #12678, a canvas presents
without losing its device: Everglade drew 8 s with no loss. Only a
screenshot leaves the canvas out, since Chrome composites in software.)

The agent pane (EDIT > AGENT on a made realm; `verse/edit/agent.coax`,
`seats.c` now takes the page's logged-in session) ships with the same release.
After step 1, on staging, logged in, on a made realm of yours: EDIT > AGENT >
START YOUR AGENT, type `look left`, then CLOSE. Also `openagents realm look
SLUG --pitch 20` on a first-person realm and `realm use SLUG build barrel` on
one of yours with build on, with a real GPU and a page that draws.

## Verse OS V7: build and boot the image on a second machine (2026-11-02)

Update: the image now builds and boots in QEMU to the running session in
10.5 s from the kernel (docs/os/2026-11-02-verse-os.md, "Status"). What's
left is a machine with a GPU, which a VM here does not have, to see the Grid
on its screen. Building the image needs about 10 GB free on the build
machine.

The image evaluates (`nix flake check ./os/nix`) and its realms package
builds, but the disk itself was not built here, to keep the machine free for
the other work on it. On a free machine with an AMD or Intel GPU and TPM2:
`REALM="newland verse everglade" verse/gpu/realm.sh local ship os/nix/ship`,
then `nix build ./os/nix#nixosConfigurations.verse-os-image.config.system.build.image`,
write the image to a USB disk, and boot it. It should reach the Grid on tty1
with no key pressed. Two choices for you: the host that serves update
versions (`verseos.image.updateUrl`, now `https://openagents.com/verse-os/`),
and whether `/home` may be encrypted with TPM2 alone or should ask for a
passphrase.

## Verse OS V5: let this machine serve as a Pylon when idle (2026-11-02)

`verse-session --pylon` starts `openagents pylon start --model M` after ten
minutes without input and stops it when you touch a key. It is off unless you
pass `--pylon` (and `--pylon-source` with this checkout). Say whether
coderos-4080 should, and with which model.

## Verse OS V2: which Spark wallet runs on Linux (2026-11-02)

Verse OS makes your Nostr identity in the Grid (docs/os/2026-11-02-verse-os.md,
"Status"), and Spark takes the same 12 words. But Breez's Spark SDK runs only
inside the iOS app today (`Wallet/SparkHost.swift`). A wallet on the Linux
session needs one of: Breez's Rust SDK through its C/UniFFI bindings linked
into the C host (a new non-C SDK, AGENTS.md firm reason 6), a small wallet
daemon on the machine that the session talks to over a socket, or waiting
for a Coax Spark client. Say which; until then the boot line reads WALLET
not set up.

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

## Every realm's 60 FPS report: which machine is the reference (2026-11-02, #12649)

`coax/web/realm-perf.coax` times every realm in `realms.json` at dpr 1 and
2 and as a 390x844 phone, and `release.sh` now runs it after each release as
a report (`releases/<time>/REALMS.tsv` and `REALMS.txt`). Since #12678 it
measures on coderos-4080's RTX 4080; the first sweep of production there
(2026-11-02, a shared machine at load 16 to 47) is on #12649: every realm
draws, and the misses are single 33 ms frames except hold-the-stair at
1440x900 dpr 1 in one of two runs. What is left for you: whether the release
Mac's sweep should stay the reference, since frames on coderos-4080 share
the machine with every agent's builds. Not blocking.

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
5. **Presence frames against the relay's rate limit (#12630).** Two Verse
   clients exchanged signed NIP-MV pose frames (kind 23300) through a local
   Coax relay with test keys: every frame arrived, in order, median 5 ms
   (`docs/verification/2026-11-03-nostr-consumers.md`). At 10 frames a
   second the relay's default limit of 60 events a key a minute (the Rust
   relay's too) refused every frame after the 60th; with `--rate
   1200,120,1200` all 200 arrived. Decide the policy for presence before a
   realm publishes frames: exempt ephemeral 23300 from the per-key limit,
   give it its own limit, or cap realms at one frame a second. The relay
   is #12631's; nothing was changed.
6. **A wallet interaction for chapter 101 (#12630).** No regtest or Spark
   test environment exists on this machine. Breez's local regtest is
   third-party `docker compose` code that `docs/lightning/testing.md` says
   to run on a cloud host, not here. Choose one: approve a GCE host for
   Breez's local regtest (pinned to the SDK tag we ship), or supply a Spark
   test environment. Until then the app-zap-with-payment exit stays open;
   no real payment was made.
7. **Encrypted key storage for the CLI, the Pylon and the coder host
   (#12630): a design to decide, not built.** Today the rule is a 0600 key
   file (the coder host: mode 600 in a mode 700 folder), never the Keychain
   unless `OPENAGENTS_KEYCHAIN=1`, and the Pylon refuses `--key-file`. That
   keeps all three consumer profiles incomplete on key storage
   (`nostr/book/reports/`). Options:
   - **A. NIP-49 `ncryptsec` file.** The same path holds an `ncryptsec1…`
     (scrypt + XChaCha20-Poly1305, `coax/lib/crypto/scrypt_nip49.coax`,
     chapter 47) instead of hex. A passphrase is read from the terminal, or
     from `OPENAGENTS_KEY_PASSPHRASE` for unattended hosts. Cost: a prompt
     per command, or a passphrase in the environment, which a headless Pylon
     needs anyway. Old hex files keep working, with a one-time `nostr
     encrypt` to convert.
   - **B. Platform keystore by default.** The Keychain on macOS and the
     Secret Service or kernel keyring on Linux, with the file as the
     fallback. Cost: the reason the Keychain is opt-in (every rebuilt binary
     prompts on macOS) comes back, and headless Linux often has no Secret
     Service.
   - **C. Keep the 0600 file and say so.** Record plaintext-at-rest as the
     accepted profile for these three native consumers; the report would
     then need a "accepted by the owner" state instead of a blocker.
   Recommendation: A, with the passphrase from the environment for the
   Pylon and the coder host, and the file path the Pylon's `--key-file`
   would read once real Pylon keys are approved (item "Real Pylon keys"
   below). Say A, B or C; nothing is built until then.
   **Also the hosted runner and the unlock opener (2026-11-03, fourth pass,
   `docs/verification/2026-11-03-nostr-consumers-4.md`).**
   - **The hosted runner.** Its key comes from Secret Manager
     (`oa-agent-runner-key`), and Cloud Run delivers it as the environment
     variable `OA_RUNNER_KEY` (`coax/web/agent/deploy.sh`). `runner.c` copies
     it and calls `unsetenv`, and no child process gets it in its
     environment. But the bytes stay in the runner's initial environment
     block. With a test key, `/proc/<pid>/environ` of the running `oa-agent`
     still showed `OA_RUNNER_KEY=` and the whole key, so any process running
     as the same user can read it. `ANTHROPIC_API_KEY` and `OA_CHAT_SEAL_KEY`
     arrive the same way. Choose one:
     - **R-A.** Mount the secret as a file
       (`--set-secrets /secrets/runner/key=oa-agent-runner-key:latest`, as
       the deploy keys already are), and have the runner read it once. It
       never enters the environment, but the file stays readable while the
       instance runs.
     - **R-B.** Keep the variable, and at start have the runner overwrite the
       original `NAME=value` bytes of all three secrets before the
       `unsetenv`. It's small and `deploy.sh` doesn't change.
       `/proc/<pid>/environ` then shows zeros.
     - **R-C.** Have the runner fetch the secret from the Secret Manager API
       with its service account. Neither the environment nor the filesystem
       holds it, but it's more code, and any process running as the same user
       can use that account too.
     - **R-D.** At start, have the runner re-exec itself with an environment
       that holds none of the three secrets, handing them to the new image on
       an inherited pipe or a sealed `memfd` that it reads once and closes.
       The new process's initial environment block never held them, so
       `/proc/<pid>/environ` can't show them, with no byte overwriting.
       `deploy.sh` doesn't change. The keys still live in the runner's
       memory.
     - **R-E.** With any of the above, have the runner call
       `prctl(PR_SET_DUMPABLE, 0)` at start. Other processes of the same user,
       the engines and the claude CLI among them, then can't read its
       `/proc/<pid>/environ` or `/proc/<pid>/mem` or attach to it. One line,
       no deployment change; it also turns off the runner's core dumps.

     Recommendation: R-B now, then R-A or R-C with the encrypted-storage
     decision above. R-D is the thorough form of R-B, and R-E is cheap
     alongside any of them. Nothing is built until you choose.
   - **The unlock opener.** A recipient key pasted into the opener is kept in
     the site's IndexedDB (`coaxial-unlock`) as hex, in the clear
     (`coax/web/unlock/page.ts`), like web sign-in's entropy. It falls under
     the same browser-storage decision.
8. **iOS profile run (#12630).** The `ios-app` consumer (Keychain storage,
   libsecp256k1 signing in `NostrKey.swift`) needs a run on the Mac: build
   with `apps/openagents-ios/build.sh`, sign in with a phone key on the
   simulator against a local relay or staging, and keep the log. This
   machine is Linux and can't run it.
9. **The unlock opener's Coax signer (#12630).** The hidden-presentation
   opener can sign its NIP-42 AUTH, verify the gift wrap's and the seal's
   signatures, check the artifact's id, and (since the fourth pass) open the
   NIP-44 payloads of the wrap, the seal and the artifact with
   `coax/web/unlock/unlock-nostr.coax` (Wasm) instead of nostr-effect. To
   switch, deploy with `COAXIAL_UNLOCK_SIGNER=coax`; `coax/web/deploy.sh`
   builds the module, and unsetting the variable is the rollback. Checked with
   test keys only: `docs/verification/2026-11-03-nostr-consumers-3.md`, and
   `-4.md` for NIP-44 against the upstream vectors and a headless browser
   run. The opener signs and decrypts with the recipient's key, so switching
   production is yours: say when, and the next deploy sets it.

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

Open 2026-10-30, mechanism built 2026-11-03 (`--checkpoint`; its home is
question 3 of "Coax relay cutover" above): **a rollback-resistant checkpoint
for the relay's store (#966b).** Crash recovery is done (`092c3e44a4`,
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

## 2026-11-02 — Coder local Parakeet dictation hardware check

Agents test recorded speech and injected permission/capture effects without
opening your microphone or showing a window. The remaining hardware check is
[local dictation](docs/voice/coder-dictation.md):

- [ ] Build/open `apps/openagents-desktop/build/OpenAgents.app` when ready;
      enable **OpenAgents → Enable Local Dictation — Parakeet**, grant macOS
      microphone permission and wait for model readiness.
- [ ] Hold **⌘D** or the microphone, speak and release. Confirm the words land
      at the composer caret, one Undo removes them, and nothing is sent.
- [ ] Cancel with Escape or switch apps during a hold/recognition. Confirm no
      stale words appear, capture stops in the background, and returning can
      rearm without reloading the already-loaded model.
- [ ] Disable the setting and confirm the microphone indicator goes away.

### Desktop compiler work-budget decision — approved

The owner approved raising the bounded shared work cap from 1,048,576 to
2,097,152 and finishing the Mac app. There is no remaining budget-choice
request. The [adoption record](docs/verification/coder-dictation-2026-11-02/adoption/README.md)
records passing source-profile adoption, focused development compiler/native
checks, current app build and recorded-audio offscreen dictation. The earlier
diagnostic evidence is retained separately; normal compiler bootstrap/release
qualification is not claimed. The microphone and macOS permission checks above
remain pending.

### Parakeet dictation input selection and route changes

The software checks use injected devices and public recorded audio; they never
open the microphone or request permission. In OpenAgents for Mac:

- [ ] Open **OpenAgents → Dictation Input…** and choose a specific microphone.
      Relaunch and confirm that input remains selected; **System default input**
      should follow macOS only when explicitly selected.
- [ ] With dictation enabled, begin a hold and unplug or change the input.
      Confirm capture stops and no stale transcript enters the composer.
- [ ] Reopen **Dictation Input…**, choose an available input and try a fresh
      hold. Confirm it produces one editable draft and never sends it.

An unavailable explicit selection must show as unavailable rather than silently
using another input. [Software record](docs/verification/coder-dictation-devices-2026-11-02/README.md).

### `openagents dictate`: a real microphone on the Mac and on Linux (#12667)

The command was built and checked on coderos-4080 with recorded audio only:
a test capture backend, and a stand-in `pw-record` that writes zeros. No
microphone was opened. The macOS capture file (`apps/openagents-cli/dictate-mac.m`,
CoreAudio and AVFoundation) could not be compiled on Linux.

- [ ] On the Mac: `apps/openagents-cli/build.sh`. Confirm it compiles
      `dictate-mac.m` and links `build/openagents-dictate` with Metal.
- [ ] Run `apps/openagents-cli/build/openagents dictate` in Terminal. macOS
      asks for microphone access for Terminal once; allow it, say a sentence,
      press Enter. Confirm the text prints, and that the microphone indicator
      goes off as soon as recognition starts.
- [ ] Deny access (System Settings > Privacy & Security > Microphone), run it
      again: confirm exit 4 and the message naming that setting. Re-allow it.
- [ ] `openagents dictate --input NOT-A-UID`: confirm exit 6, and that no
      other microphone is used. Unplug the selected input while listening:
      confirm a capture error and no text.
- [ ] Run `coax/web/tool.sh apps/openagents-cli/tests/dictate-check.coax
      ~/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99` and
      add its lines (Metal) to [the record](docs/verification/dictate-cli-2026-11-02/README.md).
- [ ] On a Linux desktop with PipeWire: `openagents dictate --seconds 10`,
      speak, and confirm the text. It now recognizes on CUDA by default
      (#12672; about 50 ms for a 3 s clip on coderos-4080's RTX 4080);
      `--device cpu` takes about 8 to 15 times the audio's length. Try
      `--capture alsa` (or `pulse`) if PipeWire is absent, and unplug the
      input while listening: expect a capture error and no text.

### Parakeet dictation on the Mac: release latency and the Coder composer (#12665)

The Linux CUDA profile was measured on coderos-4080 with recorded public audio
([record](docs/verification/dictation-eval-2026-11-02/README.md)). The same
harness builds on macOS (Metal) but was not run there, and the Coder composer
path (key release to text in the composer) needs the app:

- [ ] On the Mac, after `apps/openagents-cli/build.sh`, fetch the corpus as the
      record's "Reproduce" section says, then run
      `coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax run DIR gen release press noise seam snr startup`
      and `... score DIR`. Add the Metal tables and gate lines to the record.
      The runner's build line uses `-DPSIONIC_PARAKEET_CUDA`, `-ldl` and
      `-Wl,--wrap`; on macOS it needs the Metal define, `-framework Metal
      -framework Foundation` and no `--wrap` (Apple's linker has none), so
      expect to adapt `build_runner` first.
- [ ] In OpenAgents for Mac with dictation enabled, hold ⌘D, say a sentence of
      about 5 s, release, and time release to text in the composer (a screen
      recording at 60 fps is enough), ten times. Add p50 and p99 to the record.
- [ ] Open the app cold and time until dictation is ready (the model open now
      checks its 38 pinned pieces on several threads; on Linux it went from
      27-41 s under load to 2.4-3.7 s).

### Coder for Mac: physical long-dictation acceptance after the seam fix (#12694)

The shared dictation controller (`apps/shared/dictation.coax`,
`dictation-policy.coax`) now cuts dictations over 20.47 s in a pause and
asks for 300 ms of silence at each cut end of a model call
([record](docs/verification/dictation-eval-2026-11-02/README.md#seams-after-the-fix-12694)).
The missing C wrappers and Swift padding were implemented and checked over
SSH on the Mac on 2026-11-03. The full app builds; controller/host sanitizer
checks pass, a 30.46 s public recording preserves every reference word on
Metal, and the offscreen app passes exact insertion, Undo and zero-send checks.
See the [Mac follow-up record](docs/verification/dictation-eval-2026-11-02/README.md#mac-consumer-adoption-2026-11-03).
No physical microphone or permission check was performed.

- [ ] On the Mac, open `/tmp/openagents-parakeet-seams-20261103/OpenAgents.app`
      (or rebuild from current main), enable **Enable Local Dictation —
      Parakeet**, then dictate about 30 s in one hold, in sentences. Check that no words
      are missing or doubled where the text was split.

### Verse voice host: Parakeet recognition on the Mac (#12669)

The Parakeet recognizer (`verse/voice-host --recognizer parakeet`) was built and
qualified on coderos-4080 (Linux; first the CPU path, now CUDA by default, #12672).
The Mac was not available to the agent:

- [ ] On the Mac, run `verse/voice-host/build.sh`. It now links both
      `libpsionic-parakeet.a` and `libpsionic-whisper.a`; confirm the link has no
      duplicate-symbol error between the two Psionic archives.
- [ ] Run `coax/web/tool.sh verse/voice-host/parakeet-check.coax` there (Metal by
      default) and add its latency lines to
      [the record](docs/verification/verse-parakeet-2026-11-02/README.md).
- [ ] Hold to talk with the real microphone in Aethyrforge once its forge bar
      uses the local host; the checks use recorded fixtures only.

### Coder terminal: leading file paths

- [ ] Restart an already running Coder terminal to load the path-input fix.
      Paths such as `/Users/me/project/file.coax fix this` now go to the
      agent as messages; only registered slash command names are intercepted.

### Coder terminal: model and reasoning controls

- [ ] After updating the checkout, restart `openagents coder` or
      `coder/tui/coder.sh`. Shift-Tab to Codex now shows the model and reasoning
      level on the right of the working-step line above the top rail.
      `/model` lists models; `/model ID [LEVEL]`
      and `/effort LEVEL` change the next Codex message and keep its conversation.
      Selections last for this Coder session. The separately installed legacy
      `~/.openagents/bin/coder` (1.0.0 at verification) is a different executable.

### Coder terminal: Claude session reader in Codex (#12722)

- [ ] After updating to the #12722 Codex follow-up, restart `openagents coder`
      or `coder/tui/coder.sh` and open a fresh Codex chat. Tool definitions
      are registered when its app-server thread starts. Ask it to summarize
      the Claude conversation; it now has `coder_session_read` directly,
      including paginated compaction context and unique native ID prefixes.
      A fresh real Codex chat was verified programmatically with the owner's
      shortened ID: three reader calls, all conversation/context pages and
      no shell commands. An already running thread keeps its earlier catalog.
