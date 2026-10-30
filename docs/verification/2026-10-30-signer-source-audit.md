# The Nostr signer under the large secret-source audit

#4828's central result: the large source-audit profile analyzes the
complete exact closure of the BIP-340 Nostr signer. Its dataflow completes,
and the only items left are the signer's declared declassifications and two
public authored guards. This is a source-level result under stated premises.
It is not production approval; see "Limits".

## What was audited

- **Entry:** `nostr_sign_step` in `coax/lib/crypto/schnorr_sign.coax`
  (SHA-256 `3e970256…39c1`). It drives all six stages: key, nonce, scalar,
  self-check preparation, self-check multiply, and publish. The self-verifier
  is `schnorr_self_verify.coax` (`d8ef0af2…fb86`).
- **Closure:** 29 source units and 269,195 bytes, compiled with
  `--run-steps 250000000` by compiler `cd94879e…fcfd`.
- **Classification** (`coax.secret-large-classification-views.v4`):
  - `key` and `aux` are secret 32-byte read views.
  - `event_id` is a public 32-byte read view; `out` is a public 64-byte
    write view; `overlap` is public.
  - `work` is a secret one-element `SignWork` view, whose public fields are:
    - `SignWork.stage`, `.challenge`, `.message`;
    - `SignPrepare.public_key`;
    - `SignNoncePoint.r`;
    - `SsvWork.stage`, `.bound`, `.peer`;
    - `ShaOwned.used`, `.phase`, `.total`.

  Every public field holds data the protocol publishes (the public key, R,
  the challenge, the message digest, the lifted public key) or public
  counters.
- **Tool:** large profile `aggregate.coax` (`4782ed8d…66c5`).

## Result

The run reaches 135 procedures in 6,396,067 analysis steps, about 2 minutes.
The verdict is `guard-obligations`, with a public return value (mask 0):

| Obligation | Where | Meaning |
| --- | --- | --- |
| declassification | `schnorr_sign.coax:211` | key validity (`sw_valid_declassify`) |
| declassification | `schnorr_sign.coax:212` | the public-key bytes |
| declassification | `schnorr_sign.coax:217` | R |
| declassification | `schnorr_sign.coax:240` | the self-check's per-stage status |
| declassification | `schnorr_sign.coax:257` (×2) | the publish decision: validity and verdict |
| authored guard, origin public | `secp_field.coax:239`, `:492` | public assertions on the lifted public key's field values |
| public output | `nostr_sign_step`'s `out` | the published signature, whose s is derived from the key |

These are the points where the signer is designed to reveal a fact: whether
the key and nonce are valid, the public key, R, and whether the finished
signature verified. Every other step is admitted: scalar and field
arithmetic, both scalar multiplications, SHA-256 over secret data, and the
fixed-work self-verifier. There are no secret-dependent branches, indices or
traps, and no `_vartime` callee. The point parities stay secret.

The [report](data/signer-source-audit-2026-10-30/nostr-sign-step.json) and
its manifest are retained. `tools/coax-nostr/large-source-audit/test_signer_entry.py`
reruns the audit (about 2.5 minutes). It checks that every remaining
obligation is either a `_declassify` call in the signer or a public-origin
guard.

## How the profile got here

Records for each step:

- [contained casts](2026-10-30-secret-audit-contained-cast.md)
- [unsigned ranges](2026-10-30-secret-audit-ranges.md)
- [wrapping products](2026-10-30-secret-wrapping-products.md)
- [public fields](2026-10-30-secret-audit-public-fields.md)
- [declassification](2026-10-30-secret-audit-declassification.md)
- [loops, branches and run constants](2026-10-30-secret-audit-loops-arms.md)

Later pushes on the same day added:

- **Branch joins:** scalar bindings are joined after unresolved branches.
- **Struct public fields:** public fields may be structs, and loops leave
  their shadows to the fixed point.
- **Limits:** per-profile procedure, visit and region limits (large: 256,
  16,777,216 and 16,384; legacy and public-view unchanged at 64, 262,144 and
  4,096); a 512 KiB source budget; and up to 16 public fields.

