# MMO fault matrix, profile 2

GitHub #12842. Local isolated API/Postgres, actual native room processes and
headless muted Chrome. The bounded Coax entry is `coax/web/room/mmo-fault.coax`.
It reuses the checkpoint, drain, background and browser feature harnesses;
it does not reimplement the server's simulation or persistence decisions.

## Measured components

The room component is source **9b1abcd354**, checked Coax 0.50 room C/header,
with the unchanged 140-source world component emitted by Coax 0.48. Native
image SHA-256 **19456fe570906a245016d4a71d83f346b1005b34c9acf9ccfff1d7527b65ff21**.
Apple Clang 21, strict native room math, `-O2`, Apple M5 Max, 18 logical CPUs,
128 GiB, macOS 26.4. Tools/builds are nice 15 with one build worker. Linux's
genuine final checked 0.50 compiler independently emitted exactly the same
room C and header bytes as the Mac's original stage0 emission: C
`60b4f148118e5c2a2a10189b44a56d69c35ee90fbadc5c537eb4f5e2a6586061`, header
`88e3bf7f6ddb6a07ef310e0d560aa855b1aa0f7b17a02c08a402fc51ac6e5ebb`.
This identifies the component profile; it is not a full all-0.50 world build.

Tool compiler SHA-256
`b5872254e02de4b2fb84f039e81f602090f5526301c036c077004294a7e580eb`, with its
genuine source-bound receipt verified against the captured pre-199402193d
compiler inputs. Later terminal-notice compiler edits are outside that receipt.
No compiler source, admission policy or cache receipt is edited by this work.
The unchanged room sources retain their **659/659** room-suite result recorded
in the [room latency record](2026-11-05-room-latency.md).

## Results and injection barriers

Raw logs are in [mmo-fault-2026-11-05/](mmo-fault-2026-11-05/).

| Slice | Actual injection and consumer assertion | Result |
| --- | --- | --- |
| Durable checkpoint | Signed partial body refused; changed body under one operation conflicts; newer revision arrives before older; duplicate preserves original acceptance | Passed |
| Commit/reply crash | Separate database connection observes before/after COMMIT; kill before reply; retry cannot duplicate accepted progression; full pose/resources/death restore | Passed |
| Outage/old owner | More than 38 s unavailable; affected character pauses within 901 ticks, movement genuinely unsaved; healthy restore within 60 s; SIGSTOP/SIGCONT obsolete owner gets a fenced refusal | Passed |
| Shutdown | Final frozen capture accepted before successful SIGTERM exit; unavailable final save exits failure | Passed |
| Outbox capacity | All 256 immutable unaccepted captures survive disconnected grace bodies; 257th character refused despite free simulation seat | **44/44** combined checkpoint checks, exit 0 |
| Deploy overlap | After signed drain and saved-body barrier, replacement is ready while drained old process remains healthy; old SIGTERM; new owner rejoins and saves HP/XP | **9/9**, 23.208 s; selected scratch API parent check exit 0 |
| Failed readiness | Published world stalls; health works while all three cold socket paths refuse; publication then enables ready, sharded connection and actual state frame | Passed, **12.205 s** |
| Isolation | Mixed fight, six poster lanes, 35 s outage, saturated posts, slow socket/rejoin, independent healthy room, restored publication | Passed, **85.262 s** |
| Browser cast/socket cut | After B hears accepted Meteor Swarm, close A's socket during the 2.5 s bar; keep ticketed grace, same seat on a new socket, exactly one observed cast and no false departure | Passed |
| Browser partial snapshot | Deliver actual wire first/more snapshot to the actual socket callback, silence subsequent traffic, apply 14 producer frames, then close and reconnect | Passed; whole tick stays **323**, incomplete tick **1000323** never accepted; recovered tick **366** and exactly one other drawn body per page |

