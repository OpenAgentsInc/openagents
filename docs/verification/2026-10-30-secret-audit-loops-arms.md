# Large source-audit profile: for-each loops, arm writes, run constants

Part of #4828. Follows [declassification](2026-10-30-secret-audit-declassification.md).

## Changes

- **For-each loops.** `for item:collection` over a view or array with a
  fixed public extent of primitives is analyzed like a range loop. The item
  is any element and carries the collection's taint. Aggregate items refuse,
  and so does a loop directly in an unresolved branch. An index variable is
  not carried into the graph, so using one fails to resolve. Range and
  for-each loops now share one path, `AG_loop_run`, which also handles
  read-only bodies that return.
- **Writes inside unresolved public branches.** The inherited profile refused
  any write to a pre-existing region inside a public branch whose condition
  it could not resolve. Those writes are now admitted: taint only grows and
  the dirty flag drops literal facts, so treating the write as made whichever
  branch runs is a sound over-approximation, and it is still fully analyzed.
  Rebinding a pre-existing binding still refuses, because there is no join
  for bindings.
- **Loops in branches.** These refuse only when they sit directly in a
  branch body, where they would mark the branch's outer bindings. A
  procedure called from a branch has its own environment, so its loops are
  allowed. This replaces the earlier check on "any branch active". The
  now-unused `protected_regions` and `branch_active` bookkeeping is removed,
  and the inherited checks that pinned the old rules are restated for the new
  ones.
- **Run constants.** With attested compile-time runs (`--run-steps`), the
  value of `#run f(...)` is a public constant typed by `f`'s declared result.
  For example, `SHA_IV :: #run S_seed(0)` can be read as a struct. Other run
  expressions stay untyped, and unattested runs still refuse.
- **Masked int indices.** An `int` masked by a known non-negative constant
  (`x & 63`) has the interval `[0, mask]` for any `x`, including negative
  values. This applies only to `int` results, so the shared profile's
  conservative `u64` case is unchanged.

The legacy and public-view profiles and the shared tests are unchanged. The
exact correspondence blocks are 47, 1 and 7.

## Checks

On compiler SHA-256
`cd94879ecc21e72cc0d41a5e66c38194a0638a7167d01eb3a05380b4bf07fcfd`:

- **`aggregate.coax`** (SHA-256 `d7eae664…877d`): 227 of 227 checks pass.
  - `secret_field_cases.coax` (SHA-256 `680215e0…afbc`) adds 19 cases. They
    cover:
    - for-each over public, secret and one-byte views;
    - mismatched item types and aggregate items;
    - read-only bodies that keep a view public, versus writing bodies that
      taint it;
    - returns inside read-only and writing loops;
    - a collection expression at the depth limit;
    - an unclassified loop;
    - writes inside branches into the first region and into a public field's
      view;
    - typed, untyped and void run constants, attested and unattested.
  - Direct checks cover the mask interval.
- **Mutation:** killed **3,687 of 3,687, 0 survived**
  ([log](data/secret-loops-arms-2026-10-30/large-mutation.log)).
- **Python suites:** `test_public_views.py` (10 of 10; the real ECDH entry
  still completes at 61 procedures with its two public guard obligations),
  `test_large_closure.py`, `test_correspondence.py` and
  `secret_checked_cast_types.py` pass.

## Signer progress and what's left

With this profile and the source patch below, `sw_nonce` reaches **64
procedures** before refusing. That is the hard limit: each reached procedure
is a bit in the 64-bit memo closure masks.

The [patch](data/secret-loops-arms-2026-10-30/signer-path-sources.patch) is
not applied on `main`. It:

- adds `used & 63` block indices and splits the SHA schedule loop at 16;
- moves `so_finish`'s full-block clear into `so_compress_clear`;
- makes `snp_reduce` return its public `true`.

Each is the same computation, and the touched files' own checks pass. It is
held back because it invalidates the recorded mutation runs of the signer
(3,384) and of `event_sign.coax` (5,194), which take about 1.5 hours with 100
shards on the Mac. It lands together with the signer's `_declassify` edits so
those gates run once.

Still open for #4828:

1. **Procedure limit.** Lift the 64-procedure limit by reworking the memo
   closure representation. The nonce stage needs it, and the visit budget
   (about 200k of 262,144) is close too.
2. **Signer edits:** `_declassify` calls for key validity, the public key
   and R, then rerunning the signer and `event_sign` gates.
3. **Challenge.** `sv_challenge_vartime` with public inputs: this needs an
   owner decision on admitting public-only `_vartime` calls, or an
   analyzable `_public` challenge.
4. **Self-check.** `sw_publish`'s self-check is #89db's.
5. **Whole-entry audit.** Audit `schnorr_sign_step` / `nostr_sign_step` as a
   whole. Their stage dispatch branches write the workspace inside
   unresolved branches, which is now admitted.
6. **Remaining gates:** full reference mutation, independent review, and
   target timing.