**Independent review.** One review, at `51e4d98b3d`, found three false
accepts with a single cause (loop marks skipped for-each bodies) and an
over-broad declassification. Both were fixed, and the reproducers are
regression cases in `test_public_views.py`. A second review of the later
changes found two more false accepts and one weaker case:
1. declassifying part of a field, or one of several copies of it, cleared
   the whole field;
2. secrets written into public outputs (a public write view or a public
   field) were accepted with no report;
3. a declassification inside one branch also counted on the other path.

The fixes:
- a view resets a public field only if it covers the whole array field and
  the field occurs exactly once in a one-element root view;
- the taint a declassification can lower is joined separately for each path
  through unresolved branches, keeping only branches that fall through;
- every public writable root view and public field that ends holding secret
  data is reported as a `public-output` obligation. In the signer that is
  only `out`, which receives the signature.

The reproducers are regression cases in `test_public_views.py` and
`secret_field_cases.coax`.

**Signer source changes.** Each computes the same values. They were made
with the signer's owner (`openagents-d7`), who also changed the
self-verifier:

- explicit `_declassify` calls;
- an analyzable challenge (`sw_challenge_public`);
- a fixed-flow message digest;
- point parities passed as `odd & 1`;
- the SHA-256 workspace: block indices masked, schedule words loaded by
  index, and padding written in one loop;
- `snp_reduce` returning its public result;
- `mul_wrap` for products that provably fit, and 26-bit limb masks.

## Gates

**Large profile (`aggregate.coax`, after the second review's fixes):** 235 of
235 checks, and mutation in 20 shards killed **3,818 of 3,818**
([log](data/signer-source-audit-2026-10-30/large-mutation.log)). These are
ordinary mutation runs; a full reference-mode run is not done.

**Shared refactor:** checks and mutation (all 0 survived):

| Unit | Checks | Mutants killed |
| --- | --- | --- |
| legacy (`secret-check`) | 195 | [2,805/2,805](data/signer-source-audit-2026-10-30/secret-check-mutation.log) |
| `public-view-check` | 199 | [2,866/2,866](data/signer-source-audit-2026-10-30/public-view-mutation.log) |
| `inventory` | 44 | [840/840](data/signer-source-audit-2026-10-30/inventory-mutation.log) |

**Python suites:** `test_public_views.py` (12 of 12), `test_large_closure.py`,
`test_correspondence.py` (exact blocks against the pinned public-view
profile) and `test_signer_entry.py` pass.

**Signer and self-verifier gates:** run by the signer's owner (`openagents-d7`)
with `--run-steps 250000000` in short shards. All have 0 survivors and none
marked equivalent:

| Unit | Mutants killed | At |
| --- | --- | --- |
| `schnorr_sign` | 3,740/3,740 | `6c6dab1054` |
| `schnorr_self_verify` | 3,133/3,133 | `6c6dab1054` |
| `event_sign` | 5,550/5,550 | `6c6dab1054` |
| `schnorr_nonce_point` | 2,848/2,848 | `d017c8f902`, unchanged since |
| `sha256_workspace` | 1,091/1,091 | `d017c8f902`, unchanged since |

The memcheck probes of the compiled release signer pass, leaving only the
three declared decisions (`docs/issue4828.md`).

**Pre-existing failures:** 3 legacy and 1 public-view Python failures, the
same at `38b7f10b12` and on `main`. They are not caused by this work.

## Limits

- **Premises:** the source analysis rests on a trusted compiler, parser and
  bridge; stable sources; truthful view classification (live, aligned,
  disjoint, fixed extent); and the stated public fields.
- **Checked builds:** a secret checked `+`/`-` or narrowing cast is admitted
  when ranges prove its trap can't fire. The emitted helper still compares
  the secret, so its branch goes the same way for every value, but memcheck
  flags it. The target probes cover the release build only; checked builds
  rely on this source-level proof.
- **Not shown here:** target timing beyond the memcheck probes, compiler or
  host copies of secrets, secure erasure, iOS, and production approval.
