# Independent evaluation qualification

A successful engine process is only one input. `coder/tests/qualify.coax`
collects independent acceptance and root-only mutation results, verifies sealed
files and read-only turn checkpoints, and writes one machine-readable receipt.
The [artifact contract](../protocol/coder-qualification-v1.md) defines the fields.
The [fresh evidence](../verification/coder-eval-gates-2026-11-03/README.md)
contains a current issue policy prototype and an eight-turn conversation.

Use a dedicated worktree and a qualified compiler. Paths in seal lists must be
absolute, one per line. Protect the external acceptance source, instructions,
task and every existing file the model must preserve. Capture the seals before
inference, outside the model's allowed edits. Keep the supervisor/evidence
outside its writable surface. The collector detects observed changes; it is
not an operating-system sandbox or a complete repository scope checker.

```sh
export COAX=/absolute/path/to/qualified/coax
coax/web/tool.sh coder/tests/qualify.coax seal --list /tmp/protected-list.txt --out /tmp/protected.json
coax/web/tool.sh coder/tests/qualify.coax seal --list /tmp/read-only-list.txt --out /tmp/before.json
# Run the read-only phase here.
coax/web/tool.sh coder/tests/qualify.coax seal --list /tmp/read-only-list.txt --out /tmp/after.json
coax/web/tool.sh coder/tests/issue.coax --driver /absolute/coder --driver-repo /absolute/driver-worktree --coax "$COAX" --home /tmp/isolated-home --repo /absolute/task-worktree --task /tmp/task.txt --out /tmp/run.jsonl
coax/web/tool.sh coder/tests/qualify.coax verify --repo /absolute/task-worktree --coax "$COAX" --accept /tmp/oracle.coax --unit /absolute/task-worktree/unit.coax --seal /tmp/protected.json --checkpoint-before /tmp/before.json --checkpoint-after /tmp/after.json --execution /tmp/run.jsonl.execution.json --run-log /tmp/run.jsonl --out /tmp/fresh-qualification
```

For an issue without a read-only phase, a matching pre-run pair proves only
the baseline snapshot; it is not a continuation claim. For conversations,
`--checkpoint-chain FILE` replaces the pair with all observed read-only pairs.
`coder/tests/conversation.coax --gate /absolute/qualify --driver /absolute/coder
--driver-repo /absolute/driver-worktree` creates a fresh eight-turn fixture,
seven checkpoint pairs and a final independently qualified edit. Build these
native tools first with `coax/web/tool.sh` and use its cached binaries.

The production issue driver uses Codex and a 180-second budget. The conversation
uses Microcoder, seven 55-second read turns and a 115-second implementation
turn, with a 600-second aggregate budget. Each independent compiler invocation
is bounded at 120 seconds. The local collector smoke accepts `--gate BIN` and
tests positive qualification plus engine, acceptance, protected-file and
checkpoint refusal. Explicit `--engine-exit`, `--elapsed-ms`, `--budget-ms`
arguments exist for synthetic supervisor fixtures; they are self-attested facts,
not substitutes for actual execution evidence in live qualification.

Seals permit 1–64 distinct paths, files smaller than 2 MiB, and check size/mtime
around each hash plus canonical target identity on verification. Chains permit
1–32 pairs. Process capture keeps 256 KiB stdout and 64 KiB stderr; exhausted
capture refuses qualification. Logs and proofs must fit their declared bounded
buffers. Existing output directories and existing seals are never overwritten.

Acceptance and mutation require complete, unique compiler-owned summary lines,
consistent totals and successful command exits. Mutation requires a positive
count, every mutant killed and zero survivors. The evaluated unit cannot
contain `mutant: equivalent`. This conservative source rule is not a recursive
annotation audit of imported dependencies. Protected files are rechecked after
the independent commands. Nonzero exits, missing facts, timeout, cut output,
failed artifact writes and missing/changed seals cannot qualify.

The trusted supervisor must enforce the task's protected-path list and number
of required checkpoints. Also inspect Git status/diff for unrelated edits,
confirm original tests survive, and verify every conversation transcript against
its digest list. A receipt does not establish broad engine superiority, repair
an entire issue, or qualify untested server/client integration.
