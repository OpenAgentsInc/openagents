# Coax QR module grid

`#load "coax/lib/qr/qr-memory.coax";` supplies:

```coax
qr_encode(input: []u8, out: []u8) -> QrResult
qr_encode_mask(input: []u8, out: []u8, mask: int) -> QrResult
```

The second entry point accepts `-1` for automatic selection, or a fixed mask
0–7 for interoperability tests. `QrResult` has `status`, `version`, `size`,
`count`, `mask`, `penalty`. Success is status0. `count = size*size`; the first
count bytes of out are row-major modules, indexed `y*size+x`: **1 dark, 0 light**.
No quiet zone or colors are embedded. Render a light square with at least
**four light modules** around every edge, integer pixel scaling and no smoothing.
Draw dark modules as Coax quads; no DOM/canvas-text overlay is needed.

The roughly70-byte `/signin` URL fits version5/M, size37 and1369 output bytes.
A5929-byte output buffer accommodates every supported version. Byte mode,
error correction **M**, versions **1–15**, maximum **412 input bytes**. No ECC
boost, ECI, text normalization, numeric/alphanumeric segmentation, logos or
structured append is implied. Text callers explicitly supply UTF-8 bytes.
This encoder never authenticates a URL or signs/approves a request.

Statuses: input too long1, insufficient output2, unsupported mask3, physical
input/output overlap4. Refusals preserve output and return zero metadata.
Success leaves bytes beyond count unchanged. The memory adapter observes real
aliases; pure admission remains in `profile.coax`. Caller views must remain
live, stable and writable as appropriate throughout the call.

All encoding is Coax: version selection, segment bits/padding, GF(256)
Reed–Solomon, block interleaving, function/version/format patterns, zigzag,
eight masks and N1–N4 scoring. Ties choose the lowest mask. The scaled finder
pattern scorer includes the light border convention used by the primary
[Project Nayuki reference](https://www.nayuki.io/page/qr-code-generator-library);
its MIT notice is retained in `LICENSE-NAYUKI`. Explicit work storage is13168
bytes plus the820-byte block table and small RS locals; there is no heap
allocation in the encoder. This is not a claim about all compiler/ABI copies.

## Evidence

```sh
coax/bin/coax test coax/lib/qr/qr.coax
coax/bin/coax mutate coax/lib/qr/profile.coax
PATH=/opt/homebrew/opt/lld/bin:$PATH python3 coax/tests/qr/test_qr.py
```

Nine loaded Coax checks pass; the six resource-policy checks kill74/74
mutants without exemptions. Encoding math is separately checked against
**20 frozen primary reference grids**, covering all15 versions, the70-byte
sign-in URL and a300-byte `nostrconnect` URI with a dummy public test identifier.
Exact modules, selected masks and penalties agree in checked and release
native C and actual Wasm. Four targeted tests cover those grids, actual aliases,
capacity/mask refusals, guards and an ASan/UBSan executable.

`coax/tests/qr/capture_reference.ts` optionally refreshes fixtures from the
primary reference's public live demo, isolated in a temporary headless browser;
it records the reference JavaScript SHA-256. Native tools never import or run
that downloaded source. Normal tests are offline and need no reference network.
Python is explicit test tooling, never an app/compiler build dependency.

The iOS scanner fixture also runs the **actual Vision decoder on iOS26.5
Simulator**, through CVPixelBuffer frames rendered from generated Coax grids.
All19 nonempty reference strings decode exactly, including70/300-byte URLs.
The empty byte string remains a reference-grid test. Run it with:

```sh
python3 coax/tests/qr/ios/run.py
```

It builds a standalone probe, creates/boots an owned temporary Simulator
without opening a window, observes the result and deletes only that device.
It neither edits the real app nor needs a TestFlight build. The optional thin
`platform/QRFrameDecoder.swift` adapter performs Vision byte observation only;
camera permission, frame ownership and Coax URL/request admission are callers'
responsibilities. It selects the public QR-capable revision1 explicitly: the
Simulator's newer default detector refused with Vision error9 (inference
context unavailable), and that failed trial was not treated as evidence.

Physical camera optics and phone sign-in end-to-end are **not** established by
rendered-frame decoding. They remain in the owner/device run for
`phone-sign-in-e2e`. This source delivery unblocks the page renderer. Key custody,
challenge/link endpoints, `/signin` and TestFlight remain the fast-path owner's
work. Full NIP-46 remains in the Nostr epic.
