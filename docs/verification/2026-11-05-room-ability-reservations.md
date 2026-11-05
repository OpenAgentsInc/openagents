# Reserved ability and interaction service

Issue #12841, another tested part. Source is `c1a817b304` plus the change
`Reserve ability ends and interaction releases`. This does not qualify the
remaining downstream town projectiles, per-room CPU/admission budgets, content
warming, or integrated saturation. The work classes remain in
[`room-overload.md`](../verse/room-overload.md).

Every seat owns two world ability starts and four world ability ends between
world service calls. The 96-entry queue covers all sixteen seats. Starts and
ends have independent reservations. Overloaded starts are refused before
cooldown/resources; overloaded ends preserve concentration. The fifth end
waiting behind movement receives `r SEQ busy` instead of jumping ahead of
that movement. These are explicit refusals, not silently accepted commands.

Each seat also owns one door/talk/boat interaction and one release per tick.
The 32-entry queue lets a seat start and stop talking before the next world
step without crowding out another seat. A flood of interaction starts cannot
consume the release reservation. Extras use the existing busy response.

The new integrated overload regressions first failed against the preceding
implementation. The policy authority kills 34/34 mutants, zero survivors and
no equivalents. The complete room suite passes 595/595. The release worker is
1,023,205 / 1,060,000 bytes; `tools-check` passes. Raw logs are retained in
[`room-ability-reservations-2026-11-05/`](room-ability-reservations-2026-11-05/).
The original full
suite caught an interaction regression when starts and releases shared one
reservation; the independent release reservation fixes it without weakening
the door/talk replication check.

Rerun the bounded checks separately:

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate coax/web/room/effect-budget.coax --jobs 1
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/worker.coax /tmp/ability-reservations/everglade-room.wasm
nice -n 15 env COAX_JOBS=1 coax/web/tool.sh tools-check
```

Worker verification also fixes its source-cache path: repository-relative
source names now hash their actual file, rather than the repository directory.
Absolute source names retain their original identity. This changes build reuse,
not gameplay or the release size limit.
