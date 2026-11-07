# HTTP ranges and first-image baseline — #13000

Recorded 2026-11-07 in the asset-library worktree, based on
`506a8fb9002699a18f438929f30598e1f2372168`. World content, shaders and the
existing world Wasm binaries were not regenerated for this baseline. This
change adds the HTTP transport and observer; it does not add pack streaming.

## Measured profile

- Apple M5 Max, arm64, macOS 26.4; Chrome 157.0.8103.39,
  `hardware apple metal-3` WebGPU, 1280 × 720 at DPR 1.
- Fresh, muted, headless Chrome profile per realm; browser cache disabled and
  service workers bypassed. OS file and driver caches are not flushed. Other
  owner previews were running; these are single observations, not a controlled
  frame-rate qualification or a statistical latency claim.
- HTTP response rate: **50 decimal Mbit/s per connection**, with a **30 ms
  first-byte delay after request receipt**. Concurrent connections can exceed
  50 Mbit/s in aggregate. This is not a shared-link or packet-RTT simulator.
- First image: the host sets `coaxial.sceneReady` after an admitted scene;
  the observer additionally waits for its WebGPU queue to complete. Loading
  clears do not count. The independent fixture verifies this boundary.
- Network quiet: no outstanding HTTP asset requests and 500 ms without asset
  activity. API requests, Ping beacons, EventSource streams and non-HTTP local
  workers do not hold this condition open. Their observed HTTP transfer bytes
  still contribute to totals. WebSocket frames are outside this HTTP measure.

| Realm | First completed image | Encoded bytes before image | Body bytes before image | Last network activity | HTTP errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| [Everglade](network-first-image-13000/everglade.json) | 5,913 ms | 26,101,734 | 26,060,352 | 5,968 ms | 7 |
| [5E Lab](network-first-image-13000/5e-lab.json) | 2,839 ms | 10,687,434 | 10,677,355 | 2,656 ms | 1 |
| [Character Create](network-first-image-13000/character-creator.json) | 13,378 ms | 49,528,019 | 49,517,658 | 11,846 ms | 1 |

The quiet interval is checked for another 500 ms after activity stops. GPU
completion can follow the last download, as in Character Create. Every run
returned 0, reported scene readiness and zero pending asset requests, and had
no failed transfers or observer overflow. Everglade retained two background
requests. The static server had no API backend; the receipts retain HTTP error
counts rather than claiming complete service health. This baseline qualifies
first-image measurement, not every realm interaction or optional service.

Encoded counts use CDP `Network.dataReceived.encodedDataLength`, with each
completed request's remaining `loadingFinished.encodedDataLength` assigned at
completion. Thus headers or buffered encoded bytes not separately exposed by
Chrome can be assigned later than physical receipt. Body counts separately use
`dataLength`, including chunks of downloads still in flight at the image.
These are browser-observed byte counts, not a packet capture. Milliseconds are
rounded down for display. Events from different connections can arrive out of
timestamp order; the observer retains their timestamps and uses the maximum
for the total, rather than treating the last delivered event as the latest.

## Reproduce

From this checkout's root, with the recorded world binaries/assets present:

```sh
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:9364 --throttle-mbps 50 --latency-ms 30
# In another terminal, one realm at a time:
coax/web/tool.sh first-image http://127.0.0.1:9364/worlds/everglade
coax/web/tool.sh first-image http://127.0.0.1:9364/worlds/5e-lab
coax/web/tool.sh first-image http://127.0.0.1:9364/worlds/character-creator
```

The optional final argument is a timeout in milliseconds, 1,000–600,000;
default 180,000. An absent first image, unfinished assets at the deadline,
oversized URL/request/event tables, browser failure or output overflow fails
instead of publishing a passing measurement. The tables hold 4,096 requests
and 65,536 data/completion events.

## Checks

