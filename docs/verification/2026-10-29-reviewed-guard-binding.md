# Reviewed guard binding for the variable-base source profile

**Source-audit security correction:** historical aggregate-source results and
conditional decisions in this record are superseded and untrusted until
the exact graph and premises are revalidated with the corrected analyzer.
See the [checked-cast advisory](2026-10-29-secret-audit-checked-cast.md).

This opt-in decision covers the reviewed `smc_multiply` source profile conditionally. The classified auditor's output remains intact: `accepted=false`, `error=guard-obligations`, and all three secret-derived assertions remain visible. The separate `source_scope_decision` reports `source_scope_accepted=true` only when the exact reviewed registry entry matches. It does not approve emitted objects, Wasm or JIT timing, erasure, or production-secret use.

The outer `run.py` launcher is an explicitly trusted reviewed code anchor. It pins the inner verifier and registry policy and checks their identities before and after running the verifier. Its own recorded hash cannot authenticate itself: changed launcher code requires independent review and trusted deployment. The inner `check.py` emits only a provisional registry match; it does not itself issue the source-scope decision. The launcher preserves the raw audit and validates the provisional result before producing the separate decision. Changed pinned inputs refuse; the registry is never silently refreshed.

The decision also trusts the pinned compiler and native/Python/solver runtime and operating system, the independent manual correspondence between the field model and exact source/caller trace, and the caller's assertion that the peer is public and the scalar secret. The reviewed entry admits raw peer limbs only up to eight times their 26-bit or 22-bit masks before field arithmetic; the reviewed normalized caller closure supplies canonical field32 operands. Hashing a report or rerunning arithmetic queries cannot establish source-to-model equivalence. The pinned [guard reproof](2026-10-29-secp-field-guard-reproof.md) records the manual review and the [classified audit](2026-10-29-classified-root-audit.md) records the source-flow analysis and caller classification.

The verifier does not accept a caller-supplied audit or proof. It checks its pinned review records, proof report, model, compiler, policy and runtime identities; builds the source audit from the pinned tool closure; constructs the reviewed classification for the exact root; and reruns both the audit and all 19 pinned solver queries. The Coax registry policy compares the ordered source closure, auditor-tool closure, resolved type inventory, classification, compiler profile, complete guard records including origins and locations, proof/model and review identities, runtime and audit status. Missing, extra, duplicate, altered or unsupported inputs do not match. Source and tool files are checked before and after the live computations; the compiler-recorded source map is checked when the registry policy is built. The runtime profile is restricted to the recorded macOS arm64, Coax 0.22.0, native toolchain, Python and Z3 identities. Another profile refuses until separately reviewed. Build artifacts may have different hashes in another checkout because generated paths differ; actual local artifacts and source maps are recorded, and the trusted toolchain and verified source inputs are part of the premise.

This is not a signature or remote attestation. It assumes the trusted code, filesystem, executable loading and operating system execute the verified inputs faithfully; it does not defend against transient replacement of trusted code during execution or a forged process. Python must run isolated without site initialization or bytecode cache reads. The launcher and verifier reject loader and compiler overrides and clear the child environment except for fixed PATH and locale values. The proof process runs in a private empty directory; before Z3 import, a guarded loader permits only the exact reviewed solver path and checks its bytes before loading. The trusted interpreter, system libraries and stable local filesystem remain premises.

The [evidence](2026-10-29-reviewed-guard-binding.json) records the real-source result, raw guard records, provisional match, decision, source/tool identities, registry and solver hashes, runtime and test counts. Two policy checks and 35/35 full policy mutants passed; an earlier policy mutation pass found four survivors, killed by additional literal-bound checks. Twelve integration tests cover a live positive run and refusal of changed sources, tools, profiles, labels, guard records, proof/model/solver and review identities, malformed input and unsupported execution. A hostile working-directory library with a constructor does not load, and solver or compiler override variables refuse. Filesystem-drift and post-audit-failure controls verify that a later refusal preserves the raw audit. No mutation exemptions or increased audit limits were used.

To reproduce on the pinned host and toolchain, choose a fresh unused bytecode-cache prefix and a report path that does not exist:

```sh
coax/bin/coax test tools/coax-nostr/guard-binding/policy.coax
coax/bin/coax mutate tools/coax-nostr/guard-binding/policy.coax --mutation-stats
work_dir=$(mktemp -d)
python3 -I -S -B -X "pycache_prefix=$work_dir/unused" tools/coax-nostr/guard-binding/test_binding.py
python3 -I -S -B -X "pycache_prefix=$work_dir/unused" tools/coax-nostr/guard-binding/run.py --reviewed-variable-base --report "$work_dir/decision.json"
```

The launcher exits zero only for this conditional source decision. Without the explicit flag, with a changed pinned input, or on a different unreviewed runtime, it refuses. The underlying audit result and broader approval remain unchanged.
