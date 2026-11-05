# Shared media readiness delivery

Version 1 browser and native Metal now use `aethyr/host/media-notice.coax`
for the existing 607 payload. Its 131-mutant authority gate is unchanged
from 78c4844d91. It admits at most 112 unique root-relative URLs, omits
pending images, writes successful dimensions, forces failed images to
`0 0`, and emits no final newline. Complete validation and capacity refusal
precede any output write. A refused notice is not truncated or partially
delivered. Image failure remains local to that image; unrelated rendering
continues.

The browser's separate natural-image reactor now exports scalar row and
output projections. Coax owns physical row, URL-span, flag and output
admission, borrowed URL views and reset defaults. JavaScript copies admitted
bytes and immediately consumes the synchronous result. Renderer initialization
awaits the reactor before it can publish media readiness, so formatting adds
no asynchronous ownership gap. Completion checks its concrete engine and
media owner before notifying. Pinned pre-v1 hosts retain their earlier path.

The actual browser host records copied inbox bytes after calling the real
realm message export. The capture-only trace is bounded and reports drops;
the observer requires zero drops. It compares the final actual 607 byte
array against the Coax formatter and independently checks every ready/failed
fact. A scalar helper invoked three times is not the delivery observation.

| Actual production-adapter workload | Ready / failed | Further completed submissions | After device recovery |
| --- | --- | ---: | --- |
| Seven authored JPEGs | 7 / 0, every image 48×48 | 6 | Same complete notice and 6 more submissions |
| First JPEG replaced by malformed bytes | 6 / 1, first 0×0 | 6 | Same explicit refusal and 6 more submissions |

Both runs pass all 13 existing held-completion/frame ownership observations,
including refusal without producer progress, fresh device/ledger, a late old
completion unable to release the replacement pending frame/upload prefix,
old handles retired and resumed progress. Their served-input and observer
source fences pass. The first diagnostic attempt retained an empty delivery
trace and an animation-callback counter that excludes manual clip steps;
the observer was corrected to trace the actual generic inbox delivery and
measure completed GPU submissions individually. That failed attempt remains
retained separately.

Native Metal's matching actual error/recovery and deferred delivery receipts
are published with 10f6ae05c6/90e358f81d. Missing files, malformed metadata and
late PNG failure leave one image failed/not-ready while six world frames
continue; ASan checks cover the seven-image bank/recovery and late failure.
The later callback adapter preserves the previously frozen native90 uniforms,
graph and presented bytes exactly, so the direct 1723/1697 Vulkan composition
receipts in [media-canonical.md](media-canonical.md) remain byte-applicable.
This does not claim a new pixel or compiler qualification.

New checks were written before implementation. Final root mutation kills
54 wrapper, 29 semantic-observation and 61 JSON-projection mutants, with no
survivors or equivalents. Earlier failing mutation/build logs are retained.
The JSON checker uses a closed authored-image fixture and exact integer/byte
observations; it does not admit arbitrary image metadata or general JSON URLs.

The original admitted CPU compiler image remains SHA
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
Its genuine c224561457 root passes the unchanged receipt before and after the
separate product overlay. Profile emission has eight exact source hashes;
its generated C/header are byte-identical to those retained from the actual
reactor build. The new reactor is 53,580 bytes within the unchanged 70,000
budget, SHA `d50f26d28febc2de9aee2488b8afb9f1d5aa1751ec3efcbab7839c36369dcaae`.
No realm, shader package or compiler policy/release is replaced.
JavaScript syntax checks pass. The required tools check reports 1,786 tracked
tools, 35 vendored vectors and no new missing reason headers.

Raw delivered payloads, origin requests, both source-fenced ownership
receipts, generated C/header/profile and all attempts remain in
`~/.openagents/aethyrforge/host-contract-12869/media-notice-bridge/`.
Small receipts and the artifact inventory are retained beside this document.
These are service-component results. Ordinary presentation selection remains
disabled, WKWebView remains the desktop default, and #12868/#12869 stay open
until existing compiler/build/load admission and all integrated host gates pass.
