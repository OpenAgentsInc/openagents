# The locked-partition ledger

A suite's locked partition is read once, and the read is written down.
`gym::suite::LockedLedger` is the only path to it: `Suite::partition`
refuses `locked`, and the ledger appends a `LockedRead` record that says
what the read was spent on, why, and when. A second read of the same suite
digest is refused; `read_locked_again` takes one anyway only by recording
who authorized it and against what argument.

## A read is a transaction

The audit finding this document covers, openagents#9420, found the ledger
reading the file, deciding eligibility, and appending the record as three
steps nobody serialized. Callers released together could all see an empty
ledger and all append as the first reader — 15 of 16 did, in the audit's
probe.

A read now runs inside one transaction:

1. The ledger's lock is taken. The lock is a `*.lock` file beside the
   ledger, created with `create_new`, the same discipline the result store
   keeps for its single writer. It holds across processes on one machine,
   not only across threads.
2. The ledger file is read, and the eligibility check runs against what is
   committed — not what was committed when the caller last looked.
3. The record is appended and the file is synced. On the write that creates
   the ledger, every directory that gained a name in the transaction is
   synced too — the file's parent, and each new directory's parent up to
   the first one that already existed — so the file's name and the chain
   that holds it survive the same crash the contents do.
4. The items are handed back.

A read that returned is therefore a read that was recorded durably, and two
racing readers cannot both be first: the loser finds the winner's committed
record and is refused with `AlreadyRead`.

## One ledger, one lock

The lock is taken on the ledger's canonical path, not the path the caller
typed. A symlink, a `..` segment, a relative path, and the resolved
absolute path all resolve to one lock, so callers that spell the same
ledger differently still serialize.

Two aliases cannot be resolved to one lock that way:

- A hardlinked alias is a second name for one inode; each name is its own
  canonical path, so the two would take two locks while writing one file.
  A ledger whose link count is more than one is refused — the error says
  to point every reader at one path. On platforms without a link count the
  refusal is not checked.
- A symlink whose target does not exist is refused before the lock is
  taken, because two dangling aliases of one target would likewise take
  two locks while creating one file. Name the target instead.

## The lock

A read waits for the lock and reports `SuiteError::Locked` when its wait
expires, naming the lock file and the pid it claims.
The bound is ten seconds by default; `LockedLedger::lock_wait` sets it.

The bound measures time, not liveness. A lock still held past it may be a
read still running, a wedged holder, or a file a killed reader left — the
wait cannot tell which, and neither can the pid the file names until you
check it. Before removing the file the error names, check whether that
pid is a live read on the ledger; removing a live read's lock is how a
locked partition gets spent twice. `SuiteError::is_locked` identifies the
case for a caller that wants to retry rather than surface it.

## An interrupted write fails closed

A writer killed mid-append leaves the ledger in one of two states:

- A last line that parses counts as a spent read, even if the writer did
  not finish syncing it or return the items. The next append repairs a
  missing newline. This conservative rule prevents another first read.
- A last line that does not parse means the ledger cannot tell whether the
  read it was writing committed. `reads` and both read paths report
  `SuiteError::Interrupted` rather than guess, because guessing wrong is
  how a locked partition gets spent twice.

Recovery is yours, not the ledger's. Read the last line, decide whether the
record landed, and remove or complete it. Truncating the file to the last
committed line reopens nothing that already committed — the records above
the tear still count.

`reads` itself takes no lock, so a `reads` that meets a writer mid-append
can see the torn line that writer is still writing. The report is the same
`Interrupted`; the difference between an interrupted write and one in
flight cannot be told from the file.

## What the tests prove

`crates/gym/tests/locked_ledger.rs` runs the transaction in real child
processes — the test binary re-executed on a hidden entry point — released
together on a filesystem barrier so every reader reaches the ledger at the
same time:

- Eight racing processes get exactly one first read; eight more, spelling
  the ledger through a canonical path, a `..` segment, and a symlinked
  directory, still get exactly one.
- Two digests in one ledger get one first read each, and an override keeps
  its authority and reason across process boundaries.
- A child writes the artifacts a killed writer leaves — the lock file and
  a torn record — then exits, so the tests exercise the ledger's behavior
  against the same state a kill leaves rather than signaling a process
  inside its critical section.
- A child names its ledger `a/b/c/ledger.jsonl` relative to a working
  directory of its own, exercising the nested-chain creation and sync.

The unit tests in `suite.rs` cover the thread race, the torn-tail and
missing-newline recoveries, a held lock reported after a bounded wait —
including a zero wait and an unbounded one — a commit refused by the
filesystem mid-transaction, and the hardlink and dangling-symlink
refusals.

## What this assumes

- `create_new` is atomic across processes on a local filesystem. On network
  filesystems that is not always true; keep the ledger local.
- The directory syncs run on Unix, where a directory can be opened and
  synced. On other platforms the record's own sync still holds.
- The lock is per machine. Two hosts sharing one filesystem are not
  serialized.

## What the ledger is not

The records are not receipt-chained the way result-store rows are. The
ledger is a committed file whose protection is review: a second read shows
up in the diff. Tamper evidence over deletion or reordering belongs to the
result store, whose rows are data rather than a document someone reads.
