# Linux pool artifact qualification

The new pool CLI's frozen emitted C was compiled directly on `coderos-4080`
in fresh `/tmp/oct30-pool-linux.LbbBa1`. No compiler bootstrap, full release
gate, model job, public publish, or live Pylon/process/state change was made.

Environment: Linux 6.18.49 x86_64, GCC 15.2.0 from
`/run/current-system/sw/bin/gcc`, glibc 2.42-84, and Nix OpenSSL 3.6.3.
The existing `tls-env.sh` selected cached dev/out derivations and rpath.
TLS was linked; this slice did not requalify a TLS network connection.

Frozen v3 C:
`3339d175e284b6d2dbfd92e7900033a642d11e5504e949626604e4b7864dddde`.
The exact entropy header and native host/effect/socket/TLS/entropy/process
sources were copied before compiling, with every file hash retained. The
checkout base was `cff4d0a4d2f074fb6841e7105e3f52f0e4f29708`; the new public
output effect `.c/.h` were still uncommitted integration files then, so exact
input hashes, rather than a commit-only claim, identify the qualified bytes.
No source was regenerated on Linux.

Both plain and ASan/UBSan v3 artifacts passed:

- Default empty trust policy and sorted/deduplicated role policy, with
  identical JSON between builds.
- Seven invalid mode/budget/window/key cases each, all failing with empty
  stdout and no sanitizer findings.
- One regular-file-stdin verification each of the signed public baseline
  bundle. Both returned `verified:true,current:true` for
  `fa816cfd0f5cf9fb535785f6452b3f5daa910de787780621a80b310b7823c135`.
  Bundle SHA256:
  `81217cadd9c1d1d5138b8a9ce901cd48ab5acc89f8d92cc54906ce4ef76a2a51`.

The v4 C changed the shared malformed non-AUTH observer failure path:
`4f2c8ce08626efb13a2b2b39588c0012efb445cadb5aefa5024b7f03e45af6f2`.
Both GCC variants compiled against the same frozen native inputs. The v3
positive proof was retained without repeating its unchanged checks. Only the
new malformed-frame case ran on both v4 variants: after a fresh local
AUTH/ACK exchange and `mesh-inputs` query, a peer sent
`["EVENT","mesh-inputs"]` followed by EOSE. Both refused in 29/82 ms, with
empty stdout and no sanitizer diagnostics. The dependency-free Linux peer
ACKs AUTH to isolate this new failure path; it is not an independent AUTH
signature verifier. The stronger peer's Mac red/green evidence remains in
the main Chapter21 end-to-end record.

One newly supplied actual-capacity public bundle ran once on v4 sanitizer,
using regular-file stdin. It verified the snapshot of two actual CPU Pylons,
eight slots, one job and one checker pass, returning
`verified:true,current:true` for
`36669dc4fd9b01ffaa4e66e7df63cc9750404f97d0a59150987fe42bac25f459`.
Bundle SHA256:
`78cfdc5dca107c19cba1d0606c15528bd0cba6248e351188fc8df7ca8eed045d`.
The independent Bun all-field recomputation is in the main E2E record.
`current:true` is the status at verification time, not an enduring lease.

The first malformed-case fixture used a literal-IP relay URL, intentionally
unsupported by the existing URI profile. It never reached AUTH/query and was
not counted as a product regression result. That fixture attempt is retained.
Correcting only its URL to supported `localhost` reached the intended branch;
no product behavior was relaxed.

Exact executed scripts, public arguments, source/library hashes and outputs
are under [`data/oct30-pool-linux-artifacts/`](data/oct30-pool-linux-artifacts/).
The reusable isolated fixture is
`psionic/tests/mesh21/pool_linux_malformed.ts BINARY OUT.json`.
Public baseline/actual bundles are retained in
`data/oct30-mesh-e2e/pool-v3/` and `data/oct30-mesh-e2e/public-capacity/`.
No private witness, prompt, plaintext result, opening key, signing secret or
model credential is persisted here.
