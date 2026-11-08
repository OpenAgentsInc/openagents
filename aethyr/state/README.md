# Declared state, profile 1

`declarations.coax` admits stable numeric IDs **and full names**, typed defaults,
bounds, scope/owner, save participation, retry participation and reset masks.
`lifecycle.coax` prepares a private next store at the existing paid fixed step.
NEW, ENTER, LEAVE, DAY, LOAD, RETRY and EDITOR share the admitted defaults.
Epoch, base revision and persisted operation sequence reject stale or repeated
commands. Clock-only steps do not consume an operation sequence.

`variants.coax` compiles typed predicate DAGs, with no string evaluation. Exactly
one variant must match. Its six RCM1 roots are placements, collision, navigation,
fluid, lighting and render, in that order. A ticket owns six distinct references;
publication revalidates the facts and every live ready reference, then replaces
one `StateWorld`. Retire the previous world's references **after** publication.
Never publish a fact independently of the resources its predicates select.

`codec.coax` adds the bounded FST1 body inside the existing AESV container; it
replaces neither AESV nor ACK1. `restore.coax` rebuilds the selected resources
before publishing a load or retry. Saves contain facts, schema identity, clock
and operation watermark, never caches, pointers or resource claims. A temporary
retry copies only fields declaring retry participation. External committed
progress remains in its existing domain and is never part of that snapshot.

## Wire and migration

FST1 format 1 has 18 header words, 16 words per saved field, and eight SHA-256
lanes: at most 538 LE64 words. Header order is magic, format, schema ID,
revision, eight schema-hash lanes, epoch, tick, day, phase, sequence, field count.
Each entry is field ID followed by kind, integer, three f64 bit patterns, entity,
generation and eight content-hash lanes. SHA lanes are numeric big-endian digest
chunks encoded as LE64 words. The trailer hashes all preceding bytes.

Schema identity covers full names, defaults, bounds, participation and ownership.
An approved migration names both complete schema hashes. Defaults, bounded
clamps and discarded source fields require explicit masks; the reader reports
defaulted/repaired fields. It refuses newer or unknown schemas without that
approval, unknown IDs, duplicate IDs, stale references, nonfinite values,
truncation, corrupt hashes and owner/kind coercion. Revisions of the same schema
can migrate; foreign schema IDs cannot. Existing HH_Save v1 bytes are unchanged:
`verse/haunt-state.coax` admits their quest step and fired-zone facts through the
shared declarations instead of the old duplicated range logic.

## Bounded supported profile

32 fields, 63-byte printable names, 64 entity/content references, 8 patches per
command, 16 variants, 32 predicate terms, six roots per variant and the existing
64 resource-reference slots. Integer/scalar bounds are within ±1e12; counters
cap at 1e9; day length is 1..5,529,600 paid ticks. Scalar/vector values are finite.
Missing or ambiguous variants, resource capacity, invalid snapshots and stale
owners return typed statuses without changing the published world.

## Run the adoption fixture

```
COAXIAL_RUN_STEPS=250000000 coax/web/wasm.sh aethyr/state/fixture.coax apps/openagents-web/state-lab-face.c apps/openagents-web/build/state-lab.wasm
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:9379
coax/web/tool.sh aethyr/state/tests/run.coax 60 /tmp/state.aesv
coax/web/tool.sh aethyr/state/tests/browser.coax http://127.0.0.1:9379/worlds/state-lab /tmp/state-browser
python3 aethyr/state/tests/oracle.py
```

The playable `/worlds/state-lab` uses the shared walking, navigation, fluid query,
input and single 64 Hz frame clock. Break the gate, change the water or schedule,
save/load, capture/retry, jump a day, reset, or inject a failed resource claim.
All four facts select twelve combinations of the same six resource roles.
The fixture uses decoded CPU descriptors and Canvas presentation; it does not
claim a general GPU baked-lighting implementation. Its small immutable PNG is
an actual admitted lighting root. Host C/JS only projects memory and SDK effects.

The credit record is a checked **model of an external durable domain**, not a
wallet or server reward service. It is stored separately from FST1 and cannot
be rolled back by local retry/load. Player pose remains transient walking state,
outside this fact-save profile. Existing realm physics snapshots, account CAS,
HH_Save v1 other fields, forge documents and private realm flags remain their
existing adapters; adopting those is explicit future work, not automatic migration.

See the [qualification record](../../docs/verification/declared-state-12991/README.md).
