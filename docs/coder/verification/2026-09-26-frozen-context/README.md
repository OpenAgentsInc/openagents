# Frozen context and shared transport acceptance

Issue [#9676](https://github.com/OpenAgentsInc/openagents/issues/9676) binds
independent task checks and optional knowledge to the exact admitted context.
The [runtime contract](../../runtime/frozen-task-context.md) describes v2 source
lineage, retained Markdown, exclusions, and v1 replay compatibility.

- [Task tests](task-tests.log): 43 pass, including false-green refusal, protected
  suites, correction/disputed checks, exact knowledge, excluded provenance,
  and retained v1 contexts and typed reports.
- [Model-context fixture](model-context.log): the existing Microcoder loop
  receives the exact admitted text and lineage. The `off` profile refuses it.
- [Legacy adapter replay](legacy-admission.log): the landed grant and task
  serialize to exactly their original JSON values with no new null fields.
- [Strict Clippy](clippy.log): the Coder and labor packages pass all targets.
- [Shared artifact transport](artifact-transport.log): all 12 labor tests pass
  after moving authenticated private-artifact transport into Coder, including
  the actual encrypted relay and separate provider/buyer process fixture.
- [Container controls](container-controls.log): two explicitly admitted Docker
  fixtures pass output/artifact retention, whole-container removal, and
  cancellation of a delayed descendant. This is part of #9674, whose detached
  and fresh model-run acceptance is still separate work.

These tests establish host contracts, not source provenance attestation,
knowledge benefit, benchmark quality, or completion of the mobile/OS migration.
Historical v1 tasks remain readable; a new execution needs v2 requirements.
No official benchmark outcomes or paid model responses were used here.

The [manifest](manifest.json) records exact bytes for these retained logs.
