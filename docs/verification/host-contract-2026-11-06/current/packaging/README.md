# Current room packaging

The desktop worker job now invokes the existing Coax `worker.coax` tool.
The previous `worker.sh` path does not exist; `desktop-worker-red.log`
retains its status 127 refusal (#12978).

The admitted Linux Coax 0.52 compiler emits the six room units. The ordinary
worker builder retains their original C and headers with `COAX_ROOM_KEEP_C`;
`room-c-sources.txt` identifies their compiler and source cache keys. No cache
key, compiler receipt or admission rule changes. On macOS, the compile/link
portion of `coax/web/room/build.sh`, starting at `strict=`, consumes these
portable units. Its actual `--start-check` passes. The separate ordinary Mac
Coax emission reaches its fourteen-minute limit and is not a passing build.

The Linux Binaryen `-O2` worker measures 1,062,884 bytes against the unchanged
1,060,000-byte budget (#12980). Size optimization (`-Os`) produces
1,042,053 bytes through the ordinary worker builder and passes the canonical
`everglade-room.wasm` budget. The real room and this worker pass the local
Everglade drift probes with two background bots, on private ports 9340–9344:

```sh
coax/web/tool.sh everglade-drift --profiles local --bots 2 --port 9340 \
  --no-raid --out /tmp/oa-12869-worker-os-drift
```

The host's observed rolling tick p95 is 405 microseconds (p99 470). This first
record covers the local profile without the raid; full-sample tick qualification
and the final production web artifact are recorded separately.
