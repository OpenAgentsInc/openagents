# Synthetic meeting follow-up comparison

The actual `openagents plugin template prepare/run/check` CLI ran on scratch
state with a temporary home and no model credentials, using the public
[synthetic notes](../examples/meeting.md). The
[snapshot](snapshot.json), [comparison](comparison.json), and
[check](check.json) preserve exact source, package, program, and result digests.
The [protected expectations](../../../crates/coder/fixtures/workflow-template/protected.json)
stay outside the guest's captured file scope.

The guest returns two cited items with the template and zero with an empty
captured scope. The unassigned item remains unassigned; the completed item is
counted and excluded. Both attempts use the same retained guest and SnapshotRead
profile, empty request text, and bounded native runtime. The comparison measures
file access and deterministic extraction. It is not a model benchmark, time
saving estimate, genuine customer adoption, or paid-use evidence.

Reproduce the owning process regression with
`cargo test -p openagents-cli --test workflow_template` under a build lease.
It also verifies installation stays off, generic execution refuses the snapshot
requirement, old approvals fail after input/recipient changes, unrelated private
files and protected labels are absent, and failed reports fail the separate
checker. Native template and runtime tests cover a prior synthetic customer,
scope denial, changed rights, cancellation, and partial/truncated results.
These records grant no disclosure, publication, customer acceptance, or payment
authority; their customer and attestation flags remain false.
