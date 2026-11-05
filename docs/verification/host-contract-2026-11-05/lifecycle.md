# Shared lifecycle adapter observations

The same ten-row recording face trace passes the actual browser host,
the Linux realm SDK under ASan/UBSan (including leak checking), and the
Metal SDK. The common Coax checker admits all three raw JSON receipts:

- [Browser](browser-lifecycle.json): `check-browser-lifecycle.coax`
  drives actual `host.js`, completed GPU submissions, DOM focus handlers
  and the product world-cover callback in muted headless Chrome.
- [Linux](vulkan-lifecycle.json): `build-vulkan.coax --lifecycle --sanitize`
  links the same production `realm-host.c` used by the Vulkan realm.
  No window or GPU is opened by this SDK-only trace.
- [Metal](metal-lifecycle.json): the real MF adapter and alternate fixture
  producer; the #12868 owner retains its concrete SDK/source join.

Each run refuses a negative first timestamp, a backward offer and hidden
offers before they reach the producer. First and resumed frame deltas are
zero; the subsequent exact interval is 1/64 second. A held W key is released
immediately by hiding. Blur releases input while hidden, and a following
focus cannot restart the hidden clock. A visible unfocused frame is admitted
with zero delta. Six actual frame calls, one key-down and three blur calls
make the ten exact rows; all five final lifecycle words are checked.

The frame clock remains Coax, with no host delta clamp. Its microsecond
event projection checks negative values before converting to room ticks.
The lifecycle authority root gate kills 80/80 mutants, the semantic receipt
104/104, and the JSON adapter 39/39. No equivalents are annotated. These
unchanged successful gates were not repeated for documentation.

The browser fixture Wasm is emitted with the original admitted compiler
SHA `598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
Linux uses the existing compiler identified in the earlier Linux SDK record;
its sanitized observer SHA is
`9c9c0f92e028b0c8d519a5fac972fffeead7a0ac4a429fc9257e8a9598874c53`.
Generated C, headers and the executable remain at
`~/.openagents/aethyrforge/host-contract-12869/lifecycle-sanitized/`
on coderos-4080. The browser inputs and raw receipts remain under
`~/.openagents/aethyrforge/host-contract-12869/lifecycle/`; the Metal raw
receipt is under
`~/.openagents/aethyrforge/metal-host-12868/conformance-current-hidden/`.

This qualifies normalized SDK lifecycle/frame-clock execution. It does
not establish physical window occlusion, a complete session replay after
trap, production GPU recovery or ordinary Vulkan display-loop integration.
Those remain separate exits, together with the failed ordinary 8-bit
Vulkan comparison and Linux audio. The desktop default remains web.

## Shared WebGPU submission ownership

The actual shared WebGPU adapter now checks its physical mutable frame
buffer slot before advancing the producer. That slot remains busy until
both validation and this submission's actual queue fence complete. Device
pending counts belong to the concrete GPUDevice; a retired renderer cannot
acknowledge upload work belonging to its replacement.

`check-browser-frame-ownership.coax` runs the admitted frozen Everglade
Wasm and real shared render interpreter in muted headless Chrome. It holds
SDK completion promises after their real queue completion, independently
of validation. Eight intervening offers preserve the producer SHA, ledger
epoch and submission count. Releasing the fence permits further work. A
second held completion survives actual device destruction and recovery;
releasing it keeps the new device's pending count and a nonempty actual
upload prefix unchanged. New completion clears only its own count, the
producer advances, and all old ledger handles remain retired.

The [raw receipt](browser-frame-ownership.json) has all thirteen observations.
Its Coax authority kills 21/21 root mutants, without equivalents.
The [qualification join](browser-frame-ownership-qualification.json) retains
the actual observer image and served-input identities; the origin's
before/after source fence passes. The workload uses the separately pinned
historical-compiler/product capture from [the provenance record](qualified-shared-frame.md).
It does not regenerate or requalify the shader products. Ordinary Vulkan
display affordability and the complete cross-host GPU exits remain open.
