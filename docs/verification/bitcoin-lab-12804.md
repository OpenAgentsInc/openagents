# Bitcoin Lab #12804: explorer, exact UTXOs and admitted replay

Implementation: [`b4004c3fd5`](https://github.com/OpenAgentsInc/openagents/commit/b4004c3fd5ad52d7e92f2065f1701e92e70a7c40).
Qualified on coderos-4080, Linux/NVIDIA RTX 4080, November 5, 2026.
Compiler SHA-256:
`54308753598854075405f995a04eb94f64eeb85f231ee6840a42e77afe6d2e80`.
[Logs, captures and hashes](bitcoin-lab-12804/) accompany this record.
The implementation was rebased on `8182e4f84e`; the new process effects and
browser lifecycle host were then checked with the real native/browser paths.
A final rebase on `b617f9cc21` preserves every Lab source byte; its sole
additional runtime change retains child stderr after wait, checked through the
native host harness. Both qualified and final source hashes are retained. The last fast-forward
rebase includes `1af44e813c`'s unrelated offline-room tool port; the compared
Lab and runtime source bytes are unchanged.
No Bitcoin validation, money policy, published explorer v1 contract, chapter
status or discovery flag was promoted.

The owner retained original 02/30 (#12803), then 03/30 (#12804), and canceled
the other open Bitcoin Lab work. #12803's snapshot adapter is preserved.
This completes the reduced explorer scope, not the withdrawn 30-step book plan.

## Result

The native desktop app and browser realm share one Coax producer, exact
selection and admission model. EXPLORER opens the ordinary table inside the
same app; GRID and BACK preserve the admitted object, page and selected outpoint.
The separate published pow-only explorer v1 remains unchanged. BLOCK shows
full metadata; TXS/DETAILS, INPUTS, OUTPUTS and SCRIPT expose exact facts.
HISTORY matches the entire script. UTXOS separates unspent/immature coins
from historical/unspendable occurrences, without ownership or change inference.

Block slabs have active/stale lanes. Input/output edges appear only after
expanding the exact transaction occurrence; a transaction list has no implied
flow through its default row. Transaction tile width is page-relative
weight with a declared 12% visibility floor. Fee-rate colors use exact integer
bands against rounded virtual size `(weight+3)/4`; unknown fees are gray.
A/B/C scrubs acknowledged fixture states; PLAY/PAUSE and STEP control replay.
Detach/attach geometry starts after complete admission; integer amounts never
interpolate. MOTION disables interpolation. STATUS shows both full cumulative
work values. A detached selection remains the exact stale occurrence; missing
objects and incompatible saved cursors are explicitly unavailable.

Closed `lab1/` links bind source, fixture phase, network, cursor, query kind,
exact key/occurrence, page, row, outpoint and table/Grid presentation. Browser
fragments and the native public selection file share the same parser. Native
LINK copies through `wl-copy`, saves atomically under
`~/.openagents/aethyrforge/selection.txt`, and restores unless startup explicitly
selects a source. The token cannot name an upstream or filesystem path.

## Actual checks

| Check | Result |
| --- | --- |
| New public-link authority | 120 mutants killed, zero survivors; no equivalence annotations |
| Independent retained Core/Electrs/raw HTTP oracle | 928 responses pass: A/B/C and configured validated C, exact IDs/amounts/weights/occurrences/script balances |
| Independent large raw/HTTP oracle | 25 responses pass: 109 blocks, 72 transactions, 40 inputs/outputs across three pages, 181 script outputs, exact spend links and 10,000,000,000-sat balance |
| Large recording | Mined height-105 child of C, admitted through the full validator; 7,167-byte block, 157,475-byte final table; original A/B/C JSON stays byte-identical |
| Native ASan/UBSan producer | 1,633 actual HTTP graph/table/layout frames, including history and UTXOs; peak 33,672 vertices, zero budget refusals or sanitizer diagnostics |
| Native ASan/UBSan API/files | Both reorg directions, equal-height branch changes, stale cursors, five torn journals, corrupt checksum, 256 MiB refusal, unavailable bodies/recovery and genuine L origin retaining pow-only pass |
| Actual Vulkan desktop host | RTX 4080, three passes, 1440×900, 2,000 frames; Tab/Enter reaches LINK through the native C face, persists a valid public selection and exits owned children |
| Native process integration | Real owned API/fetch children load fixtures, change branches, select the explicit node journal, clear amounts, save selection and reap owned processes |
| Focused Coax checks | Eight realm checks, five native input checks, exact breadcrumbs/strict restore, stale occurrence replay, history↔UTXO script identity, a11y amounts and rounded-vbyte fee colors pass |
| Font/virtual table | Every printable ASCII glyph fits actual font metrics at 320/390/1440 widths; full hashes/scripts wrap without omission; overlapping row pages preserve whole touch targets |
| Muted headless browser | Desktop and 390×844 phone, one open session per viewport: real fetch, FIND, graph/table/back/link, exact parent/spender IDs, rendered 2,000,000,000-sat output and 2,999,998,000-sat Core/Electrs script balance, real keyboard and mouse/touch, reduced motion, A→B→C→B→A, cancellation/rescue and GPU recovery pass |
| Protocol | Pinned protoc 29.3/protobuf 5.29.3: 30 tool tests, all 30 owned schemas and published compatibility/migration/consumer profiles pass; additive block metadata descriptor regenerated |
| Repository tools | 21 unrelated existing missing-reason headers after rebase; none names an owned file |

The realm state is 8,690,256 bytes; the measured arena is 12,146,608 bytes,
within its 16 MiB declaration. Geometry stays under the 70,000-vertex budget
and 4,096 mesh-vertex cap. The final Wasm is 344,799 bytes, within its updated
365,000-byte budget; the ordinary table/navigation/replay account for this
increase from 315,000. Only visible text rows produce geometry.

Unchanged authority evidence is retained from
[#12803](bitcoin-lab-12803.md): 617 zero-survivor route/identity/page/origin/exact-
occurrence mutants. The writer/reader qualification is retained from
[the block milestone](bitcoin-lab-blocks.md), including 167 mutants and 663
physical cuts. Renderer, shader, discovery and chapter-map results remain
those of [#12802](bitcoin-lab-12802.md); those unchanged gates were not repeated.

## Reproduction

Use fresh directories for recorder/file fixtures. Each command is bounded to
under 15 minutes. `NORMAL` is the retained A/B/C recording directory; `SAME`
is the equal-height recording directory from #12803.

```sh
coax/bin/coax test bitcoin/lab/explorer-check.coax --run-steps 100000000 --only 'Lab '
coax/bin/coax test apps/openagents-web/bitcoin-lab.coax --only 'Bitcoin Lab'
coax/bin/coax test apps/aethyrforge-desktop/linux-native.coax --only 'native Aethyrforge'
coax/bin/coax mutate bitcoin/lab/selection.coax --root-only --jobs 6
coax/web/tool.sh bitcoin/lab/build.coax --sanitize
coax/web/tool.sh bitcoin/lab/stress-record.coax LARGE
ulimit -s 65536
coax/web/tool.sh bitcoin/lab/check.coax "$PWD/bitcoin/lab/build/sanitize/bitcoin-lab" NORMAL/state.kv SAME/same-b.kv FILE_CHECK
bitcoin/lab/build/sanitize/bitcoin-lab --port 18845 --state NORMAL/state.kv --blocks "$PWD/bitcoin/fixtures/explorer-core-reorg/blk00000.dat" --chain regtest
python3 bitcoin/lab/http-oracle.py http://127.0.0.1:18845
bitcoin/lab/build/sanitize/bitcoin-lab --port 18847 --state LARGE/state.kv --blocks LARGE/blk00000.dat --chain regtest
python3 bitcoin/lab/stress-oracle.py http://127.0.0.1:18847 LARGE/blk00000.dat
coax/web/tool.sh bitcoin/lab/scene-check.coax 18847
coax/web/wasm.sh apps/openagents-web/bitcoin-lab.coax apps/openagents-web/bitcoin-lab-face.c apps/openagents-web/build/bitcoin-lab.wasm
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:18866 --api 18845 --build true
CDP_WEBGPU=swiftshader coax/web/tool.sh coax/web/bitcoin-explorer-check.coax http://127.0.0.1:18866/worlds/bitcoin-lab
apps/aethyrforge-desktop/build.sh
AETHYRFORGE_LAB_SELECTION_FILE=/tmp/fresh-selection.txt coax/web/tool.sh apps/aethyrforge-desktop/linux-lab-link-check.coax
```

For the sanitized scene run, emit `scene-check.coax --library`, then link its C
with `tool-native.c`, `process-native.c`, `store-file-native.c`,
`read-file-native.c`, `socket-native.c`, `entropy-native.c`,
`entropy-foreign.c` and the generated entropy unit. Flags:
`-std=c11 -D_DEFAULT_SOURCE -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer`;
link `-lpthread -lm`. This uses the same Coax tool entry, without a second
implementation. Source/artifact hashes pin the qualified binaries and inputs.

Browser capture is muted/headless and uses the existing offscreen RGBA path:
Chromium 152's SwiftShader swap chain is unavailable on this Linux desktop.
The native window was refreshed on desktop 3; the separate Verse desktop-4
window stayed in place and terminal focus was restored. Installed binaries,
API/fetch helpers and runtime assets live outside the checkout. The hidden
old window required the existing service's ten-second stop timeout.

## Limits

Qualification uses public regtest material, not mainnet, Mac or a physical
phone. The 72-transaction fixture is separately labeled synthetic qualification
material, mined and fully validated after the retained Core/Electrs C state.
Reference implementations and Python are independent test oracles, never a
product/runtime dependency.

The view is bounded: 24 blocks, 32 transactions and 16 inputs/outputs per page;
reply limit 2 MiB, script query keys 128 bytes, 12 navigation breadcrumbs.
Large tables/graphs page instead of implying whole-chain completeness.
Long public scripts are readable through text rows; scripts over the query
key limit refuse history requests explicitly. Oversized/incomplete replies
refuse wholly. Own-node fees, parent receipts and cumulative work remain
explicitly unknown where this adapter has no authenticated derivation;
observation/background times remain unknown. No profile is upgraded from
pow-only, and no money decision is made from this viewer.

The separate reader process retains #12803's 504,472,048-byte bounded workspace,
256 MiB journal and 64 MiB query-table limits. Broader native hidden-window
lifecycle behavior remains outside this issue. No book, wallet, Lightning,
public-listing or complete 30-step-epic claim is made.