Isolation sent **1,040 casts**, with peaks **8 pets / 4 zones / 37 raiders**.
The independent room received **2,251 frames**, maximum gap **66 ms**, all
**150 health requests** succeeded. The slow socket closed and rejoined;
pending posts peaked at **3/6**, recovery and publication both succeeded.
These loaded-host samples qualify the stated assertions, not fleet capacity.

Profile seed is **1284201**. Crash, drain, ownership and partial-message
injections occur at printed state barriers, not a guessed sleep into a commit.
Wall-clock identifiers and temporary ports are recorded by their feature
harnesses; a fixed seed does not make process scheduling deterministic.

The orchestration authority has **39/39 killed mutants**, zero survivors and
no equivalent exclusions. Its full tool check has **102/102** passing checks,
including refusal of failed starts, nonzero exits, deadlines, truncated output
and missing success markers. Each slice writes complete bounded stdout/stderr
and prints the native image hash, declared source label, seed, elapsed time,
exit status, timeout and truncation state. Labels are not emission receipts.

## Repeatable local and pre-release qualification

Run from an owned worktree using a completed, source-identified native image.
Use a new output directory for each slice and retain its outer report as well
as `stdout.log` and `stderr.log`. No production environment is required.

```
coax/web/tool.sh coax/web/room/mmo-fault.coax BIN SOURCE_LABEL OUT/durable durable
coax/web/tool.sh coax/web/room/mmo-fault.coax BIN SOURCE_LABEL OUT/overlap overlap
coax/web/tool.sh coax/web/room/mmo-fault.coax BIN SOURCE_LABEL OUT/startup startup
coax/web/tool.sh coax/web/room/mmo-fault.coax BIN SOURCE_LABEL OUT/isolation isolation
```

Deadlines are respectively 14, 10, 2 and 3 minutes, including the underlying
feature command. Run slices separately. `durable` selects the scratch API's
combined checkpoint runtime/capacity mode; `overlap` selects its existing
drain/deploy mode. Both API modes were measured directly before the wrapper
landed; startup and isolation were measured through the wrapper itself.
No unchanged passing API run is repeated merely to confirm the wrapper.

Browser interruption qualification reuses the pair harness's loopback-only
`--faults` mode, with the normal local room and static page servers already
running. Use local fixtures with levels off (the existing `local.coax --levels
off` mode), because Meteor Swarm is intentionally a high-level spell:

```
coax/web/tool.sh everglade-pair \
  'http://127.0.0.1:61252/realms/everglade/?relay=ws://localhost:61251' \
  --faults --size 800x600
```

The page is the pinned **17a742c08c** primary producer, release Wasm
`1e0abdfa8953cdd888f8646e14302e5213284f231339882f911cd0e652d4416d`, with
the #12840 captured host. The actual ordinary Coax receiver and rendered
character probes are used. This qualifies fault recovery for those exact
components, not newer page edits or optical presentation. The unit suite
passed **143/143**, and the added numbered-instance case passed separately.
Mandatory presentation drift remains #12778's gate. The existing room suite
covers released effects through caster removal, retained grace state,
ownership, effect reservation and lifecycle identifiers; it is reused here.

### Failures retained and corrected

Two initial preflights never admitted a page: the old pair harness generated
`pair-HEX`, but the current API distinguishes a realm from its numbered
instance. The harness now requests `pair-HEX-1`, matching its socket path and
the API ticket. Incoming-only diagnostics retain refusal context without
printing ticket/hello credentials.

The first numbered-instance run reached all physical faults. Its snapshot and
reconnect checks passed, but its cast assertion incorrectly expected immediate
`left` cancellation. The existing ticketed reconnect contract retains accepted
casts and effects through grace; only casts still waiting for missing input
are discarded. Correcting that assertion produced the final passing run above.
No simulation or reconnect policy was changed to satisfy the check.

The initial partial-header unit check also caught its required trailing space;
the injector uses the existing wire encoder. Syntax/import failures are not
counted as behavioral red tests. Logs retain each attempt under its own name.
