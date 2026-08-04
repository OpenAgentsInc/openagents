# Addendum A — Could the public projection be GPUI in the browser instead of a DOM app?

- Date: 2026-08-04
- Class: research addendum to
  [`2026-08-04-nostr-native-hardening-program.md`](2026-08-04-nostr-native-hardening-program.md) §6.3
- Status: research finding and recommendation. Authorizes nothing.
- Question asked: instead of building the hardening projection with
  TanStack/React and separately defined components, could we compile Omega's
  GPUI components to WebAssembly and render them on the web?

## 1. Short answer

**Yes, technically — and much more readily than expected. Our own tree already
contains a working GPUI web backend, and it builds.** But it is the wrong tool
for *this particular page*, and the right tool for two adjacent things we will
want soon.

The measured position:

| Question | Answer |
| --- | --- |
| Does GPUI run in a browser at all? | Yes. `crates/gpui_web` is a complete platform backend in our own tree |
| Does it build here, today? | Yes — a hello-world example compiled clean in 51s on this machine |
| What does it cost to ship? | **10.6 MB of wasm, 3.2 MB gzipped**, for hello-world, before wasm-opt |
| Will it render for our audience? | For roughly 70% of browsers. Firefox on Linux and Chrome on Android are still gaps |
| Is the page accessible, searchable, linkable? | **No.** Canvas rendering with no accessibility adapter on web |
| Is upstream healthy? | Mixed. The work is recent and active, but Zed publicly slowed GPUI in 2026 |
| Is any Omega product code wasm-ready? | Yes — `omega_forensics` type-checks clean for `wasm32-unknown-unknown` today |

## 2. What is actually in our tree

This is the surprising part, and it is worth recording precisely because a
reasonable person would assume otherwise.

Omega (`~/work/omega`, our Zed fork) contains a **crate-split GPUI** with a
first-class web platform:

```text
crates/gpui            core framework
crates/gpui_platform   platform abstraction
crates/gpui_macos      Metal / AppKit
crates/gpui_linux      Vulkan / Wayland / X11
crates/gpui_windows    DirectX / Win32
crates/gpui_web        ← browser backend (wasm)
crates/gpui_wgpu       ← wgpu renderer used by the web backend
```

`crates/gpui_web/src` is 2,816 lines across nine modules — `platform.rs`,
`window.rs`, `events.rs`, `dispatcher.rs`, `display.rs`, `http_client.rs`,
`keyboard.rs`, `logging.rs`. It is not a stub:

- **Input**: pointer, mouse, wheel, keyboard, drag, and IME composition events
  wired through `web-sys`, plus `ResizeObserver` and `VisualViewport`.
- **Clipboard**: real read/write through the async clipboard API.
- **Networking**: an HTTP client implemented over `fetch`, including from
  background workers, with configurable credentials.
- **Concurrency**: a real dispatcher with a `multithreaded` feature over
  `wasm_thread` (pointed at a Zed-maintained fork), plus idle-period
  scheduling.
- **What it refuses**: `prompt_for_paths`, `prompt_for_new_path`, `minimize`,
  `zoom`, and `quit` log honestly that they are unsupported in a browser.
  Those are the inherently-native ones.

