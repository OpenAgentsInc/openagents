# 178. Start fast

> **Status: partly implemented.** A startup plan that predicts each phase of
> a replica's start (verify, map, allocate arenas, golden warm-up) from file
> sizes, page-cache state, verified-file records and measured host rates; a
> readiness machine that reports ready only after every golden warm-up
> passes; a kernel-cache rule keyed by device, compiler and source; and a
> receipt of cold and warm starts measured on one host, with Elephant's
> original file verified and mapped. Machine and process start, fetching
> weights over the network, timing a real generated-kernel compile, real
> golden decisions in `psionic-serve`'s start and parallel hashing on host
> threads are not built yet.

Scale to zero and autoscaling only work if a new replica starts quickly, and
an autoscaler can only plan if it knows how long a start takes. This chapter
splits a start into phases, predicts each phase's time before the replica
starts, measures the same phases while it starts, and shows where the time
goes when the page cache is cold, when it is warm, and when the replica must
hash its weights again.

The library is [`startup.coax`](../lib/startup.coax). It reuses four earlier
pieces rather than rebuilding them:

- [chapter 7](ch007-reading-original-weights.md)'s `pw_digest`, the checked
  SHA-256 of a file's original bytes;
- [chapter 19](ch019-keep-the-model-resident.md)'s verified-file records,
  `pv_record` and `pv_admits` in
  [`model-io/access.coax`](../lib/model-io/access.coax), which let a session
  skip the hash of a file it already verified;
- [chapter 18](ch018-time-a-decision.md)'s page-cache labels: zero resident
  pages is `cold`, every page resident is `warm`;
- [chapter 35](ch035-use-every-core.md)'s `pc_plan` partition, to split
  verification over workers.

A small [native harness](../platform/startup.c) owns the files, the mappings,
the clock and the page-cache observation; every decision is a Coax procedure
it calls.

## The phases

A replica starts in four phases, always in this order:

1. **Verify.** Each weight file must match its pinned SHA-256. A file with a
   verified record that still admits it skips the hash. Any other file is
   hashed in full.
2. **Map.** Each file is mapped and every page touched, so the first request
   doesn't pay for page faults.