- `coax test coax/web/http-range.coax`: 2 checks pass; mutation: **92 killed,
  zero survivors**, no equivalent labels. Covers suffix/open/closed ranges,
  empty files, zero suffix, clipping, invalid/multiple ranges, mixed-case units
  and exact signed-64-bit overflow boundaries.
- `coax test coax/web/first-image.coax --only 'first image'` and
  `--only 'network quiet'`: pass. Bytes at the boundary, later transfers,
  out-of-order rows, HTTP assets, local workers, API traffic and beacons.
- `python3 coax/web/tests/serve_network.py`: exact independent HTTP bytes,
  206/416 framing, duplicate fields, If-Range fallback, sparse-file ranges
  beyond 3 GiB, invalid CLI values, concurrent shaping and API forwarding.
  Two concurrent 1 MiB responses at 8 Mbit/s and 120 ms finish in **1.384 s**.
- The same oracle with `SERVE_BINARY=/tmp/13000-serve-sanitized`, built with
  AddressSanitizer and UBSan: passes, no sanitizer errors; concurrent responses
  **1.450 s**. Each measured header delay is at least 100 ms and each full
  response is between 1.10 and 1.90 s.
- `python3 coax/web/tests/first_image_oracle.py`: real WebGPU fixture with a
  524,288-byte download still running when a red frame completes, followed by
  a separate 16,384-byte transfer. Passes: first image **256 ms**, body bytes
  before it **41,831**, encoded bytes before it **38,244**, encoded total
  **542,607**, quiet activity **2,353 ms**. This catches completed-resource-only
  accounting, loading-clear attribution and including future transfers.
- `coax/build.sh`: installed the existing checked compiler for these sources
  from the local cache; no compiler source changes in this issue.
- Native and deployment builds both link the same Coax-generated range
  implementation; no separately authored C range selector.
- `coax/web/tool.sh tools-check`: 1,859 tracked tool files, zero new files
  without a firm reason. Shell syntax and staged whitespace checks pass.

## Served identities

SHA-256 of the principal existing consumer artifacts (not an assertion that
every build file was freshly rebuilt from the source commit):

| Path | SHA-256 |
| --- | --- |
| `coax/bin/coax` | `52402aa1520ff7f267ce7c44b3d3dfa8e9dacc868bf7d84d95cdf95a0586d7f1` |
| `coax/web/host.js` | `451a3b475fdf03c049c93ac1ba33e2df99ab01364f50a5517cac7cdb4d1b12e1` |
| `apps/openagents-web/build/everglade5e.wasm` | `a613933622fd781d4c6a97deab5c857abd8f3e7a627bbd97119f9702648072b2` |
| `apps/openagents-web/build/fivelab.wasm` | `5ce387d67eac4696e876382e40dc2648dbf1cf2a581d88e3a9b8a70d0419b9dc` |
| `apps/openagents-web/everglade.wgsl` | `e32829c5909b2ba5d7f8d333178b5a8c57fe730b804c1d8fd66038df1367e3eb` |
| `apps/openagents-web/fivelab.wgsl` | `6ef020a1c43f2577ccc95d987d327c3f5b6389ea69296d7f3d162b8b53833226` |
| `apps/openagents-web/charactercreator.wgsl` | `2cdc0f9ec115bd6f3f3799189fc2bd077616db70fafe9f1f96130e7e5bb0fe82` |
| `apps/openagents-web/realms/everglade/assets/everglade.pack` | `1c3fcbd6af5bac6b05639cab360725b798c3eb2ff16df0f71512ea68cf7433bc` |
| `apps/openagents-web/realms/everglade/assets/everglade-avatar.pack` | `65a9d471559f6b6189c72f5e033dab6e047da58720961b51aaa8a0efd8179ea1` |
| `apps/openagents-web/realms/character-creator/assets/glade.pack` | `1a5074593e311d54a45c59d723a3bb00ba1fcad6535cb5baf361e77fd5167064` |

No asset payloads or screenshots were added by this change.
