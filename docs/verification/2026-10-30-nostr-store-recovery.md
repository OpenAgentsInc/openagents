# Nostr store: crash recovery of the relay's --store file (#966b, relay part)

Task: nostr-store-trusted-recovery (#966b), the part R1 needs: after a crash
or a torn write, the Coax relay restarts from its append-only `--store` file
with every complete record verified again, a crash's partial last write cut
off, other damage refused, and no event lost that was acknowledged OK true.
Worker: claude-store-recovery, 2026-10-30.

**#966b is not done.** Its board acceptance asks for an independently
trusted, rollback-resistant checkpoint provider (a store identity, schema,
monotonic generation and digest held outside the file). Choosing and
provisioning that provider is an owner decision (coax/nostr-handoff.md).
Nothing here detects an older copy of the whole file: it replays as a valid
older store. This record covers crash safety only.

## Was OK true sent only after the durable commit?

Yes, already, and it still is. `rsd_admit_saved` (store_saved.coax) plans the
admission, and when the plan writes bytes it appends the frame and calls
`cxstore_commit`, which `pwrite`s and `fsync`s, before it applies the plan.
A failed write applies nothing and answers RS_FULL (OK false). The relay
(`ry_event`) writes the OK only after `rsd_admit_saved` returns, then fans
the event out. A duplicate that writes nothing gets OK true for bytes that
are already durable. Two gaps in the C host were closed:

- A newly created file's directory entry was never synced, so the first
  acknowledged events could vanish with the file after a power loss.
  `cxstore_load` now creates with `O_EXCL` and fsyncs the parent directory.
- When an append failed and cutting it back also failed, the next append
  went after the partial frame, putting damage in the middle of the file.
  Now every later commit fails (the relay answers OK false) instead.

## The recovery rule

`coax/lib/nostr/store_recover.coax` (`#authority`, loads nothing) decides
each frame's fate; store_saved.coax's `rsd_replay` scans and calls it; the C
host only reads, appends with fsync and truncates.

Every append is one frame written and fsynced before the OK, so a crash can
damage only the last frame, and only the way an interrupted append does: the
file ends inside it, or the file grew but some new bytes are still zero. A
JSON event never holds a zero byte, and every frame header begins with one
(lengths are at most 262144). So a crash leaves one shape: non-zero bytes,
then zeros to the end of the file. Replay:

| At a frame | Decision |
| --- | --- |
| The end of the file | recovered |
| Torn: the file ends inside the header | cut there |
| Torn: whole header, body runs past the end, the bytes after the header are non-zero bytes then zeros | cut there |
| Torn, but a zero byte is followed by more data (later frames) | refuse |
| A length of 0 or above 262144, and every byte from there to the end is zero | cut there |
| Such a length with any non-zero byte after it | refuse |
| A whole last frame whose body ends in zeros (a prefix, then zeros) | cut there, unread |
| A whole frame otherwise | read, verify (`nostr_event_verify_vartime`), re-admit |
| ...the store takes it (stored, duplicate, or an older version) | read on |
| ...anything else (not an event, bad id or signature, a kind or time the relay refuses, the store full) | refuse |

Refuse means: the relay prints `relay: refusing to start: the --store file
is damaged at byte N (reason R)` and exits before it listens, and the file
is not changed. Skipping a damaged record could lose an acknowledged event
or bring back a replaceable version it replaced, so the operator decides.
Previously a bad length anywhere truncated the file there, dropping every
acknowledged event after it, and a frame that failed verification was
skipped.

Limits of the rule: a damaged length on the *last* frame that runs past the
end looks like a torn append and is cut, losing that one event (its bytes
were committed, then damaged; no format without redundancy can tell). A real
interrupted append whose zero bytes are not all at the end (a filesystem
writing a later page before an earlier one) is refused, not cut: safe, but
it needs the operator. The file format did not change.

## Checks

Compiler: coax0 0.24.0 built in the worktree (`coax/bin/coax` SHA-256
`c0786a7e46ad23a2f14e7c59910ef7b6426802b00aec079e094f84ea856099d0`).
store_recover.coax SHA-256 `14ebcf5c624c02b342777fceeaafa7db8120607743540a625c9ce327729b39c8`.

| What | Result |
| --- | --- |
| `coax test coax/lib/nostr/store_recover.coax --library` | **9/9**: 8 tests (end of file, torn header and body, a torn-looking frame with frames after it, bad lengths, a last frame ending in zeros, a middle frame, inconsistent scans and unknown statuses, which admission reasons read on) and one `#forall` over 107,520 cases (every cut is a crash's shape, DONE only at the end, NEXT only for a whole frame) |
| `coax mutate coax/lib/nostr/store_recover.coax --library --jobs 4` | **72/72 killed**, 0 survivors, no `// mutant: equivalent`. The first run had 4 survivors (a clamp `min(at + 4, total)` whose boundary cannot be observed, and no test at offset 0); the clamp became an explicit torn-header case and tests at offset 0 were added |
| `coax test coax/lib/nostr/store_saved.coax` | 319/319 (the relay's reasons and the file's statuses agree with store_recover's mirrors) |
| `coax test coax/lib/nostr/store_file.coax --library` | 16/16 (unchanged; its 242/242 mutation result stands) |
| `python3 coax/tests/nostr/store_file.py` (ASan+UBSan, one real file) | **38/38** (was 15): empty and missing file (created 0600), a missing directory, restarts, a torn tail, a zero length, a last frame ending in zeros, a header and zeros, a duplicate record, and eight kinds of committed damage refused with the file unchanged: a last frame that is not an event, a forged signature in a middle and in the last frame, a middle length one too long, one too short, zero, past the end, a zero byte in a middle frame; restored, it replays |
| `python3 coax/tests/nostr/store_crash.py 18 30` (ASan+UBSan relay, on the rebased tree with wss:// linked in) | **27/27**: 18 SIGKILLs; 70 events sent, 57 acknowledged OK true; tail damage torn 5, zero-body 4, zeros 2, none 7 (4 because the last frame was acknowledged); 10 restarts cut a partial write. After every restart: no acknowledged note lost, nothing unsent served, heads never older than the newest acknowledged version, the file whole frames. 5 committed-damage cases refused with the file unchanged; restored, all 39 acknowledged notes served; no sanitizer report. About 6 minutes. An earlier run before the rebase (seed 30, 20 rounds, 27/27, 52 acknowledged of 69 sent) shows the allowed case of an event durable but never acknowledged (the kill came between the fsync and the OK): 38 frames for 37 acknowledged in round 14. Logs: [data/nostr-store-recovery-2026-10-30/](data/nostr-store-recovery-2026-10-30/) |
| `python3 coax/tests/nostr/relay_session.py` (ASan+UBSan, wss://, rebased) | 72/72, including the restart with `--store` |

`store_crash.py` is the crash test. Each round starts the relay on the same
`--store` file, checks what it serves, then streams signed writes (kind 1
notes, kind 0 and kind 30617 versions with increasing `created_at`, and now
and then an acknowledged event again) over one NIP-42-authenticated
WebSocket and kills the relay with SIGKILL after a random 0.1 to 3 s. Before
the next start it damages the file as an interrupted append can (nothing, a
torn frame, a frame whose last bytes are zero, a run of zeros, or a cut
inside a last frame that was never acknowledged). After every restart:
every acknowledged note is served, nothing is served that was not sent, the
kind 0 and 30617 heads are never older than the newest acknowledged version,
and the file is whole frames. At the end, five kinds of committed damage
(a middle signature byte, a middle length, a middle length made impossible,
a zero byte in a middle frame, the last frame's signature) each make the
relay refuse with the file unchanged; restored, it starts and serves every
acknowledged note.

## What is missing

- **The trusted checkpoint (the rest of #966b).** An older copy of the
  whole file, or a deleted file, replays as a valid older or empty store.
  The board task needs a real rollback-resistant provider, a writer fence
  and their tests (see
  [2026-10-29-nostr-store-recovery-model.md](2026-10-29-nostr-store-recovery-model.md)).
  That is an owner decision. #966b stays open on the board.
- **Power loss.** SIGKILL tests the process dying; the kernel still writes
  the page cache out. Power loss and filesystem reordering are emulated
  only by the tail damage above, not by a real power cut or a fault-
  injecting block device.
- No per-frame checksum or format version, so a damaged last-frame length
  cannot be told from a torn append (above). Compaction is still missing.
- `store_saved.coax` (the scan loop) is glue, checked by store_file.py and
  store_crash.py, not by mutation.
- No refuse-override for the operator (for example a salvage flag that
  copies the file aside and keeps the whole frames before the damage).
