# Bounded browser pack component

This is component evidence for the v1 reader. Ordinary `host.js` pack loading,
physical HTTP cancellation and player/recovery composition are separate exits;
the desktop default and public Web opt-in are unchanged.

`pack-stream.coax` bounds each VPK1/encoded stream to 64 MiB and separately
charges live allocations in 64 KiB pages, at most 256 MiB/64 tickets. The
private metadata reader admits at most 4,096 bytes, requires bounded same-site
source identity, and checks plaintext SHA before the existing Coax `pack_read`
admits the complete layout, catalogue and licences. No prefix is published.
The stream/private/actual-observation root gates kill 155/132/72 mutants,
with zero survivors and no equivalent annotations.

The real Chrome component uses actual ReadableStream cancellation,
WebCrypto AES-CTR and gzip decompression with an explicitly mocked fetch
source. It checks split twelve-byte headers, identical public/private entry
bytes, seven whole-pack refusals, and retirement during a held reader cancel,
crypto completion and asynchronous catalogue borrow. A retired borrow cannot
publish its returned result. Thirty-one simultaneous reader instances plus
the retained master exhaust the 256 MiB budget; the next offer refuses before
another Wasm instance is created. Every operation then releases its tickets;
the page's master retains its measured 8 MiB workspace. The complete observed
rows and unchanged source fence are in `browser-pack-component/`.

An SDK/OS/GC memory or network-buffer bound is not inferred from these
application ownership observations. Tickets remain live until actual pending
SDK completion, and old Wasm views are detached on disposal. The component
does not use a realm, GPU or audio device.

The release reactor is 34,411 bytes, SHA-256
`4cd63e8f2c02a9f15ed48ad74d45c25cf7e45c8e0e6a5d88ff9b1f3019c43ffd`.
It was built through unchanged `wasm.sh` with the original admitted b587
0.50 image in the genuine protected 73b root, with separately overlaid product
inputs. Original receipt verification passes before and after. All thirteen
actual generated-C source paths/hashes match both the prepared root and this
product worktree. The genuine C/header cache key is
`23d256c7eebe241b67b115b3a42ca029ed07294cf4a3a23e52207adaed4acb3a`;
their SHA-256 values are respectively
`b7dae02e4da1edf6c2d5f0c72bfaafc35db9903515e1be916c992f00def6a8d9`
and `3efc52d331e917e439d09e7d4682bc146922d8eb1b8e60d747c3ef0b92301d57`.
No original-emission profile sidecar or current compiler release is inferred.

Durable artifact root:
`~/.openagents/aethyrforge/metal-host-12868/browser-packs/`.
`browser-reservation/` is the final SDK run; `emission/` retains genuine C,
header and source list. Earlier failed baseline/copy/constructor attempts and
earlier passing adapter revisions remain there with their own identities.