Provenance: the feature landed upstream as **"GPUI on the web" (Zed #50228) on
2026-02-26**, and has been iterated since — recent commits in our tree include
fetch-from-worker support (#61782), a `wasm_thread` fork fix (#61807), and
web idle scheduling (#61827). We track it through our `zed` remote; we did not
write it and we do not maintain it.

Notably, the crate split that a former Zed employee described as the
prerequisite for web support — *"the crate needs to split into
core/web/native"* — **has already happened**. That public commentary is older
than the code.

## 3. What we measured on this machine

Reproduction, from `crates/gpui_web/examples/hello_web`:

```sh
rustup component add rust-src --toolchain nightly
cargo +nightly build --release --target wasm32-unknown-unknown \
  -Z build-std=std,panic_abort
```

Results:

- **Builds clean**, `Finished release profile in 50.88s`, one benign warning.
- **`hello_web.wasm` = 10.6 MB raw, 3.2 MB gzipped.** This is *before*
  `wasm-bindgen` and `wasm-opt`, which Trunk runs and which typically recover
  a meaningful fraction — but it is also a hello-world that draws text and
  counts primes. A real component set is additive, not smaller.

Build requirements, all of which become deployment requirements:

- **Nightly Rust** with `rust-src` and `-Z build-std` — an unstable toolchain
  in the release path for a public page.
- **wasm atomics and shared memory**: `+atomics,+bulk-memory,+mutable-globals`,
  `--shared-memory`, `--import-memory`.
- **Cross-origin isolation**: the example's `trunk.toml` sets
  `Cross-Origin-Embedder-Policy: require-corp` and
  `Cross-Origin-Opener-Policy: same-origin`, required for `SharedArrayBuffer`
  and the threaded renderer. Cross-origin isolation constrains every
  third-party asset, iframe, and embed on that origin — a real architectural
  commitment for a page on `openagents.com`, not a build flag.

Rendering goes through `gpui_wgpu` → `wgpu` → **WebGPU**. As of 2026 WebGPU
ships by default in Chrome/Edge, Firefox 147+ on Windows and macOS, and
Safari 26+ on macOS/iOS/iPadOS — roughly **70% of browsers**, with Firefox on
Linux still in development and Firefox on Android targeted for late 2026.

## 4. The disqualifier for this specific page

The hardening projection is not an application. It is **public evidence**, and
its entire job is to be legible to strangers and to machines. A canvas gives
up the properties that make it evidence:

| Property the page needs | DOM | GPUI canvas |
| --- | --- | --- |
| Screen-reader accessible | Yes | **No adapter.** `accesskit` is wired in `gpui_macos`, `gpui_linux`, and `gpui_windows`; `gpui_web` has no accesskit reference at all |
| Ctrl-F, text selection, copy a digest or event ref | Yes | Must be reimplemented, badly |
| Search-engine indexable, link previews | Yes | No |
| Deep links to a target, finding, or attestation | Yes | Only with hand-rolled routing |
| Renders on any browser, old device, no GPU | Yes | ~70%, GPU required |
| Cheap first paint on a phone on bad wifi | ~100 KB | 3.2 MB gzipped minimum |
| Quotable in someone else's article or issue | Yes | Screenshot only |

There is also a credibility argument specific to this program. We are asking
an ecosystem to trust a coverage ledger whose thesis is *do not make
completeness claims you have not earned*. A page that silently fails to render
on a Bitcoin developer's Firefox-on-Linux, or in a screen reader, is the same
category of error dressed differently. The projection should be the most
boring, most durable, most linkable thing we ship.

## 5. Where GPUI-on-web is genuinely the right tool

The research changes my view on three adjacent things, all of which are worth
more than the page itself:

**5.1 Omega in the browser, as a distribution channel.** The strongest possible
answer to "how do I join the hardening effort" is a URL rather than an
installer. GPUI-on-web makes an Omega-shaped surface in a browser tab
plausible for the first time — same component set, same event handling, no
second UI to maintain. The blocker is not GPUI, it is the rest of the crate
graph: filesystem, PTY, git, LSP, process spawning, and node are all native.
A browser Omega is necessarily a **subset build** — read, review, answer,
approve, steer — which happens to be exactly the mobile-controller scope the
All Work thesis already wants.

**5.2 One interactive artifact embedded as a canvas island.** The evidence
graph, the divergence explorer, or a diff viewer is a legitimately better
canvas widget than DOM widget. Mounting one GPUI canvas inside an otherwise
ordinary DOM page — behind a WebGPU capability check with a DOM fallback —
gets the demo value with none of the page-level costs, and proves the pipeline
end to end for a few days of work.

**5.3 Sharing logic rather than pixels — and this one is already true.** The
uncomfortable duplication is not buttons, it is *decoders*: the same event-kind
parsing, coverage arithmetic, and completeness rules exist in Rust for Omega
and would be written again in TypeScript for the web. Compiling the pure-Rust
projection logic to wasm and calling it from a DOM page removes the
duplication that actually causes bugs, while leaving rendering to the platform
that is good at it.

Measured on this machine: `cargo check --target wasm32-unknown-unknown -p
omega_forensics` **succeeds with zero errors in 29.7s.** Omega's forensic
domain crate — entropy campaigns, repository state, prior work, ranked
tranches, tool ingestion, verdict workflow — is already wasm-clean today,
without anyone having tried to make it so. That makes this the cheapest of the
three: a much smaller wasm payload than a UI framework, no cross-origin
isolation, no WebGPU requirement, and it composes with the SDK instead of
replacing it.

## 6. The option the repository already mandates

There is a third answer that the question's framing ("TanStack with separately
defined components") slightly understates: **the repo's declared UI
architecture is already "one typed component set, thin swappable renderers."**

`~/work/effect-native` ships `render-dom`, `render-canvas`, and `render-rn`,
and `apps/openagents.com/apps/start` already depends on all three plus
`@effect-native/core`, `khala-ui`, and `tokens`. React and TanStack are the
renderer adapter and serving host — not the component definition. So the
components for the hardening page do not have to be "separately defined" in
the first place; they are authored once in Effect Native and rendered to DOM
today, with a canvas renderer already present if a surface needs one.

GPUI-in-browser and Effect Native are therefore **not competing for the same
slot**. Effect Native is the product component set across web, mobile, and
desktop. GPUI is Omega's native runtime, which happens to also target
browsers. The interesting question is not which one wins; it is whether Omega
eventually renders Effect Native components, or whether the two remain
deliberately separate with the SDK and contracts as the shared layer.

## 7. Recommendation

Three tiers, in the order I would take them:

**Tier 1 — the page itself: DOM, via Effect Native, over the SDK.** No new
toolchain, no cross-origin isolation, accessible and indexable by default,
and it satisfies §6.3 of the spec as written. This is the Episode 266 path and
nothing in this research changes it.

**Tier 2 — one GPUI canvas island, opportunistic.** Pick a single artifact
that is genuinely better on canvas, ship it behind a capability check with a
DOM fallback, on a separate origin or route if cross-origin isolation proves
awkward. High demo value, contained blast radius, and it establishes the
build path for Tier 3.

**Tier 3 — browser Omega, gated and strategic.** Worth a real feasibility
spike, not a commitment. It should be gated on four answers: how much of the
workbench crate graph is wasm-clean; how key custody and signing work in a
browser (NIP-07 and NIP-46 are the obvious adapters, and Omega's own
assurance doc already marks web rows `not-admitted`); whether cross-origin
isolation is acceptable for the hosting origin; and whether upstream GPUI
maintenance is healthy enough to depend on.

## 8. The risk that decides Tier 3

Zed publicly slowed GPUI in 2026 — reported as *"GPUI development is getting
some major brakes put on it. We gotta focus on some business relevant work in
2026"*, confirmed by a former Zed employee who noted it is hard to justify work
on GPUI that is purely for the community. A community fork, **gpui-ce**,
exists and publishes to crates.io; a separate effort, WGPUI, has diverged.

This cuts both ways and should be stated honestly rather than resolved by
preference:

- The web backend is *already written*, recent, and in our tree. We inherit it
  whether or not upstream continues.
- But betting a public product surface on a paused upstream means we would own
  the maintenance — including the accessibility adapter that does not exist,
  and the nightly-toolchain dependency.
- Our fork already carries this exposure for the desktop app. Extending it to
  the web multiplies the surface without adding a second maintainer.

## 9. Cheap experiments that would change this conclusion

Each is falsifiable and small:

1. **Run `wasm-opt -Oz` on the hello-world build** and re-measure. If the
   shipped bundle lands near 1 MB gzipped, Tier 2 gets easier and Tier 3 gets
   plausible.
2. **`cargo check --target wasm32-unknown-unknown -p <product crate>`** across
   the remaining workbench crates. `omega_forensics` already passes clean
   (§5.3); the crates that fail are the actual Tier 3 backlog, and the shape of
   their failures — filesystem, process, PTY, git — will say whether a subset
   build is a weekend or a quarter.
3. **Load the example in Firefox on Linux and Safari on iOS** and record what
   happens. Our audience is not a Chrome monoculture.
4. **Prototype the shared-decoder path** (§5.3): compile one projection
   function to wasm, call it from the DOM page, and compare its size and
   correctness against the TypeScript implementation.
5. **Watch whether an accesskit web adapter lands.** That single dependency is
   most of the difference between "canvas island" and "viable public page."

## 10. What this addendum does not claim

- Not a claim that Omega runs in a browser. A hello-world example built; the
  application did not.
- Not a claim about shipped bundle size after full optimization — 10.6 MB raw
  and 3.2 MB gzipped is a pre-`wasm-opt` measurement of a trivial program.
- Not a recommendation to adopt or reject GPUI-on-web as a platform. It is a
  recommendation about **one page**, plus three places the capability is worth
  spending on.
- Not an authority change. The UI architecture remains what `CLAUDE.md` says
  it is, and any Tier 2 or Tier 3 work needs its own admission.

## Sources

- Local evidence: `~/work/omega/crates/gpui_web/`, `crates/gpui_wgpu/`,
  `crates/gpui/Cargo.toml`, `crates/gpui_web/examples/hello_web/`, and the
  build measured on this machine 2026-08-04.
- [GPUI on the web — Zed #50228](https://github.com/zed-industries/zed/pull/50228)
  (via `git log crates/gpui_web`, 2026-02-26)
- [GPUI WASM — zed-industries/zed discussion #8203](https://github.com/zed-industries/zed/discussions/8203)
- [Hacker News: Zed slowing GPUI development](https://news.ycombinator.com/item?id=47003569)
- [gpui-ce — GPUI Community Edition](https://github.com/gpui-ce/gpui-ce)
- [gpui.rs](https://www.gpui.rs/) and
  [zed-industries/awesome-gpui](https://github.com/zed-industries/awesome-gpui)
- [WebGPU is now supported in major browsers — web.dev](https://web.dev/blog/webgpu-supported-major-browsers)
- [WebGPU](https://webgpu.org/)
