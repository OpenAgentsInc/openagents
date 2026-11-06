# Browser HTTP response bytes

The version 1 contract delivers ASCII `STATUS\n` plus raw body bytes. The
ordinary unbounded import permits malformed UTF-8; the inbox clips its byte
sequence at capacity. Bounded requests require valid UTF-8 and a whole
response inside the inbox. Linux and Metal already hand owned bytes to the
shared `ht_http_finish`; Metal's Swift UTF-8 check supplies a validity fact
without changing its `Data` payload. No native implementation changed here.

The original browser used `response.text()` for ordinary replies and then
encoded the string again. Its actual import/inbox receipt showed:

| Response | Before | After |
| --- | --- | --- |
| body `ff` | replacement character, 7 total bytes | original `ff`, 5 bytes |
| body starts `ef bb bf` | BOM stripped, 11 bytes | original BOM, 14 bytes |
| 129-byte ordinary response, 128-byte inbox | status only, 4 bytes | original first 128 bytes |
| bounded malformed/oversized response | `0\n` | `0\n` |

The browser now reads ordinary v1 responses as SDK bytes. Bounded v1
responses validate the complete body but retain its original bytes, including
the BOM. The ordinary inbox path accepts those bytes directly. The existing
Coax owner/count/whole-bounded-response authority remains unchanged.
Pre-v1 helpers keep their previous text behavior.

The Coax probe extends the common fixture without editing that fixture or
its native consumers. It calls the actual `coaxial_http` and bounded imports
through ordinary host.js and observes the producer's inbox length/FNV hash,
returned slot, exactly one callback and a following producer frame. The real
loopback peer serves malformed, valid multilingual, BOM and oversized bodies.
Eight actual host requests plus two actual pre-v1 helper requests require all
**66 fields**, including a valid request after refusals. The original red
and corrected green are retained in [`http-bytes/`](http-bytes/). Both source
fences are unchanged during their respective runs; green passes all66.
The headless Chrome is muted and opens no visible window. Its owned process
and the private loopback services are stopped after capture.

Replay with a fresh private site and unused ports (the recorded run used
61285/61286):

```sh
COAX_JOBS=1 COAX_CHECK_JOBS=2 COAX=/path/to/admitted/coax \
  nice -n15 coax/web/wasm.sh aethyr/host/http-bytes-fixture.coax \
  aethyr/host/tests/http-bytes-fixture-face.c /tmp/http-bytes-site/fixture.wasm
```

Supply the existing fixture WGSL and qualified `build/host-services.wasm`
and `build/host-sockets.wasm`. Serve the directory with ordinary
`coax/web/serve.sh --host-dir "$PWD/coax/web" --api PEER_PORT` and a page
loading host.js with `data-wasm=/fixture.wasm data-wgsl=/fixture.wgsl`.
Run `coax/web/tool.sh aethyr/host/tests/network.coax PEER_PORT` for the finite
private peer. Then the one-command observation is:

```sh
COAX_JOBS=1 COAX_CHECK_JOBS=2 COAX=/path/to/admitted/coax \
  nice -n15 coax/web/tool.sh aethyr/host/check-browser-http-bytes.coax \
  'http://127.0.0.1:PORT/?probe&probe-manual' /tmp/http-bytes-new.json
```

The probe is 28,540 bytes, SHA-256
`ae25133787f08c2fa7792cfc5f3c5eab39c520f3f1b4947cccda63c354c16f8f`.
The actual fetched services/socket companions and WGSL hashes are in each
receipt. The genuine Coax emission uses the admitted Mac 0.50 compiler
`b5872254…`, with unchanged original compiler admission scope; its build
timings/argv and emitted-source dependency record accompany the fixture.
The receipt authority kills **73/73** root mutants and the additive fixture
kills **3/3**, without equivalents. All **42** network fixture checks pass.
The first BOM check called a clock-bearing fixture routine and correctly
refused compile-time host effects; separating the pure byte projection from
its clock effect fixed that check without changing existing peer routes.

This component supplies no new world, performance, default or deployment
claim. The final three-host/package/default gate remains separate.
