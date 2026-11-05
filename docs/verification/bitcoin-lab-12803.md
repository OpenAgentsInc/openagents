# Bitcoin Lab #12803: bounded own-node snapshots

Implementation: [`a419dcf53c`](https://github.com/OpenAgentsInc/openagents/commit/a419dcf53c).
Qualified on coderos-4080, Linux/NVIDIA RTX 4080, November 4, 2026. Compiler
SHA-256: `54308753598854075405f995a04eb94f64eeb85f231ee6840a42e77afe6d2e80`.
The implementation was finally rebased on main at `ef63c33f1e`; owned source
bytes remained unchanged. Main had removed cancelled automation schemas; the
shared descriptor was regenerated for that final tree, and all 30 owned
schemas and published compatibility checks pass. The manifest records both
descriptor hashes and the qualified pre-rebase revision. No shared engine or
published explorer/mempool v1 contract was changed.

The Lab now has the same bounded Coax client in the native Wayland/Vulkan
desktop app and the unlisted browser realm. Recording is the offline source;
Fixture API uses acknowledged public regtest facts; explicitly selected Own
Node reads operator-configured committed journals and authenticated block
files. V/X retains validated-v1; a genuine L/I/E origin retains pow-only.
Empty, damaged or foreign validation metadata cannot fall back to a different
profile. The disposable query view is never the source of a validation claim.

The persistent strip shows source/profile, network, exact tip and unknown
observation time. STATUS exposes the full cursor and validation/background
frontiers. Each response carries complete metadata and a string ticket.
Source/profile/network/cursor/tip changes clear related views, including
same-height branch changes. Two owned effects, separate undispatched-slot
sentinels, monotonic tickets, cancellation and bounded transport backoff reject
late callbacks and slot reuse. JSON offsets and parser scratch belong to the
realm's arena; a rejected callback cannot overwrite an admitted object's data.

## Results

| Check | Actual result |
| --- | --- |
| Independent HTTP/raw/Core/Electrs oracle | 928 no-store responses pass across fixture A/B/C and configured V/X journal C: identifiers, bytes, weights, amounts, exact active/stale occurrences, paged histories/outputs and exact-script balances |
| Native file and ASan/UBSan service | Both reorg directions, independently rebuilt equal-height tips, stale cursors, five torn files, corrupt checksum, 256 MiB limit refusal, unavailable bodies, recovery and a genuine pow-only L origin pass; no sanitizer diagnostics |
| Native app's real child processes | Fixture load, branch change, explicit configured node selection, synchronous invalidation and cleanup/reaping of API/fetch children pass |
| Native scene | 1,356 chapter/block/transaction/layout frames pass; peak 29,376 vertices; realm state 8,674,744 bytes, arena 12,131,096 bytes, within the declared 16 MiB allocation budget |
| Actual Vulkan host | RTX 4080, three passes at 1440×900, 600 frames; CPU median 0.14 ms / p99 0.25 ms; GPU median 0.45 ms / p99 0.57 ms in the recorded run |
| Focused realm/client checks | Eight realm checks and 17 Lab/client checks pass, including short-phone map geometry, bounded script text, slot collision and increasing transport backoff |
| Muted headless browser | Desktop and 390×844 phone: actual bounded fetch, fixture/node selection, reorg, paging, transaction graph, immediate clearing, source status, pending-request trap/rescue, device recovery and return to recording pass; offscreen GPU readback screenshots retained |
| Protocol | Pinned protoc 29.3 / protobuf 5.29.3: 30 tool tests, owned/vendor schemas, compatibility/migration and consumer profiles pass; regenerated owned descriptor committed |
| Repository tools | 31 unrelated existing missing-header violations; none names a changed file in this issue |

Authority gates have **617 mutants killed, zero survivors**, without equivalent
annotations: closed routes 198, response identity/tickets 309, exact transaction
occurrence 47, bounded page admission 47, original-profile admission 16. Gate
logs and outcome logs are in [bitcoin-lab-12803/](bitcoin-lab-12803/).

The existing writer/cache qualification is unchanged: retain
[`b04fbc603d`](https://github.com/OpenAgentsInc/openagents/commit/b04fbc603d)
and its [record](bitcoin-lab-blocks.md), including 167 writer/metadata/view
mutants and 663 physical journal cuts. Unchanged renderer/discovery, chapter
map and shader results are retained from that record and
[#12802](bitcoin-lab-12802.md), rather than repeated as new work.

## Reproduction

Use a fresh output directory for each command that creates a recording or
native file fixture. `RECORDED` below is the previously qualified normal
recording directory. `SAME` is the equal-height fixture directory.

```sh
coax/web/tool.sh bitcoin/lab/build.coax
coax/web/tool.sh bitcoin/lab/build.coax --sanitize
coax/web/wasm.sh apps/openagents-web/bitcoin-lab.coax apps/openagents-web/bitcoin-lab-face.c apps/openagents-web/build/bitcoin-lab.wasm
coax/web/tool.sh bitcoin/lab/record.coax SAME --same-height
ulimit -s 65536
coax/web/tool.sh bitcoin/lab/check.coax "$PWD/bitcoin/lab/build/sanitize/bitcoin-lab" RECORDED/state.kv SAME/same-b.kv NEW_DIRECTORY
bitcoin/lab/build/sanitize/bitcoin-lab --port 18835 --state RECORDED/state.kv --blocks "$PWD/bitcoin/fixtures/explorer-core-reorg/blk00000.dat" --chain regtest
python3 bitcoin/lab/http-oracle.py http://127.0.0.1:18835
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:18836 --api 18835 --build true
CDP_WEBGPU=swiftshader coax/web/tool.sh coax/web/bitcoin-source-check.coax http://127.0.0.1:18836/worlds/bitcoin-lab
apps/aethyrforge-desktop/build.sh
BITCOIN_LAB_STATE=RECORDED/state.kv BITCOIN_LAB_BLOCKS="$PWD/bitcoin/fixtures/explorer-core-reorg/blk00000.dat" coax/web/tool.sh apps/aethyrforge-desktop/linux-lab-check.coax
coax/web/tool.sh coax/web/bitcoin-lab-native.coax
```

Run each authority gate with `coax/bin/coax mutate FILE --root-only --jobs 6`.
Files are `bitcoin/lab/{request,model,page,origin}.coax` and
`coax/lib/bitcoin/txindex-occurrence.coax`. Baseline dependency checks remain
enabled. Browser capture uses the existing offscreen path because Chromium
152's SwiftShader WebGPU swap chain is unavailable on this Linux desktop.
These captures run headlessly with muted audio and never open a browser window.

The native launcher, API and fetch images install under `~/.openagents/bin`,
with runtime assets under `~/.openagents/aethyrforge/native-linux`. They run
outside this checkout. Models, provider credentials, CLI installations and
Verse OS's separate desktop-4 session are preserved.

## Limits and remaining work

Qualification is public regtest material, with Core/Electrs stopped after
reference capture; no mainnet or Mac/physical-phone qualification is claimed.
The product uses no public explorer, reference node or Python runtime. The
loopback service can be proxied by the existing same-origin `/api` host; no
production service deployment was performed.

The realm's bound excludes the separate native reader process. Its state
allocation is 504,472,048 bytes: bounded journal/query/archive/reply workspaces.
Journal limit is 256 MiB, query tables 64 MiB, reply 2 MiB; window/transaction/
input-output page limits are 24/32/16. Overflow refuses the entire page.
Exact script keys support 128 bytes or a standard address. Unmasked block
files are supported; missing/pruned bytes refuse. Node fees, unavailable
parent receipts and cumulative work remain unknown. Observation/background
time is unknown; no background validator is created by this adapter.

The updated native window was returned to desktop 3; Verse OS remained on
desktop 4 and the previously focused terminal was restored. The existing
Linux host can wait on its swap chain while hidden; stopping the old hidden
window during this update used the unit's ten-second process-group timeout.
Background framing and broader native lifecycle parity are not qualified here.

Full explorer handoff, broader large-block navigation/LOD and reorg animation
remain #12804. No book chapter, discovery flag, wallet policy or money gate is
promoted. Work stops after #12803 at the owner's request.