3. **Allocate arenas.** The working memory a decision needs (its size comes
   from a residency plan such as
   [chapter 33](ch033-measure-the-work-and-plan-the-memory.md)'s `rp_plan`)
   is allocated and touched once.
4. **Warm up.** The replica runs its golden decisions and compares each
   answer with the pinned one, as in
   [chapter 22](ch022-run-it-live.md).

Only then is it ready. The plan needs a few numbers about the host, each
measured in bytes per microsecond (1 B/µs is 1 MB/s), and a description of
each file:

<!-- source: psionic/lib/startup.coax types -->
```jai
// Measured host rates in bytes per microsecond (1 B/us is 1 MB/s): SHA-256
// over cached bytes, reading uncached pages from disk, mapping and touching
// cached pages, and first-touching fresh arena memory. One golden warm-up
// decision takes decision_us. Pages are 4, 16 or 64 KiB.
CsHost :: struct {hash:int;read:int;fault:int;zero:int;page:int;decision_us:int;}
// A weight file: its pinned name and digest, its identity now, the verified
// record the host found for it (empty if none), and how many of its pages
// the page cache holds.
CsFile :: struct {name:string;sha:string;stat:PaStat;record:[]u8;cached_pages:int;}
// Each phase's time in microseconds, the bytes hashed and the bytes read from disk.
// lane is the worker whose hashing takes longest, the first of equals.
CsPlan :: struct {status:CsStatus;verify_us:int;map_us:int;arena_us:int;warm_us:int;total_us:int;hashed:int;read:int;lane:int;}
```

## Predict each phase

The cost of a phase depends on more than a file's size. Hashing reads the
whole file, so a file that is hashed is also pulled into the page cache, and
mapping it afterwards only pays for page faults on cached pages. A file that
a record admits is never read during verification, so its cold pages are
read from disk when it is mapped. Hashing is arithmetic and can be split
across cores; the disk is one queue, so its reads add up whichever worker
asks for them.

<!-- source: psionic/lib/startup.coax plan -->
```jai
// Predict each phase. A file whose verified record admits it skips the hash;
// its cold pages are read when it is mapped. Any other file is hashed, which
// reads its cold pages, so mapping finds it cached. Hashing splits across
// workers by chapter 35's partition; the disk is one queue, so reads add up.
// A lazy map defers the mapping into the warm-up, which touches every weight.
cs_plan :: (files:[]CsFile,h:CsHost,arena:int,golden:int,workers:int,eager:bool)->CsPlan #export_c {
    if !cs_host_ok(h) {return CsPlan.{status=CsStatus.rate};}
    if files.count>CS_FILES_MAX || arena<0 || arena>CS_ARENA_MAX || golden<1 || golden>CS_GOLDEN_MAX {return CsPlan.{status=CsStatus.shape};}
    lanes:=pc_plan(files.count,1,workers);
    if lanes.status!=PcStatus.ok {return CsPlan.{status=CsStatus.shape};}
    for f:files {if !cs_file_ok(f,h.page) {return CsPlan.{status=CsStatus.shape};}}
    p:=CsPlan.{status=CsStatus.ok};lane_max:=0;reads:=0;load:=0;
    for w:0..<workers {
        lane:=0;
        for i:pc_first(lanes,w)..<pc_first(lanes,w+1) {
            f:=files[i];cold:=cs_cold(f,h.page);p.read+=cold;
            if pv_admits(f.record,f.name,f.sha,f.stat) {load+=cs_up(cold,h.read);}
            else {lane+=cs_up(f.stat.size,h.hash);reads+=cs_up(cold,h.read);p.hashed+=f.stat.size;}
            load+=cs_up(f.stat.size,h.fault);
        }
        if lane>lane_max {lane_max=lane;p.lane=w;}
    }
    p.verify_us=lane_max+reads;p.arena_us=cs_up(arena,h.zero);p.warm_us=golden*h.decision_us;
    if eager {p.map_us=load;} else {p.warm_us+=load;}
    p.total_us=p.verify_us+p.map_us+p.arena_us+p.warm_us;
    return p;
}
```

Every division rounds up, so a prediction never comes out lower because of
rounding.

### Trace it by hand

Take round rates: hash 500 B/µs, disk read 2,000 B/µs, fault on cached pages
8,000 B/µs, arena first touch 4,000 B/µs, 4,096-byte pages, and 20,000 µs for
one golden decision. Two files:

- **A**: 409,600,000 bytes (100,000 pages), none cached, no record.
- **B**: 204,800,000 bytes (50,000 pages), 25,000 pages cached, with a record
  that admits it.

The arena is 400,000,000 bytes and the replica runs 3 golden decisions, on
one worker, mapping eagerly.

- **Verify.** A is hashed: 409,600,000 / 500 = 819,200 µs. Hashing reads all
  of A from disk: 409,600,000 / 2,000 = 204,800 µs. B is admitted by its
  record and costs nothing here. Verify: 819,200 + 204,800 = **1,024,000 µs**.
- **Map.** A is now cached: 409,600,000 / 8,000 = 51,200 µs of faults. B
  has 25,000 cold pages, 102,400,000 bytes, read now: 102,400,000 / 2,000 =
  51,200 µs, then faults on all of it: 204,800,000 / 8,000 = 25,600 µs.
  Map: 51,200 + 51,200 + 25,600 = **128,000 µs**.
- **Arenas.** 400,000,000 / 4,000 = **100,000 µs**.
- **Warm-up.** 3 × 20,000 = **60,000 µs**.
- **Total.** 1,024,000 + 128,000 + 100,000 + 60,000 = **1,312,000 µs**.

Three variations of the same start:

- **Lazy mapping.** Skip the map phase and let the warm-up fault the pages
  in. The warm-up reads every weight, so it pays the same 128,000 µs: map 0,
  warm-up 188,000 µs, total still 1,312,000 µs. Lazy mapping moves the cost;
  it doesn't remove it, and the replica is ready no sooner.
- **A warm restart.** Every page cached and both records admit their files:
  verify 0, map 51,200 + 25,600 = 76,800 µs, total 76,800 + 100,000 + 60,000
  = **236,800 µs**, about a fifth of the first start.
- **No records, 1 or 2 workers.** With B's record gone, B is hashed too:
  409,600 µs more of hashing and its 51,200 µs of reads move from map to
  verify. On one worker verify is 819,200 + 409,600 + 204,800 + 51,200 =
  1,484,800 µs. On two workers the hashing runs side by side, and the longer
  lane, A's 819,200 µs, decides: verify 819,200 + 256,000 = 1,075,200 µs.
  The reads stay serial.

## Ready only after warm-up

Readiness is a small state machine. Each phase ends with one event from the
host, passing or not, and only the current phase's own event moves the
replica forward:

<!-- source: psionic/lib/startup.coax life -->
```jai
CS_NEXT :: [4]CsEvent.[CsEvent.verified,CsEvent.mapped,CsEvent.allocated,CsEvent.warmed];
cs_start :: (golden:int)->CsLife #export_c {
    if golden<1 || golden>CS_GOLDEN_MAX {return CsLife.{state=CsState.failed,why=CsStatus.shape};}
    return CsLife.{state=CsState.verify,golden=golden};
}
cs_fail :: (l:CsLife,why:CsStatus)->CsLife {n:=l;n.state=CsState.failed;n.why=why;n.at=l.state;return n;}
// One event: the phase in progress finished, passing (ok) or not. Only the
// current phase's own event moves a replica forward; a failed check, a
// broken report or an event out of order fails it for good. Ready needs
// golden passing warm-ups; a ready replica stays ready through passing re-checks.
cs_step :: (l:CsLife,e:CsEvent,ok:bool)->CsLife #export_c {
    if l.state==CsState.failed {return l;}
    if e==CsEvent.broken || (!ok && e==CS_NEXT[cast(int)cs_last(l.state)]) {return cs_fail(l,CsStatus.check);}
    if l.state==CsState.ready {if e==CsEvent.warmed {return l;}return cs_fail(l,CsStatus.order);}
    if e!=CS_NEXT[cast(int)l.state] {return cs_fail(l,CsStatus.order);}
    n:=l;
    if l.state!=CsState.warmup {n.state=cast(CsState)(cast(u8)l.state+1);return n;}
    n.warm+=1;if n.warm==n.golden {n.state=CsState.ready;}
    return n;
}
// The only answer to "may this replica take requests?".
cs_ready :: (l:CsLife)->bool #export_c {return l.state==CsState.ready;}
```

A failed check (a hash that doesn't match, a golden answer that doesn't
match), a `broken` report, or any event out of order fails the replica for
good, and the failure keeps the phase it happened in and its reason. A ready
replica stays ready through passing golden re-checks; anything else fails
it. `cs_ready` is the only answer to whether the replica may take requests,
so there is no path to readiness that skips a phase.

[Chapter 22](ch022-run-it-live.md)'s `ls_ready` already requires
authentication, sealed storage and three passing golden decisions. This
machine adds the order: mapping and arenas must finish before the first
golden decision, and nothing counts after a failure.

## Reuse a compiled kernel only under its own key

Generated kernels can be cached between starts, but only for the exact
device, compiler and source they were compiled from. A cached entry is filed
under all three; a lookup that differs in any part is stale, and an entry
whose bytes no longer hash to the digest recorded when it was compiled is
refused as a failed check:

<!-- source: psionic/lib/startup.coax kernel -->
```jai
cs_device :: (s:string)->bool {
    if s.count<1 || s.count>CS_NAME_MAX {return false;}
    for c:s {if c<=32 || c>=127 {return false;}}
    return true;
}
cs_key_ok :: (k:CsKernelKey)->bool {return cs_device(k.device) && pw_hex_digest(k.compiler) && pw_hex_digest(k.source);}
// A cached kernel filed under `entry` may serve a compile asking for `want`
// only if every part of the key is the same and its bytes still hash to the
// digest recorded when it was compiled.
cs_kernel :: (entry:CsKernelKey,want:CsKernelKey,binary:[]u8,binary_sha:string)->CsStatus #export_c {
    if !cs_key_ok(entry) || !cs_key_ok(want) || !pw_hex_digest(binary_sha) {return CsStatus.shape;}
    if entry.device!=want.device || entry.compiler!=want.compiler || entry.source!=want.source {return CsStatus.stale;}
    if !pw_digest(binary,binary_sha) {return CsStatus.check;}
    return CsStatus.ok;
}
```

## Run it

The [example](../examples/start-fast.coax) prints the hand-worked plans and
walks the readiness machine:

<!-- source: psionic/examples/start-fast.coax example -->
```jai
main :: () {
    h:=CsHost.{hash=500,read=2000,fault=8000,zero=4000,page=4096,decision_us=20000};
    rb:[512]u8;nb:=pv_record("b.safetensors",SHA_B,STAT_B,rb[..]);
    ra:[512]u8;na:=pv_record("a.safetensors",SHA_A,STAT_A,ra[..]);
    // A: 100,000 pages, none cached, no record. B: 50,000 pages, half cached, with its record.
    files:=[2]CsFile.[CsFile.{name="a.safetensors",sha=SHA_A,stat=STAT_A},
        CsFile.{name="b.safetensors",sha=SHA_B,stat=STAT_B,record=rb[0..<nb],cached_pages=25000}];
    arena:=400000000;
    say("first start",cs_plan(files[..],h,arena,3,1,true));
    say("lazy map",cs_plan(files[..],h,arena,3,1,false));
    warm:=files;warm[0].record=ra[0..<na];warm[0].cached_pages=100000;warm[1].cached_pages=50000;
    say("warm restart",cs_plan(warm[..],h,arena,3,1,true));
    fresh:=files;fresh[1].record=rb[0..<0];
    say("no records, 1 worker",cs_plan(fresh[..],h,arena,3,1,true));
    say("no records, 2 workers",cs_plan(fresh[..],h,arena,3,2,true));

    l:=cs_start(3);
    events:=[6]CsEvent.[CsEvent.verified,CsEvent.mapped,CsEvent.allocated,CsEvent.warmed,CsEvent.warmed,CsEvent.warmed];
    for e:events {
        l=cs_step(l,e,true);
        print("% passed: state %, golden % of %, ready %\n",e,cs_state_name(l.state),l.warm,l.golden,cs_ready(l));
    }
}
```

```shell
coax/bin/coax run psionic/examples/start-fast.coax
```

```text
first start: verify 1024000 + map 128000 + arenas 100000 + warm-up 60000 = 1312000 us (hashed 409600000, read 512000000, slowest lane 0)
lazy map: verify 1024000 + map 0 + arenas 100000 + warm-up 188000 = 1312000 us (hashed 409600000, read 512000000, slowest lane 0)
warm restart: verify 0 + map 76800 + arenas 100000 + warm-up 60000 = 236800 us (hashed 0, read 0, slowest lane 0)
no records, 1 worker: verify 1484800 + map 76800 + arenas 100000 + warm-up 60000 = 1721600 us (hashed 614400000, read 512000000, slowest lane 0)
no records, 2 workers: verify 1075200 + map 76800 + arenas 100000 + warm-up 60000 = 1312000 us (hashed 614400000, read 512000000, slowest lane 0)
verified passed: state map, golden 0 of 3, ready false
mapped passed: state arenas, golden 0 of 3, ready false
allocated passed: state warmup, golden 0 of 3, ready false
warmed passed: state warmup, golden 1 of 3, ready false
warmed passed: state warmup, golden 2 of 3, ready false
warmed passed: state ready, golden 3 of 3, ready true
```

## Refuse what cannot start

The [second example](../examples/start-fast-refuse.coax) changes one byte of
a three-byte weights file, gives a record for a file whose change time moved,
looks up kernels under other keys, and sends the plan and the machine inputs
they refuse:

<!-- source: psionic/examples/start-fast-refuse.coax refuse -->
```jai
main :: () {
    // The weights are the three bytes "abc", pinned by their SHA-256. One byte changes.
    weights:=[3]u8.[97,98,100];
    l:=cs_step(cs_start(3),CsEvent.verified,pw_digest(weights[..],SHA_ABC));
    life("\"abd\" against the pin of \"abc\"",l);
    life("then the host reports mapped",cs_step(l,CsEvent.mapped,true));

    // A record written before the change: any change to the file moves its change time.
    st:=PaStat.{device=66306,inode=12,size=3,mtime_ns=1793000000000000000,ctime_ns=1793000000000000000};
    rec:[512]u8;n:=pv_record("w.bin",SHA_ABC,st,rec[..]);
    now:=st;now.ctime_ns+=1;
    h:=CsHost.{hash=500,read=2000,fault=8000,zero=4000,page=4096,decision_us=20000};
    f:=[1]CsFile.[CsFile.{name="w.bin",sha=SHA_ABC,stat=now,record=rec[0..<n],cached_pages=1}];
    print("record for the old change time: admitted %, hashed % bytes\n",pv_admits(rec[0..<n],"w.bin",SHA_ABC,now),cs_plan(f[..],h,0,1,1,true).hashed);

    abc:=[3]u8.[97,98,99];
    key:=CsKernelKey.{device="cpu-x86_64",compiler=COMPILER,source=SOURCE};
    other:=key;other.device="cuda-sm89";print("kernel for another device: %\n",cs_status_name(cs_kernel(key,other,abc[..],SHA_ABC)));
    other=key;other.compiler=SOURCE;print("kernel from another compiler: %\n",cs_status_name(cs_kernel(key,other,abc[..],SHA_ABC)));
    other=key;other.source=COMPILER;print("kernel for other source: %\n",cs_status_name(cs_kernel(key,other,abc[..],SHA_ABC)));
    print("kernel bytes changed: %\n",cs_status_name(cs_kernel(key,key,weights[..],SHA_ABC)));

    print("a hash rate of 0: %\n",cs_status_name(cs_plan(f[..],CsHost.{read=1,fault=1,zero=1,page=4096,decision_us=1},0,1,1,true).status));
    f[0].cached_pages=2;print("2 cached pages of a 1-page file: %\n",cs_status_name(cs_plan(f[..],h,0,1,1,true).status));
    f[0].cached_pages=0;print("no golden decisions: %\n",cs_status_name(cs_plan(f[..],h,0,0,1,true).status));
    t:=[5]int.[100,90,200,300,400];print("a clock that runs backwards: %\n",cs_status_name(cs_measured(t[..]).status));
    life("mapped before verified",cs_step(cs_start(3),CsEvent.mapped,true));
    r:=cs_start(1);r=cs_step(r,CsEvent.verified,true);r=cs_step(r,CsEvent.mapped,true);r=cs_step(r,CsEvent.allocated,true);r=cs_step(r,CsEvent.warmed,true);
    life("ready",r);
    life("ready, then a golden re-check fails",cs_step(r,CsEvent.warmed,false));
    life("ready, then the host reports broken",cs_step(r,CsEvent.broken,true));
}
```

```shell
coax/bin/coax run psionic/examples/start-fast-refuse.coax
```

```text
"abd" against the pin of "abc": state failed, ready false, failed at verify (check)
then the host reports mapped: state failed, ready false, failed at verify (check)
record for the old change time: admitted false, hashed 3 bytes
kernel for another device: stale
kernel from another compiler: stale
kernel for other source: stale
kernel bytes changed: check
a hash rate of 0: rate
2 cached pages of a 1-page file: shape
no golden decisions: shape
a clock that runs backwards: clock
mapped before verified: state failed, ready false, failed at verify (order)
ready: state ready, ready true
ready, then a golden re-check fails: state failed, ready false, failed at ready (check)
ready, then the host reports broken: state failed, ready false, failed at ready (check)
```

A replica whose weights changed fails at verify with `check` and stays
failed when the host reports the next phase. A record written before a change
no longer admits the file, so the plan hashes it in full: a stale cache is
never trusted. Both examples exit with status 0 and print the same
transcript in debug and release builds.

## Measure it on this host

The [harness](../platform/startup.c) writes a 256 MiB file of random bytes on
the build's disk, pins its SHA-256, and measures the host's rates, each the
fastest of three passes: SHA-256 through a fresh mapping (`pw_digest`), cold
pages faulted in through a fresh mapping after `posix_fadvise(DONTNEED)`,
faults on cached pages, first touch of a 64 MiB arena, and one stand-in
golden pass (`cs_golden`, which reads every byte once and must reproduce the
answer pinned for those bytes). Then it starts a replica five times, timing
each phase with the monotonic clock and reporting each phase's end to
`cs_step`:

1. cold page cache, no record;
2. warm page cache, the record the first start wrote;
3. warm page cache, no record;
4. cold page cache, the record;
5. one byte of the file changed, the record still on disk.

```shell
psionic/tests/startup.sh --elephant ~/models/elephant-1-b3f8a934 --hash --record
```

It first runs the same starts on a 4 MiB file under AddressSanitizer and
UndefinedBehaviorSanitizer and discards their timings. The
[receipt for coderos-4080](../fixtures/startup/coderos-4080.json), an Intel
Core i7-14700K (28 hardware threads) under Linux 6.18 with GCC 15.2 at `-O2`,
was recorded 2026-11-01 at a load average of 16.5 on a shared machine.
Measured rates: hash 246 B/µs, cold read through a mapping 984 B/µs, faults
on cached pages 40,599 B/µs, arena 2,522 B/µs, one golden pass 81,859 µs.
Times in milliseconds, predicted / measured:

| Start | Verify | Map | Arenas | Warm-up (3) | Total | State |
| --- | --- | --- | --- | --- | --- | --- |
| Cold, no record | 1,364 / 1,018 | 6.6 / 1.0 | 26.6 / 20.7 | 245.6 / 225.4 | 1,642.8 / 1,264.9 | ready |
| Warm, record | 0 / 0.001 | 6.6 / 3.0 | 26.6 / 20.5 | 245.6 / 225.4 | 278.8 / 248.9 | ready |
| Warm, no record | 1,091 / 989 | 6.6 / 1.0 | 26.6 / 19.8 | 245.6 / 225.3 | 1,370.0 / 1,234.8 | ready |
| Cold, record | 0 / 0 | 279.4 / 271.0 | 26.6 / 20.4 | 245.6 / 229.3 | 551.6 / 520.7 | ready |
| One byte changed | 1,091 / 999 | — | — | — | — / 998.8 | failed at verify (check) |

What the receipt shows:

- **The record is what makes a restart fast.** A warm start with the record
  reached ready in 249 ms; a warm start without it took 1,235 ms, four fifths
  of it hashing. A cold start with the record took 521 ms, most of it
  reading pages from disk.
- **The changed byte was caught.** Writing one byte moved the file's change
  time, the record stopped admitting it, the hash failed, and the replica
  never reported ready.
- **The plan is close where the phases are serial and high where they
  overlap.** The cold first start came in 23% under its prediction: the
  plan adds disk time and hashing time, but the kernel reads ahead while the
  hash runs, so the two overlap. Other totals are within 11%.
- **A rate measured the wrong way misleads.** An earlier revision of the
  harness measured the disk with `read()` into a buffer (3,264 B/µs) and
  predicted 89.6 ms for the map phase of the cold, record start; it took
  267 ms. The start
  faults pages in through a mapping, which is about three times slower here,
  so the harness now measures the disk that way.

Elephant's original `model.safetensors` (18,819,722,392 bytes, 4,594,659
pages) is observed read-only: nothing is evicted and no record is written.
Its existing verified record admitted it in under a microsecond. In the
recorded run every page was already cached, and mapping and touching all of
it took 224 ms (predicted 464 ms). Hashing all of it took 73.98 s (predicted
76.50 s) and matched the pin. In the earlier run the file was cold, and
mapping it took 15.96 s. The record turns a 74-second verification into a
comparison of a few stat fields; that difference is the cost
[chapter 177](ch177-route-queue-and-scale-replicas.md)'s autoscaler needs.

