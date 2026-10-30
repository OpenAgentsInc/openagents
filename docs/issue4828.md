# #4828: the secret-source audit of the signer (status, 2026-10-30)

Board task: #4828 `crypto-secret-audit-large-closure` (milestone m-assurance),
claimed by `claude-secret-audit`. The work is split between two Claude
sessions, with independent Codex verification below. This page says what is done, what is in flight, who
holds which files, and how to check it.

## Why it matters now

#4828 is what stands between the Coax signer and **real keys**. Everything
else on the path to Psionic chapter 19 (a Pylon that answers decisions over
Nostr) is on main and passes end to end on test keys:

- `pylon_e2e.py` passes 52/52 over ws:// and 53/53 over wss:// at
  `fed0a442b2` and again at `753e949fc6`. Run it with
  `python3 -I psionic/tests/pylon/pylon_e2e.py [--wss] OUT_DIR`.
- The Pylon refuses `--key-file` until #4828 passes. After that:
  - the Psionic session (`openagents-1a`) creates the real Pylon key and the
    owner's `auth` signature (`NEEDS_OWNER.md`);
  - the signer (#51f4) can be closed;
  - the Nostr roadmap moves past test keys.

## What #4828 has to show

From the board task:

1. A separately versioned audit profile that handles the whole signer
   closure (27 physical source units, past the old 16-unit profile), with
   identities bound end to end. The old profiles and refusals stay unchanged.
2. Adversarial cases for every way secret data can leak: control flow,
   indices, aliases and guards. Unsupported behaviour fails closed.
3. Affected authority checks and mutation pass with no survivors, plus an
   independent review.

There is also a target-level half: what the *compiled* code does with
secrets. That's measured by the memcheck probes below.

## Done (on main)

| Commit | What |
| --- | --- |
| `38b7f10b12` | Large source-audit profile v3: unsigned ranges for secret checked operations |
| `7ca28e856e` | Removed the secret checked u64 multiplies on the ECDH path (`mul_wrap`); the audit reaches its guard obligations |
| `06af4ad5d5` | Public fields, void calls, the audit reaches into the signer |
| `7a65e17c3c` | Explicit, reported declassification (`_declassify` obligations) |
| `7640d0a571` | For-each loops, writes inside branches, run constants |
| `753e949fc6` | Signer fix from an independent review: the message is now bound across stages, so one nonce can't sign two messages |
| `a193b526cb` | Self-verifier memcheck probe: the release build has no candidate-dependent branch or address; the variable-time control is flagged |
| `10d8e604b8` | **Signer secret-flow map** (`coax/tests/nostr/schnorr_sign_secret_probe.sh`) |
| (b) step 1, this push | Masked narrowing casts in the six kernels use `cast,trunc(u32)` (43 casts plus 5 carries); the signer map drops from 217 to 189 reports. Fast checks pass. Mutation is owed |

## In flight

### `openagents-20` (the audit owner)

These are the source-audit side, in order:

1. Raise the profile's 64-procedure cap (owner-approved; `sw_nonce` needs more than 64).
2. An analyzable `_public` challenge instead of `sv_challenge_vartime` (owner-approved).
3. `_declassify` edits in `sw_key` and `sw_nonce` of `coax/lib/crypto/schnorr_sign.coax`, rebased onto `753e949fc6`. The signer's stages 3–5 stay untouched.
4. Pending patch to `coax/lib/crypto/sha256_workspace.coax` and `coax/lib/crypto/schnorr_nonce_point.coax`:
   - index masks `&63`;
   - the schedule loop split at 16;
   - `so_compress_clear`;
   - `snp_reduce` returning true.

Estimate: a few hours. It pushes once fast checks pass. Full gates follow the push (the owner's rule as of 2026-10-30: no hour-long checks before a push).

### `openagents-d7` (this session)

- **(c) done:** the signer's key-path probe, below.
- **(b) next:** make the compiled kernels branch-free on secrets.
  - First change: replace the range-checked narrowing casts of masked values with the non-trapping `cast,trunc(u32)` (`coax/spec.md`: "`cast,trunc(T) x` converts between integer types keeping the low bits").
  - Then replace checked add/sub with `add_wrap`/`sub_wrap` where the sum provably can't wrap.
  - Files: `secp_field32_total.coax`, `secp_field32.coax`, `secp_group32.coax`, `secp_scalar.coax`, `ecdh_fast.coax`, `secp_mult_const.coax`.
  - The two files in `openagents-20`'s patch are not touched. Their SHA-256 lines (`so_compress`, `sha256_update_checked`) are passed to that session to apply.
- **(a) after `openagents-20` pushes:** rerun the signer and `event_sign` mutation gates in small parallel shards and record them.

### Codex / `signer-independent-verification` (GPT-6, 2026-10-30)

I'm the Codex session previously working in `psionic-roadmap-review`. The
owner asked me to help without overlapping either session, and to coordinate
here. I own independent verification and evidence capture only:

- Compare the diagnostic `-O1 -fno-inline` build with the Pylon's release
  optimization (`-O2`) using the same generated release C and pinned inputs.
- Check exact fixture signatures and refusal paths, and include a deliberate
  leak control. Record compiler flags, source/artifact hashes and full reports.
- Put new tooling in `tools/coax-nostr/signer-verification/` and evidence in
  a separate dated verification record. The existing crypto sources, analyzer
  and `*secret_probe*` files remain with their current owners.

Baseline on `5e4f553e20` (before the kernel changes in `335fc24472`): GCC
15.2.0 / Valgrind 3.26.0 reproduce **217** diagnostic reports and **26**
reports at `-O2`. Both builds complete six stages with the exact BIP-340
vector-0 signature; both detect the deliberate leak control. These are
report contexts, not a count of proven leaks or production approval.
The `-O2` first-frame breakdown is 19 `sf_normalize`, 3 `smc_peer_bounded`,
and one each in `sw_key`, `sm_equation`, `ssv_prepare` and `sw_publish`.
Next I will qualify the comparison tooling and check newly landed changes.

**Coordination:** `openagents-20` and `openagents-d7`, please leave requests
or replies in this section, with the source commit and the check you want.
Please flag ownership changes here too. I will read this section before
each verification pass. I have an earlier uncommitted analyzer/SHA prototype
in the old worktree; it is paused and will not be pushed over your work.
Production-key approval and your mutation gates remain with their owners.

## The signer's secret-flow map (`10d8e604b8`)

`coax/tests/nostr/schnorr_sign_secret_probe.sh` works like this:
- It builds the release generated C of `event_sign.coax`, which loads the signer.
- Memcheck marks the secret key and aux as undefined.
- One whole signature runs: 6 calls, status 2, a valid signature.
- Only the signature and the step status are declassified.

It reports every branch or address that depends on the key. It is a map, not a gate: it always exits 0.

The result: **217 reports.**

- **201 come from range-checked narrowing casts.** `coax_cast_uu` traps with `if (v > hi)` even in release, on values like `cast(u32)(wide & SF32_WORD)`. Those values are masked to 32 bits, so the trap can't fire, but the test is still a branch on secret data. Callers:

  | Function | Reports |
  | --- | --- |
  | `sf32_fold_total` | 58 |
  | `sf32_mul_total` | 57 |
  | `ef_field_mul` | 37 |
  | `ef_square_n` | 36 |
  | `sg32_double` | 34 |
  | `sf32_canonical_total` | 17 |
  | `sf32_add_total` | 13 |
  | `sc_mul`, `sc_montgomery` | 11 each |
  | `so_compress` | 10 |
  | `sf32_sub` | 7 |
  | `sg32_from_point`, `sg32_from_field` | 6 each |
  | `sc_sub`, `sc_negate`, `sc_subtract_n` | 5–6 each |
  | `smc_multiply` | 5 |
  | `sha256_update_checked` | 5 |

  These are (b)'s first target.
- **16 more** are spread across a few functions:

  | Function | Reports |
  | --- | --- |
  | `sw_publish` | 4 |
  | `smc_peer_bounded` | 3 |
  | `sf_normalize` | 3 |
  | `coax_index` | 3 |
  | `sw_key` | 1 |
  | `ssv_prepare` | 1 |
  | `sm_equation` | 1 |

  These are mostly where the public key P and the nonce point R become public. They are `openagents-20`'s `_declassify` points.

Masked narrowing casts per file (counted by `grep 'cast(u32)(...&...WORD)'`):

| File | Masked | All `cast(u32)` |
| --- | --- | --- |
| `secp_field32_total.coax` | 12 | 20 |
| `secp_field32.coax` | 8 | 13 |
| `secp_scalar.coax` | 9 | 16 |
| `ecdh_fast.coax` | 11 | 17 |
| `secp_group32.coax` | 2 | 3 |
| `secp_mult_const.coax` | 1 | 2 |

Other casts are of constants, of `carry` (a value of 0 or 1, or bounded by a shift) and of shifted values. They are checked one by one.

The self-verifier's probe (`a193b526cb`, `coax/tests/nostr/schnorr_self_verify_secret_probe.sh`):
- **Release:** no candidate-dependent report.
- **Checked:** 126 reports, all checked-arithmetic and bounds traps.
- **Positive control:** flagged 51 times in release and 104 in checked.

The signer map above is the key-side equivalent, and it shows that the release kernels still trap-check casts.

## Who holds which files

| Session | Files |
| --- | --- |
| `openagents-20` | `coax/lib/crypto/schnorr_sign.coax` (`sw_key`, `sw_nonce`, `sw_scalar`), `sha256_workspace.coax`, `schnorr_nonce_point.coax`, `tools/coax-nostr/secret-check/` |
| `openagents-d7` | The kernels listed under (b); `schnorr_sign.coax` stages 3–5 (`sw_check`, `sw_publish`); `schnorr_self_verify.coax`; the probes in `coax/tests/nostr/*secret_probe*` |
| Codex / `signer-independent-verification` | New `tools/coax-nostr/signer-verification/` tooling and a separate verification record; coordination replies in the Codex section above; no existing signer, kernel, analyzer or probe edits |

Ask before editing the other session's files. Cross-session messages go
through `SendMessage` (names `openagents-20`, `openagents-d7`); the owner
relays when needed.

## How to check

```sh
# Signer key-path map: report count, then the reports per function
coax/tests/nostr/schnorr_sign_secret_probe.sh
# Self-verifier probe: release must be clean, the control flagged
coax/tests/nostr/schnorr_self_verify_secret_probe.sh
# The signer end to end: native, Wasm, 10,000 round trips
python3 coax/tests/nostr/schnorr_sign.py --round-trips 10000
# Unit checks (fast)
coax/bin/coax test coax/lib/crypto/schnorr_sign.coax --run-steps 250000000
coax/bin/coax test coax/lib/nostr/event_sign.coax --run-steps 250000000
# Chapter 19 end to end
python3 -I psionic/tests/pylon/pylon_e2e.py build/pylon-e2e
```

On this Linux host `cc` is not a C compiler. Use gcc: `CC=gcc`, or a
`cc -> gcc` symlink first on PATH. Wasm needs `~/.cache/coax-wasm-toolchain/bin`
on PATH. Valgrind comes from nixpkgs through the probe scripts.

## Owed after the push

These run in short shards, each well under 15 minutes, not as one long run:

- Mutation gates for every file changed by (b) and by `openagents-20`'s
  signer edits. Recorded runs, by mutant count:
  - signer: 3,754 mutants, recorded at `753e949fc6`;
  - `event_sign`: 5,564 mutants, recorded at `753e949fc6`;
  - self-verifier: 2,865 mutants, recorded at `c9eed2394b`.
- The probes again, with the goal of zero release-build key-dependent
  reports outside the declared `_declassify` points.
- `schnorr_sign.py` (native, Wasm, iOS Simulator on the Mac) and `pylon_e2e.py`.

## Not in scope here

- **arm64 address trace:** the owner chose timing evidence only. The arm64
  timing result is in `docs/verification/2026-10-29-schnorr-self-verify.md`.
- **Wasm JIT probes:** no tool for them.
- **Production-secret approval:** a separate owner gate.
