# Owner actions

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

1. **Where the Pylon key lives on the Linux machine.** There is no Keychain.
   Options: a 0600 file under `~/work/.secrets/`, the desktop secret service,
   or a hardware key. The key is generated once and never printed.
2. **Your `auth` signature.** The beacon carries your key's signature over the
   Pylon key (`["auth", <owner key>, "kind=30200", <signature>]`). You sign
   that once per Pylon key.
3. **Which relays** the Pylon publishes to and lists in its NIP-65 record.
4. ~~**TLS for `wss://`.**~~ Decided 2026-10-29: the C host may link a TLS
   library (AGENTS.md, Rules). Being built.

Signing with a real key also waits on the #4828 secret audit and the review
gate in coax/nostr.md.

## Coax relay: which NIPs survive, and the cutover (2026-10-29)

coax/nostr.md section 17 plans a Coax relay to replace relay.openagents.com
(the Rust relay, removed from the tree at the Nuke). Phase R1 runs only on the
tailnet. Left for you: approval of the data migration and the DNS cutover (R3). Nothing touches the production relay before that.

Decided 2026-10-29: the old Rust relay may be built and run inside a sandbox
for the comparison; the Coax relay aims for full NIP support, starting with
what the website uses. Still yours: the R3 migration and DNS cutover.

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
