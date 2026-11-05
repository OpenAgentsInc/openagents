# Actual native HTTP redirect and local reconnect projection

The Metal URLSession adapter now consumes both shared Coax predicates:
`hhr_follow(version)` in its redirect callback and
`hhr_response(version,status)` before accepting response bytes. Version 1
refuses 301/302/303/307/308, including final responses without Location.
The request is marked failed before returning to URLSession, so its final
3xx body cannot become a successful prefix. Pre-v1 keeps its earlier path.

`metal-http-redirect.coax` drives the actual production HTTP imports,
transport owners, URLSession delegate and recording face. The unchanged
common `hhrc_accept` admits both normal and AddressSanitizer observations:
`[30,30,15,15,15,15,15,0,0,1,1,1,1]`. All 45 original physical requests are
retained. The 30 direct SDK requests cover each status, same/cross-origin
Location and no Location, with GET and POST. All 15 POSTs retain the exact
fixture authorization/player headers and body; GETs contain none. All 15
face commands receive the exact two-byte `0\n` callback. Neither destination
receives a follow-up. The separate 200 control succeeds, owners reuse slot
zero, and no trace is dropped. No production endpoint or credential is used.

The first harness run used the same `sdk.json` name for process metadata
and callback output. Process metadata overwrote the latter, and the gate
correctly failed despite a peer showing 45 requests and no follow-ups.
The corrected run uses separate filenames; the failed attempt remains in
`http-normal/`. The initial missing Basic import failed before execution
and is also retained. All 235 driver/dependency checks pass.

The same regenerated native facade consumes the public exact reconnect
mapping. `metal-conformance --local-reconnect` opens
`local:/api/room/relay/everglade/local-1?re=1` through the actual raw worker,
asks its private peer to echo the physical request path, echoes UTF-8 and
closes. All nine command/result/callback rows match without normalization
in normal and instrumented runs. The new complete-row observation helper
kills 21/21 root mutants, with no survivors or equivalents. Existing route
60/60 and local-target 11/11 policy gates from `117245d2af` are reused;
no broader URI/origin admission is added.

The regenerated facade has 86 actual source-table entries, all byte-equal
between the frozen `http-source/` closure and the observed worktree inputs.
The compiler is the original admitted b587 Mac 0.50 image, verified in the
genuine `73b60ebdf5` root. The later terminal-notice compiler-source change
correctly makes its original receipt fail against current main; no receipt
is rewritten and no current-compiler release claim is made. This is a
separately pinned product emission, not a new primary realm emission.

The instrumented run covers the new generated facade, native SDK,
Swift adapter/driver, recording Coax producer and its face. The unchanged
instrumented texture object is reused. Apple framework leak detection is
disabled; this is an AddressSanitizer result, not a LeakSanitizer claim.
Both drivers exit successfully with no sanitizer report. The raw socket
worker's own prior pressure/TLS/instrumented gates remain separately scoped.

Raw plans, callbacks, physical requests, build logs and original receipt
checks remain under
`~/.openagents/aethyrforge/metal-host-12868/graphics-admitted-3f8/` in
`http-{corrected,asan}/`, `reconnect-{normal,asan}/`, `http-sdk/`,
`http-asan-sdk/` and `http-source/`. This directory commits both small
redirect receipts, both complete reconnect traces, the source join and the
exact C/header/object/driver/fixture SHA manifest. Full current realm
packaging, integrated ordinary session gates and the three-host default
remain separate. Neither issue is closed and WebKit stays the default.
