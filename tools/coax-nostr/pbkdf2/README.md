# Resumable PBKDF2-HMAC-SHA256

`coax/lib/crypto/pbkdf2.coax` implements the byte primitive and work/phase
policy in checked Coax. The memory adapter observes actual borrowed-view
overlap. The optional worker combines that primitive with the existing
identified-buffer erasure effect; it adds no compiler or app build dependency.
Python is explicit test tooling only.

This slice **does not replace** `coax/web/unlock/gate.ts`, AES-GCM, the sealed
pointer format or the current phrase reader. Format review, consumer migration,
secret-copy review and production cutover remain separate tasks.

## Public budgets and API

| Resource | Bound |
| --- | --- |
| Password and salt | Each 0–1024 bytes |
| Derived output | 1–256 bytes |
| Total HMAC evaluations | `rounds × ceil(length/32)` ≤1,000,000 |
| One resumable step | 1–1024 HMAC evaluations; worker default 256 |
| Caller state | Exactly 832 bytes, private process-local format |

The work gate divides before multiplying, so hostile signed-64-bit metadata
cannot overflow the budget calculation. These are local resource bounds, not
the mathematical maximum of PBKDF2. Input text encoding/normalization belongs
to a versioned caller profile; the generic primitive accepts arbitrary bytes,
including empty inputs and embedded zero bytes.

Load `pbkdf2-memory.coax` for the native API:

- `pbkdf2_init(state,password,salt,rounds,length)` initializes caller storage.
- `pbkdf2_step(state,budget)` returns error, phase, completed and total work.
- `pbkdf2_info(state)` reports admitted progress without deriving anything.
- `pbkdf2_result(state,out)` releases only the complete requested result.
- `pbkdf2_cancel(state)` marks the job stopped and blocks all output.
- `pbkdf2_gate_init` and `pbkdf2_worker_request` enforce the legacy profile
  and worker metadata before input transfer.

Phases are active 1, done 2 and stopped 3. Cancellation is idempotent and can
also invalidate an unreleased finished result. A completed step is idempotent;
an invalid budget does not advance state. A step computes in a bounded local
working buffer and commits once. Invalid input/state, early result, capacity
and alias refusals preserve caller state/output. Initialization does not
silently destroy a previous caller context on refusal; that caller still owns
its cleanup obligation.

The state precomputes keyed HMAC inner/outer contexts and another HMAC context
with salt already absorbed. Each round copies these prefix states, rather than
rehashing the password. The previous U, XOR accumulator and up to 256 staged
output bytes live in caller state. No borrowed password/salt pointer survives
initialization. Multi-block results are never released partially. Conservative
whole-view aliases between state and inputs/output refuse; read-only password
and salt views may overlap.

Local statuses are success 0, bad state 1, input 2, output capacity 3, phase 4,
alias 5, work 6, budget 7 and cancelled 8. Internal HMAC refusals propagate as
nonzero failures. A decoded or admitted context is not authenticated remote
state. Callers own truthful live writable pointers and serialized access.

## Legacy profile and compatibility

The actual current gate derives a 32-byte AES-GCM key with SHA-256 PBKDF2,
**210,000 rounds** and a **16-byte salt**, from 4–40 normalized ASCII
lowercase letters/digits. Its TypeScript normalizer applies `toLowerCase()`
and removes `[^a-z0-9]` before UTF-8 encoding. The Coax gate helper requires
those already normalized bytes; it does not invent a second Unicode/text
normalizer. Its worker profile refuses changed rounds/output lengths instead
of silently interpreting another format.

`gate.ts` still owns the existing AES-GCM 256-bit key/12-byte IV path and the
64-bit salted phrase check. Tests verify those actual source parameters and
pin its bytes in the retained evidence. No salts, production phrases,
credentials or private presentation material are read by these tests.

## Dedicated worker and cleanup boundary

`pbkdf2-worker.coax` is the combined generated policy closure.
`platform/pbkdf2-worker.c` owns one worker's fixed state/password/salt/output
buffers and transports scalars/progress. `pbkdf2-worker.js` owns fetching,
instantiation, transferred ArrayBuffer transport and MessageChannel scheduling.
Coax owns all KDF/work/output admission. Exact JavaScript-to-Wasm signed-i32
transport is checked before coercion; oversized original integers cannot wrap
into valid KDF parameters. Coax preflight precedes copying into fixed buffers.
One worker instance accepts one job at a time.

