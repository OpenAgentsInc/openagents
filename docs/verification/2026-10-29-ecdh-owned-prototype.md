# Experimental owned ECDH prototype

**Source-audit security correction:** the historical aggregate-source acceptance
or conditional decision in this record is superseded and untrusted until
the exact graph and premises are revalidated with the corrected analyzer.
See the [checked-cast advisory](2026-10-29-secret-audit-checked-cast.md).

This is partial work toward the owned lifecycle, not completion of ECDH or
approval for production secrets. The one-slot host has a private, statically
initialized work region and separate result staging. A Coax state policy
tracks work, staging and a private output through success, cancellation and
wipe failure. A nonblocking atomic guard serializes operations; a busy return
means the attempted call took no action. A pending return means cleanup or
result transfer is incomplete. It must not be treated as completed erasure.

The caller must supply valid, live, stable, disjoint input regions and an
exclusive private writable output. Output is marked dirty before copying and
must remain private across pending cleanup; a failed stage wipe retains the
same generation and requires both staging and copied output to be wiped
before an error is completed. A caller must preserve the output allocation and
retry or cancel on the original handle. A handle is a bearer value for this
loaded process instance; fork, reload, restart and unauthorized copies are
outside the contract. A trustworthy synchronous whole-region wipe effect,
serialized caller use and absence of process failure remain premises.

The [record](2026-10-29-ecdh-owned-prototype.json) pins the source, compiler,
host fixtures and independent oracle. Eight Coax checks, including 378 finite
state cases, passed and all 222 authored policy mutants were killed. Direct
and injected host tests passed in checked and release native, native
ASan/UBSan and Wasm builds. Native injected tests exercised reentry and a
competing thread; no threaded Wasm claim follows. The owned path matched 783
official, adapted and adversarial raw-x cases in each checked/release native
and Wasm run, including a seeded default-hash negative control. Another 266
Wycheproof cases are excluded by strict x-only adaptation. Native checks
input preservation; the Wasm fixture checks output guards and error output,
but does not check input preservation. The prior raw audit
remains refused; its separate reviewed guard decision is conditional on the
exact historical Coax source graph and stated caller/tool premises.

Independent source review found no concrete modeled dirty-region completion
or host cleanup bypass under the stated synchronous effect and ownership
premises. It did not rerun the tests or establish physical erasure. The
legacy caller-workspace wrapper and raw stage are still externally accessible:
metadata refusal can leave unadmitted caller work untouched, and cleanup
pending can retain dirty owned work. The prototype does not close those paths
or the ECDH task's literal acceptance. A separate ABI migration is required.
Hidden compiler, register and ABI copies, crash cleanup, target or JIT timing,
and production-secret suitability remain unproved.
