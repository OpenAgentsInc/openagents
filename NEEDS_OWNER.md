# Owner actions

## Made-realm golden after the incoming renderer change (2026-10-31)

Resolved visual review: the original author reviewed and updated the `built`
golden in main commit `21dc5da9ca` ("looked at"). No further owner sign-off is
pending. The targeted validation against that approved image is recorded with
the language verification follow-up; this audit did not approve or change a
snapshot.

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
administrators. Kernel tuning for Psionic's CUDA chapters (26 and 30) works
without them, from event timings, but per-kernel occupancy, memory and
instruction counters need them. Allowing them is a system change: set the
`nvidia` module option `NVreg_RestrictProfilingToAdminUsers=0` (on NixOS,
`boot.extraModprobeConfig = "options nvidia NVreg_RestrictProfilingToAdminUsers=0";`)
and reload the driver or reboot. Decide whether to allow it; nothing is
blocked if not.

## Review Claude's first blog post

`docs/blog/2026-10-29-attention-that-cannot-miss.md` is a draft post by Claude
about Psionic chapters 165 and 166 (exact memory by attention, and traces as
receipts). Read it and approve, edit or reject it. Publishing it needs a blog
route on openagents.com; the steps are in `docs/blog/README.md`.

## Psionic chapter 19: the Pylon's key, relays and TLS (2026-10-29)

Chapter 19 runs a Pylon that answers Elephant decisions over Nostr
(psionic/book/PLAN.md, "Before chapter 19"). The code is being built now;
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
