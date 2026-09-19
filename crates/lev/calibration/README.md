# Calibration records

One fitted reliability table per question family, per door, written by
`gym eval --fit --records crates/lev/calibration` and read by
`lev-serve --calibration crates/lev/calibration/<door>`.

A record is a claim that a raw signal can be turned into a probability for
one family on one door. It carries what that claim rests on: the door's name
and identity, the base model signature, the adapter when one is attached, the
operating system build, the suite and its content digest, the partition the
map was fitted on, the estimator and the seed block it drew, and the gate that
judged it, by id and by digest. `gym::calibrate::Record::serve_to` checks
every one of those against the door that is running and names the field that
refuses.

## Which records a door actually serves

A record in this directory is a candidate, not a grant. A door started with
`--manifest` serves only the records that release names in its `evalRef`,
with the digest and verdict it recorded, so a record that appears here later
or changes afterwards does not quietly start serving. See
[`../manifests/README.md`](../manifests/README.md).

## Why the directory is per door

Three doors scoring the same suite would otherwise overwrite each other's
records, and the last one to run would silently win. `lev-base/` holds the
maps fitted against the base model with no adapter attached; an adapted door
gets its own directory, because an adapter changes the door and a map fitted
on the base must not survive the change.

## What was here before

Three records — `routing.json`, `severity.json`, `urgency.json` — fitted on
2026-09-19 against the two-partition `support-v2` suite. They were deleted on
the same day rather than kept, and the reason is the fault this whole
directory now exists to prevent.

They named an operating system build and nothing else. A build is identical
for every door on one machine, so the records could not say which model they
were fitted against, and they sat here unchanged through two adapter runs
that altered which families are admitted at all. One of them, `routing.json`,
was marked admitted: the only admitted map in the repository, fitted against
a door nobody could identify, in a directory no door ever opened.

Regenerating them was the alternative to deleting them, and it is what
happened: the files below are that run. Keeping the old ones beside the new
ones was not an option, because an unattributable map is worse than no map —
it reads as evidence and cannot be checked. Their numbers survive in
`docs/lev/calibration.md` and in this repository's history.