The worker yields between every bounded step, reports progress every 4096
HMAC evaluations and processes cancellation between steps. A lower admitted
budget (for example 64) reduces each quantum on slower devices; work limits
are not wall-clock latency guarantees. The caller can always cancel. Avoid
running an entire derivation in a loop on the render thread. There is no
exported synchronous whole-derivation helper in this primitive.

After initialization, the C adapter erases its identified password and salt
buffers. Success copies the intended output into a transferred result buffer,
then erases all four identified Wasm buffers **before** reporting done. Invalid
requests, traps and cancellation refuse key release and attempt the same
cleanup; cleanup failure is reported and never produces a successful key.
Every rejected receive path also zero-fills any received mutable input
ArrayBuffers (including an invalid view's backing buffer) before its terminal
response: busy/uninitialized workers, malformed/partial input pairs and ABI
errors share one cleanup helper. A final cleanup guard covers thrown paths.
Caller transport should bound buffers before transfer; erasing a rejected
oversized received buffer still costs time proportional to that buffer.
WASI file/log effects are denied rather than printing data. Test-only
`COAX_PBKDF2_TEST_OBSERVE` adds an observable all-zero check; normal worker
builds omit that export.

The generic Coax cancel call changes phase only: native callers must explicitly
erase the 832-byte state and their own input/result buffers on every exit,
using the platform erasure contract. Worker-managed input ArrayBuffers are
zero-filled after transfer into Wasm, but JavaScript/JIT/GC copies have no
physical-erasure guarantee. HMAC and PBKDF2 local stack/register/by-value
copies, OS copies and the intentionally delivered result are outside the
identified-buffer erasure claim. This is not production-secret or timing-side-
channel approval, and terminating a process/worker is not proof of erasure.

## Focused evidence

```sh
coax/bin/coax test coax/lib/crypto/pbkdf2.coax
coax/bin/coax mutate coax/lib/crypto/pbkdf2.coax
python3 tools/coax-nostr/pbkdf2/test_pbkdf2.py
```

On 2026-10-28, Coax 0.15.0 passed **58 combined checks** and killed
**1,238/1,238 mutants**, with no equivalent waivers. Six targeted tests cover
both RFC 7914 §11 SHA-256 vectors (including c=80,000/dkLen=64), independent
Python `hashlib.pbkdf2_hmac` cases at byte/block/key boundaries, checked and
wrapping-release native C, real aliases/capacities/guards, cancellation,
explicit erasure, invalid original integers, and native ASan/UBSan.

Actual headless Chrome 156 workers execute the Wasm in both compiler modes,
compare against independent WebCrypto, and run a main-thread canvas/rAF
fixture. The retained run on this Mac observed:

| Coax mode | Step budget | Legacy 210k wall time | Frames during work | Largest worker step |
| --- | --- | --- | --- | --- |
| Checked | 256 | 166 ms | 10 | 0.4 ms |
| Checked | 64 | 173 ms | 10 | 0.2 ms |
| Release | 256 | 126 ms | 8 | 0.4 ms |
| Release | 64 | 133 ms | 8 | 0.2 ms |

Cancellation after the first progress message stopped at 4,352/210,000 work
units, released no key and observed all identified buffers zero. These are
single scoped observations after RFC warmup, not statistical throughput,
physical-phone, Safari/JavaScriptCore, engine WebGPU-frame or universal latency
claims. Slower hardware may take much longer; the dedicated worker and bounded
steps preserve the scheduling/cancellation architecture, not a timing SLA.

A separate test-only worker wrapper observes the actual received input buffers
at the terminal-message boundary. Eight real-worker rejection cases in both
compiler modes verify zero contents **before** error delivery and no key
release, including busy/uninitialized, each one-valid/one-invalid pair,
malformed views/scalars/messages and oversized input. The busy worker's original
job still cancels and cleans normally. This JS cleanup revision reran only the
affected worker/browser test; the unchanged primitive/vector/mutation/native
gates retain their earlier passing results.

`evidence/conformance-2026-10-28.json` identifies the actual compiler, source,
fixture, legacy gate and native/Wasm artifact bytes, flags and browser results.
The ordinary compiler/bootstrap/app remain Python-free. This slice changes no
protocol schema, existing gate format, app flow or production crypto provider.
