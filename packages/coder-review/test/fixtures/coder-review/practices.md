# Fixture ledger

A two-entry stand-in for `docs/coder/best-practices.md`, in the same heading
shape so `ledger:<id>` refs resolve the way they do against the real file. The
tests use this rather than the live ledger so a real entry being renamed does
not turn a review test red for a reason that has nothing to do with reviews.

## Tool habits

### T1. Batch independent commands into one tool call — `adopted`

Independent commands go in one call. Detection: a trajectory with consecutive
single-command shell calls that do not depend on each other.

## Measurement

### M1. One lever per cycle — `adopted`

One change per measured cycle. Detection: a cycle whose diff touches more than
one improvement axis.
