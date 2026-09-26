# Frozen task knowledge and check lineage

A task's execution grant can include exact local Markdown knowledge in its
`requirements`. This is an explicit context input, not retrieval, admission of
a learned method, network access, or permission to execute tools. The repository
adapter requires `adapter_configuration.knowledge: "frozen-context"` for these
inputs; `off` refuses them. The host retains the same context that the adapter
passes to its model.

New requirement sets use `openagents.coder.task-requirements.v2` and contexts use
`openagents.coder.task-context.v2`. Every requirement set names `task_sources`, `source_exclusions`, and
`check_lineage`. Each protected check has exactly one lineage record containing
its check ID and a nonempty list of source IDs. Each optional `knowledge` input
has this shape:

```json
{
  "id": "git.recovery",
  "version": 1,
  "digest": "sha256:<exact document SHA-256>",
  "path": "context/git.recovery.md",
  "sources": ["reference"]
}
```

The path is relative to the admitted workspace. The host opens only a confined,
singly linked regular file, parses the existing knowledge entry format, and
checks the exact document digest, ID, positive version, non-withdrawn status,
and `provenance.written_from` against the grant. A changed document, symlink,
missing file, unknown provenance, or mismatched identity refuses admission.
The host retains the exact UTF-8 text and pin in the context manifest. It reads
at most 16 entries, 64 KiB per entry, and 256 KiB in total.

Knowledge and check sources must not overlap the current task's declared source
IDs, its task ID, or `source_exclusions`. Matching checks both literal IDs and
run-name normalization, so a trailing numeric run ID cannot bypass a source
exclusion. Every check still requires its separate externally pinned executable,
capability manifest, coverage map, candidate identity, and typed evidence.
Source lineage does not replace those independent checks.

These are operator and document **provenance declarations**. Exact-byte
validation establishes which declaration and content were supplied; it does
not independently prove an author's account of where knowledge originated.
The host cannot discover an omitted source or establish out-of-sample benefit
from a declaration. The fixed entry text, check lineage, exclusions, scoped
instruction files, effective prompt, and revision all participate in the frozen
context digest. Replay validates the retained text and metadata against the
original grant. Later workspace edits cannot rewrite what the model received.

Corrections preserve the earlier context and effects, mark it superseded, stop
pending work, and dispute earlier acceptance. A changed source declaration or
knowledge version requires a new exact grant; it does not silently relabel the
old run. Private decryption, ambient knowledge loading, and remote lookup are
outside this path. A `bounded-command` adapter retains this context without
claiming that an arbitrary external command passed it to a model.

The landed v1 requirement and context formats remain readable with their
original serialized shape, digest, and typed check reports. They make no new
lineage claim. New execution refuses v1 requirements rather than silently
upgrading an old grant. A new check request on a legacy task returns unavailable;
its old check report remains readable under its original contract. Retained
negative results and their costs stay unchanged.

See [the task owner](task-owner.md), [knowledge evidence](knowledge-evidence.md),
and [immutable/private bundles](knowledge-bundles.md) for the separate execution,
measurement, and delivery boundaries.