The numbers are from one run on a shared machine and are not stable; that is
why no measured number appears in an exact transcript, and why the receipt
names the machine, compiler, flags and load.

## Check it

```shell
coax/bin/coax test psionic/lib/startup.coax
coax/bin/coax mutate psionic/lib/startup.coax
coax/bin/coax run psionic/examples/start-fast.coax
coax/bin/coax run psionic/examples/start-fast-refuse.coax
psionic/tests/startup.sh
```

The independent checks:

- the hand-worked plans above, written out as expected values;
- a `#forall` over every sequence of five events drawn from six symbols
  (each phase's event passing, a failed warm-up, a broken report) for 1 and
  2 golden decisions, 15,552 cases, compared with an oracle that knows only
  the legal path: ready exactly when the events so far are verify, map,
  allocate and then only passing warm-ups, at least as many as the golden
  count; failed as soon as an event leaves that path;
- each part of the record's key changed in turn (device, inode, modification
  time, change time, pin, name, a truncated record), each forcing a full
  hash in the plan;
- each part of the kernel key changed in turn, in both directions, each
  refused as stale, and changed kernel bytes refused as a failed check;
- the published SHA-256 of `"abc"` as the kernel digest.

`startup.coax` is `#authority`: it decides when a replica may take requests.
`coax mutate` leaves no survivors (3,210 mutants, including the loaded
record, digest and partition code its checks reach).

## What is not built yet

- **Machine and process start.** The receipt starts inside one running
  process. Booting a machine and starting `psionic-serve` are not timed.
- **Fetching weights.** A replica that holds a file whose verified record
  admits the pin needs no fetch, and `pv_admits` already decides that; a
  content-addressed fetcher that downloads only missing files is not built.
- **Generated-kernel compiles.** The cache key and its checks run; timing a
  real compile, and a cache hit against it, on a GPU is not done.
- **Real golden decisions in `psionic-serve`.** The harness's warm-up is a
  stand-in pass over the weights. The server still starts with chapter 22's
  `LsLife`; moving it to `cs_step`, with its real golden decisions, is next.
- **Parallel hashing on threads.** The plan predicts hashing split over
  workers by `pc_plan`; the harness hashes on one thread.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
