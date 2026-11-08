# Retained Long Rest activity journal

Base: `8374a1b892`. Source: pinned SRD 5.2.1 p185, as recorded in
`docs/dnd/long-rest-implementation.md`.

`long-rest-state.coax` owns explicit sleep/light/exertion segments, retained
progress, interruption count, pause/resumption and completion history. Clock
passage never starts sleep. The owner must synchronize the previous mode before
changing activity or reporting damage/casting/Initiative. Interrupted time is
not credited. An interruption reports optional Short Rest eligibility once;
resumption clears that segment's eligibility while preserving prior progress
and adding the source's extra hour.

Advancement stops at the first completion, one-hour accumulated exertion, or
two-hour light-activity boundary. The light boundary requires the owner to
select the next actual activity; it neither invents sleep nor silently credits
excess light activity. Completion is found with 63 bounded binary-lifting
steps, including signed 64-bit maximum timestamp tests. Unsigned intermediate
offsets avoid signed overflow. Completion becomes pending; only acknowledgement
after benefits application records its actual timestamp and starts the
sixteen-hour exclusion. It does not use the late poll's timestamp.

## Verification

- `coax/bin/coax test aethyr/rules/srd/long-rest-state.coax --library`:
  nine composed checks pass (seven new state checks plus two unchanged policy
  checks). Tests preceded implementation and were sharpened against survivors.
- `coax/bin/coax mutate aethyr/rules/srd/long-rest-state.coax --library --unit aethyr/rules/srd/long-rest-state.coax --jobs 4`:
  259 mutants, 259 killed, zero survivors or exemptions.
- `coax/web/tool.sh aethyr/rules/srd/long-rest-state-native.coax`:
  `PASS LONG-REST-STATE (16 activity/poll schedules)`. Actual compiled execution
  varies watch duration and poll lateness, including a long pause, interruption,
  Short Rest eligibility, exact completion and sixteen-hour readmission.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, zero missing
  reasons. No product/world ABI changed; the unchanged previous world build
  receipt was not rerun.

Compiler SHA256:
`52402aa1520ff7f267ce7c44b3d3dfa8e9dacc868bf7d84d95cdf95a0586d7f1`.
The compiler reports its existing source-version advisory; no bootstrap was
needed. State SHA256:
`6d5b526910ef90fa03a010c8861db4df251a2a8b655a431f2289555d470c5f18`.
Native entry SHA256:
`3e760638535c96b32498d99173c2f6cbb5a4f06cc4b3ca5a50ee33b9eca6e511`.

## Required integration remains

This is a private, trusted journal, not an untrusted serialized-state decoder.
Its functions require nonempty owned state and the owner must preserve its
reachable invariants; arbitrary forged counters are not admitted world state.

It has no world completion caller yet. Sleep/Unconscious ownership, authoritative
clock conversion/scheduling, causal producer synchronization, atomic full
benefit application and actual rest controls/rejoin persistence remain required.
Short Rest eligibility is an event for the owner, not an automatic resource
reset or healing grant. No existing Short Rest behavior was replaced. Issues
#13040/#13044/#13045/#13046 remain open.
