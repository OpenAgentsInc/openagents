# Final compiler binding for the rebased B3 consumer

[The final record](record.json) binds the coherent R2 image
`710c4f5708d4c7ccbbe3207f89b9e9bb141860988ec9d98e71eca7a14712ac4b`
to the [fresh post-rebase native witnesses](../README.md).

The final image regenerated **all 11 C/header artifacts** for the raw owner,
canonical typed request, actual Lab consumer, factory/defaults and RED/GREEN
entry checks. Every byte matches the fresh witness artifact. The complete
checked source closure, actual native host, test oracles, C compiler and original
receipts also match. The O0/O2/sanitizer observations therefore retain their
original execution identity; no passing native run was repeated.

The final image separately ran all 42 adapter/dependency check groups and
authority coverage. It killed all 32 typed-outcome and 34 owner-policy mutants
with zero survivors and no equivalent marking. Intentional misuse again
reported E0907 at line 2, E0904 at line 3 and E0383 at line 4.

[qualification.json](qualification.json) records the equality and complete
source/tool/native-input checks. Its temporary orchestration script completed
all 12 commands successfully, then failed while serializing a tuple-key metadata
field. [The original receipt](execution-before-finalization.json) and
[explicit failure summary](metadata-failure-summary.txt) remain unchanged.
A separate [finalizer](finalize.py) independently read back all outputs,
diagnostics, mutation inventories and input identities without rerunning a
stage; it produced the final passed receipt. The original script is retained as
[rebind.py](rebind.py), including the metadata typo.

[The archive](proof.tar.xz) and [read-back manifest](manifest.json) retain the
new emissions, actual logs and finalization inputs. The compiler's own coherent
source/model/bootstrap qualification belongs to the overarching R2 record.
The canonical-owner and caller-storage preconditions and the separate full-app
E0381 readiness gate are unchanged.
