# Chapter 15 (first part): verified-file records, 2026-10-29

Every start of the `/v1/systemone` server hashed Elephant's 18.8 GB checkpoint
on one CPU core before answering: 72 s on an idle host, 195 to 321 s on this
host today with its load average near 50. The checkpoint does not change
between starts. Now the session hashes a file in full once, the host keeps a
record of that file's identity, and a later start of the same file skips the
hash.

## The record

`psionic/lib/model-io/access.coax` (`#authority`) writes and admits records:

    psionic-verified 1 NAME SHA SIZE DEVICE INODE MTIME_NS CTIME_NS

A record admits a mapped file only if it is byte for byte the record that
file would get now, for the pinned shard name and SHA-256, and the stat size
equals the mapping's length. Any write to a file changes its change time
(ctime), which no ordinary tool can set back, so a changed file no longer
matches and is hashed in full. `PaFile` carries the record and the stat;
`PwShard.verified` skips only that shard's hash (config, tokenizer, readout and
index are still hashed, and every tensor bound as before).
`decision-session.coax` decides `verified` and exports `ps_shard_record`, the
record the host keeps after an open that hashed.

`psionic/platform/verified-file.{c,h}` is the host's part: stat the path, read
the record kept for that device and inode from `PSIONIC_VERIFIED_DIR`
(default `$XDG_CACHE_HOME/psionic/verified`, else `~/.cache/psionic/verified`),
and after a successful hashing open write the new record atomically. Used by
`serve-host.c` (every build) and chapter 30's `elephant_device.c`.

## Results

GPU server beside gpt-oss-20b, 512-token capacity:

| Start | Authentication | Note |
| --- | --- | --- |
| first, no record | 321.4 s | hashed in full, then kept the record |
| next, with the record | 0.25 s | `a verified-file record names this checkpoint; its hash is skipped` |

The answer after the fast start was unchanged: `yes`, p_yes 0.757202773, in
671 to 753 ms.

The record of the running server was written by hand once, through Coax's
`ps_shard_record`, from the file's stat (ctime 2026-10-29 10:26:08): the file
had passed full hashes at 20:24, 20:35 and 20:41 and had not changed since
10:26. The hashing start above then wrote the same record itself.

The start still takes about 17 s more to pin 14.45 GiB of streamed weights
(chapter 30a) and 0.5 s to compile kernels.

## Checks

- `coax test`: `access.coax` 77 checks (3 new: the record's format, every
  field refusing, decimals and overflow), `rank5-bundle.coax` 107 (a verified
  shard skips only its own hash), `decision-session.coax` 279 (a record of
  the mapped file opens a pin whose digest the bytes lack, and an empty
  record, another inode, a stat size or a mapping that differs refuses;
  `ps_shard_record`), `bundle.coax` 90.
- `coax mutate`: `access.coax` 2,001 mutants and `rank5-bundle.coax` 2,727,
  0 survivors each (2 shards). A first version of the record code left 6
  survivors (a -1 sentinel made two checks equivalent); it was restructured
  and tested, not exempted.
- `serve-build.sh` release plain with and without `gpu`; chapter 30's
  harness compiles with the helper.

## Not done

- `coax mutate psionic/lib/decision-session.coax` is owed. It did not finish
  in 2 shards or in 8 shards (3 jobs each) within the 15-minute limit; the
  host's load average was 94 to 138 from other work. Every mutant runs all
  279 session checks, so it needs an idle host or a smaller split of the
  unit. The new code there is the `known` line and `ps_shard_record`, both
  covered by the two new tests.
- The host stats the path after mapping it. A file replaced at that path
  between the two would get the new file's identity for the old mapping; a
  `fstat` of the mapped descriptor would close that.
- `psionic/tests/serve.sh` (sanitized builds) was not run.
- `psionic-serve --device-mib N` (chapter 48c) caps the device plan's budget;
  release builds with and without `gpu` pass, and `--toy --device-mib`
  is refused.
- Pinning the streamed weights (17 s) is the next start-up cost.

## Part III follow-up, same date

The [Part III record](psionic-partIII-2026-10-29.md) closes the mapped-path
replacement race with descriptor-bound `fstat`, adds the native race check
under debug/release plain and ASan/UBSan, and runs the existing HTTP/native
sanitizer checks. It also measures cold pages/full hash, warm pages/full hash
and warm pages/verified-record reuse separately. The earlier observations
above remain historical; the deferred full session mutation run remains
omitted under the owner's focused-check instruction.
