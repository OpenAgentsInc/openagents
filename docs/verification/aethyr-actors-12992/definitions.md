# Typed archetype compilation (#12992, partial delivery)

Base: `a70321f380`, the previously delivered phase compiler. The new definition
compiler resolves authored defaults/inheritance, service inputs and declared
fact/resource bindings before an actor runtime can use them. This is not a
completed actor issue or a claim of playable adoption.

## Boundary

Typed Coax authoring is the supported source profile. Unknown struct fields
fail in the language; numeric parameter keys/types and duplicate overrides
fail at definition admission. No JSON or durable actor wire format is added.
All library identities, parent cycles and declared parameter/service/state/
resource references are admitted before flattening the requested lineage.
Invalid results contain an empty invalid archetype, not a partial candidate.

The owned output contains stable ID/revision and complete lineage, fixed
parameters, native service definitions and exact input slots, the shared phase
plan, typed fact slots plus full schema identity, and resource roots plus
complete manifest identity and deduplicated dependency closure. Source-table
changes cannot rewrite an already returned record. It holds no input views.

RCM1 canonical reserialization/hash checks prevent altered costs or dependency
edges from retaining the original manifest pin. Actual payload identity,
decode success, resource reservation and aggregate instance/work admission
remain the existing runtime authorities and the next actor integration work.
The same content under a different ResourceSpec cannot alias a root.

The definition compatibility matrix admits kinematic stationary/route/rider
controllers with absent/in-place animation; riders require in-place animation.
It rejects rigid/root-driven combinations. No physical execution, nav result,
model result, action timing, entity flush or lifecycle hook is qualified here.

## Checks and mutation

The new checks cover all parameter defaults/types and exact bounds, NaN and
both infinities; band ordering and controller/animation/physics combinations;
bad/duplicate identities and references; cyclic parent/service graphs; source
ownership; manifest substitutions; full/empty replacement groups; and minimum
and maximum counts. The maximum metadata fixture has 16 inheritance levels,
32 catalog services, 20 overrides, eight fact bindings, eight roots and a
64-owner transitive closure. It tests metadata admission, not decoding those
synthetic sources. The ordinary three-resource fixture derives its plans from
real four-byte index payloads using the shared builder and reader.

The final entry-source mutation gate kills **316/316 mutants, zero survivors**;
all loaded dependency checks also run. The earlier phase source/checks remain
unchanged, so their 137/137 result at `a70321f380` is reused. The new gate
does not claim to mutate the whole imported engine again.

The first definition gate retained 59 survivors, then seven. Exact boundary
and diagnostic checks were added, duplicate range validation was removed where
the admitted catalog already proves the role, cycle visitation now uses a
bounded boolean array, and the compiled record retains exact service inputs.
No mutants were waived or marked equivalent. Initial, intermediate and final
logs are retained alongside this record.

## Independent native evidence

`definition-cases.coax` executes the actual emitted compiler on 64 authored
cases: 32 complete archetypes and 32 typed refusals. Each successful row
contains all parameter value lanes, revision lineage, services and exact input
slots, task order, fact bindings, resource roots/closure and both complete
identities. The separate Python oracle independently computes expected values,
the declared schema's canonical SHA-256 and all RCM1 bytes/hash. It checks each
word, exact result length and the complete case set. These are definition
records, not simulated actors or rendered frames.

The 44,607-byte observation text has SHA-256
`da9a3d30080a4438d5646b9b1b5d461cb9b33abf06c10becfb0ed5427f33ddda`.
It is retained as `definitions-native.txt`; the independent oracle can run
against it without rerunning the compiler.

The same normal emitted C (898,174 bytes) is compiled with GCC 15.2.0,
`-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all`,
`-fno-omit-frame-pointer -fno-fast-math -ffp-contract=off`. ASan/UBSan with
leak detection enabled reports no diagnostics; all output bytes match the
already-oracled native observations. Native stack settings are unchanged.
Owner-local generated C, executable and both outputs are retained below
`~/.openagents/aethyrforge/actors-12992/definitions-v2/`.

Compiler: Coax 0.52.0, SHA-256
`bd13739490a65b8969c6fb4298ca8f94264416e41ad1a146ca8170fdec8acacc`.

| Source | SHA-256 |
| --- | --- |
| `aethyr/actors/definitions.coax` | `9c1789958d989e8603e135fde50773380c9e39b5e7983ced992501ae5b73a8bb` |
| `aethyr/actors/tests/definitions.coax` | `cf69394096dbeb912b7d9dff55aa8558e361b2e2d911e5b4a1314428e8e02048` |
| `aethyr/actors/tests/definition-cases.coax` | `1aa4955a8f7fb80b7d87386fc54c8f7320fbd9235136521f028504c4405a39b1` |
| `aethyr/actors/tests/definition_oracle.py` | `0ae2faea4fd5a5270bfd80e54ccf8fbb2cecb7e56f2aac5f51f2ae0f436297a5` |

## Remaining issue requirements

Actor instances and lifecycle hooks, aggregate pool admission, bounded
behavior/weighted-choice/schedule/perception execution, current-owner nav/model
result admission, rider input parity, real guard/villager adoption, resource
retirement, fixed-step native/Wasm replay and captures remain. No existing
realm scheduler is migrated by adding these compilers, and no chapter status
is promoted. The module README tracks this complete residual scope. #12992
stays open.
