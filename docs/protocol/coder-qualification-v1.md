# Coder qualification evidence v1

[Schema](../../proto/openagents/coder/qualification/v1/evidence.proto).
These are local supervisor-owned evidence artifacts, not model replies or a
remote authorization protocol. They do not grant execution permissions.
Existing observability versions retain their identity.

The Coax JSON mapping uses snake_case field names, JSON numbers for integer
fields, explicit booleans, and lowercase 64-character SHA-256 strings. It is
an application JSON mapping, not protobuf's canonical JSON encoding. Unknown
fields confer no authority. Required decision facts must be present.

| Message | `schema` identity | Meaning |
| --- | --- | --- |
| Seal | `coder.seal.v1` | 1–64 distinct absolute requested paths, resolved paths and coherent file hashes |
| Checkpoints | `coder.checkpoints.v1` | 1–32 before/after seal pairs; each pair's serialized seals must match exactly |
| Execution | `coder.execution.v1` | Actual supervisor exit, elapsed time, budget, timeout/truncation flags and bound run-log hash |
| Qualification | `coder.qualification.v1` | Independent acceptance, mutation, protected bytes, checkpoint and artifact decision |

A qualified receipt requires engine exit zero within a positive budget, no
supervisor timeout/truncation, unchanged protected files and every checkpoint,
independent acceptance success, positive mutation count with all mutants killed
and no survivors, and successfully written evidence. The benchmark refuses
`mutant: equivalent` annotations in the evaluated unit. Failed or unobserved
command statuses/counts may be -1; they cannot qualify. Counts are never inferred
from model prose. Compiler-owned summary footers must be complete and unique.

The supervisor and artifact directory are trusted. Seals establish observed
bytes at named boundaries, not continuous filesystem isolation. Checkpoint
snapshots must actually be collected around each protected turn. A consumer
must not infer the number of conversation turns from the presence of one pair.
A digest-list log binds the list itself; auditors also check each listed
transcript digest. See the [operating guide](../coder/evaluation-qualification.md)
for limits and reproducible commands.
