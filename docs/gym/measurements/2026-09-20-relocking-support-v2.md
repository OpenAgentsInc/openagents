# Re-locking support-v2 from the items no adapter saw

openagents#9399 found that half of `support-v2-three-way`'s locked
partition is training data for every Lev adapter. This record says what
was found, which of the issue's options was taken, what the manifest now
says, and what the tooling does about it. Nothing here is a door
measurement; every number is a count over committed suite files.

## What happened

Two files partition the same 196 items:

- `crates/lev/suites/support-v2.json` is the two-way split, 98
  `calibration` and 98 `evaluation`.
- `crates/gym/suites/support-v2-three-way.json` came later and
  partitions the same items 79 `calibration`, 78 `development`, 39
  `locked`.

`training/lev-adapter/convert.py` trained the three adapters measured so
far from the two-way file's `calibration` split, before the three-way
partitioning existed. Twenty of those 98 items are locked in the
three-way file. Every adapter measured so far saw them, so the three-way
locked partition can confirm a base door but not an adapted one.
`convert.py` has since been changed to hold back whatever the three-way
file locks, which keeps the next adapter clean but does nothing for the
three already trained.
[`../../lev/calibration.md`](../../lev/calibration.md) tells the story
from the Lev side.

The counts are checked by
`suite::tests::the_three_way_suite_says_its_locked_partition_is_exposed`
and `the_unseen_suite_is_the_three_way_suite_minus_what_the_adapters_saw`
in `crates/gym/src/suite.rs`:

```text
cargo test -p gym exposed
cargo test -p gym unseen
```

## What was chosen

The issue favoured re-locking from the items no adapter saw, under a new
suite ID, with the old digest left as it was. The data on disk allows it,
so that is what was done. The other two options were not taken: growing
the suite would need new labelled items this session cannot author
honestly, and declaring the whole locked partition unusable would throw
away the 19 clean items and the base-door reads already in the ledger.

`support-v2-unseen` holds the 98 items of the two-way `evaluation` split
— the items `convert.py` never trained on — under the partitions
`support-v2-three-way` already gave them: 40 `calibration`, 39
`development`, 19 `locked`. Labels, questions, and partition assignments
are unchanged; only the trained-on items are gone.
`crates/gym/suites/build_support_v2_unseen.py` rebuilds it from the two
source files and refuses to write if the counts move.

| Suite | Items | Calibration | Development | Locked | Digest |
| --- | --- | --- | --- | --- | --- |
| `support-v2-three-way` | 196 | 79 | 78 | 39 (20 exposed) | `54fbf4137c3de538…` (unchanged) |
| `support-v2-unseen` | 98 | 40 | 39 | 19 | `2182ac195b4c06a2…` |

Nineteen locked items is a small partition. It is enough to refuse a
regression it can see, not enough to confirm a small win; the gate's
existing unverifiable verdict covers that, and the record that spends
it should say so.

## What the manifest says

The choice is written in the suite, not only here. `support-v2-three-way`
now carries:

```json
"exposure": {
  "partition": "locked",
  "items": 20,
  "through": "training/lev-adapter/convert.py, which trained every Lev adapter from the support-v2 calibration split before this suite locked 20 of those 98 items; fixed forward on 2026-09-20 (openagents#9399)",
  "successor": "support-v2-unseen"
}
```

The record is outside the item digest, so the digest every existing row
and record pins is the one the file still carries.

## What the tooling does

`LockedLedger::read_locked` and `read_locked_again` take the adapter the
door serves through `Spend::adapter` and refuse an exposed locked
partition for any non-empty adapter with `SuiteError::Exposed`, before
the ledger's lock is taken, so nothing is created or written. The error
names the successor. A base or hosted door still reads the partition.
`gym eval` prints the exposure in the suite header.
[`../ledger.md`](../ledger.md) describes the check beside the
transaction it precedes.

`suite::tests::an_exposed_locked_partition_is_refused_to_an_adapted_door_and_not_recorded`
is the test the issue asked for; its neighbours check that the base door
is still served, that an adapted door can spend `support-v2-unseen`, and
that an `exposure` record that does not describe its suite is refused at
load.

## What this does not settle

- No adapter has been scored on `support-v2-unseen`. The adapter records
  under `docs/lev/measurements/` stand as development-partition numbers;
  a locked-partition claim about an adapted door is confirmed on 19 clean
  items only once a run spends them.
- `support-v2-unseen`'s 40 calibration and 39 development items are the
  two-way `evaluation` split, so an adapter trained today by
  `convert.py` has seen none of them either. Another session owns
  `training/`; whether a future adapter trains from `support-v2-unseen`
  is its call, and the 19 locked items stay clean either way because the
  partitions are inside the digest and `Suite::load` refuses a moved
  item.
