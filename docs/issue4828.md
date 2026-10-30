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
| `335fc24472` | (b) step 1: masked narrowing casts in the six kernels use `cast,trunc(u32)` (43 casts plus 5 carries). Mutation after the push: all six at 0 survivors: `secp_field32_total` 348/348, `secp_field32` 233/233, `secp_scalar` 385/385, `secp_group32` 1,125/1,125, `ecdh_fast` 1,342/1,342, `secp_mult_const` 1,442/1,442 (96 shards, each under 15 minutes). At -O2 this does not change the probe count (see the correction below) |
| (this push) | **Signer probe at -O2 with P, R and the validity bits declassified: passes.** Only the 3 declared decisions remain, and the control is flagged |

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
- **(b) stopped at step 1**, by agreement with `openagents-20`. The large audit profile already admits a secret checked add/sub or narrowing cast only when operand ranges prove the trap can't fire. So checked builds are covered at source level, and target probes cover release. Whether checked builds must also pass memcheck is an owner policy call, stated as a limit in the #4828 record. The original (b) plan follows:
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

## The signer's secret-flow probe (compiled code)

`coax/tests/nostr/schnorr_sign_secret_probe.sh` works like this:
- It builds the release generated C of `event_sign.coax`, which loads the
  signer, with GCC -O2, the shipped flags.
- Memcheck marks the secret key and aux as undefined.
- One whole signature runs: 6 calls, status 2, a valid signature.
- After each call the probe declassifies exactly what the signer publishes
  or decides on, as #4828's `_declassify` points do: the public key P, R,
  and the key, nonce and equation validity bits.

**Result: it passes.** 3 reports remain, one each in `sw_key` (key
validity), `sm_equation` (nonce and equation validity) and `sw_publish`
(publication). Those are declared decisions made inside a call, which the C
driver can't declassify mid-call. Apart from them, there is no key-dependent
branch or memory address anywhere in the compiled signer. The control, a
deliberate branch on a key byte, is flagged.

**Correction to the first version of this page:** it reported "217 reports,
201 of them range-checked narrowing casts". That run built with
`-O1 -fno-inline`. Without inlining GCC can't fold `coax_cast_uu`'s range
test, even for widening casts whose bound is `UINT64_MAX`. At -O2 the same
code gives 26 reports. Those 26 are all on P, which is lifted in the
self-check, and on the three decisions. Declassifying P leaves the 3 above.
The `cast,trunc` change (`335fc24472`) is still correct, and it removes trap
tests in non-inlined builds. But it did not change the -O2 count: 26 before
and after.

The self-verifier's probe (`a193b526cb`,
`coax/tests/nostr/schnorr_self_verify_secret_probe.sh`) gives this at -O2:
- **Release:** no candidate-dependent report.
- **Checked:** 126 reports, all checked-arithmetic and bounds traps.
- **Positive control:** flagged.

**Checked builds still branch on secrets** through their overflow and bounds
traps, so checked builds are not for secrets. Making checked arithmetic on
secret paths trap-free (`add_wrap`/`sub_wrap` where the bound is provable) is
the remaining part of (b).

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
