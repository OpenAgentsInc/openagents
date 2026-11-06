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

## Final worker

The `-Os` candidate is rejected after full-sample timing failures. The final
ordinary builder uses Binaryen `-O3` and measures 1,042,322 bytes, within the
same 1,060,000-byte budget. It preserves all Coax integer checks, the existing
river-specific C optimization and strict floating-point flags.

The final Linux replay passes every gameplay drift probe at local, 100 ms
and 250 ms with zero stalls or dropped beat. Measured worker p95 is
810/910/956 microseconds, within the host contract's 11,000-microsecond budget.
The additional `--tick-us 600` qualification fails; the command exits 1.
The contended Mac also has a 250 ms NPC-condition mismatch. Raw logs and tick
samples are in [production](../production/). These additional performance and
contention observations remain open in #12981; they are not green results.
