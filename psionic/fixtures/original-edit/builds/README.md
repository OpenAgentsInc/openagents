# Observed builds

`matrix.sha256` names the executable, generated C/header, Metal source and
experiment sources used by the development-baseline batch. `held.sha256`
names the later build used by held-out and supplementary controls. Hashes
are observations, not a promise that a fresh compiler build is bit identical.
Generated C and executables remain ignored build outputs, not fixtures.

The exact historical root Coax sources are retained as inert `.coax.txt`
files: `baseline-{run,data}` match the corresponding source hashes in
`matrix.sha256`; `held-{run,data}` match `held.sha256`. The independent oracle
checks those matches. Import dependencies already in the repository can be
read at base commit `eead95dc9d`; new window/proposal/study modules are in the
chapter's implementation commit. The report/preparation/check wrapper may
change without rebuilding the frozen model executable.

The initial screen/stress binary hash and total command wall time were not
captured. Their saved outputs, original windows, reference parity vectors
and per-generation milliseconds remain, but their binary provenance is less
complete than the later batches. No later hash is retroactively attributed
to that initial executable.
