# Local backend watcher qualification, 2026-10-30

The new `openagents-backend-watch.service/.timer` is installed and enabled on the
existing `psionic-rtx6000-pylon` VM, `us-central1-b`. Its exact window remains
`1793394583` through `1793416183`. No healthy backend was restarted; no existing
model or Pylon unit file, VM, GPU configuration or IAM role was changed.

The first real oneshot completed successfully at **21:20:49 UTC**. All three
observations were valid and ready, with `restart:false` and failure count zero.
The before/after InvocationIDs of Elephant, Mammoth and GPT-OSS are identical
in the [installation receipt](data/oct30-backend-watch/install.json).
The timer was active and enabled and scheduled its next check two minutes later.

Its automatic, timer-owned followups at **21:22:49** and **21:24:50 UTC**
also observed all three healthy, with no restart and zero failures. The
[followup receipt](data/oct30-backend-watch/followup.json) includes distinct
watcher invocation IDs, the last/next trigger and unchanged backend invocation
identities. No manual source gate or model inference was repeated for that
schedule observation.

The Linux guardian binary SHA-256 is
`242fb74107170ba01fc6abbf2e01dd2776a0556420b700f01fd3395718605fc9`.
It is generated from the Coax authority and linked only with the small C
clock/file/argument face and libc/libm. The recorded
[source hashes](data/oct30-backend-watch/source.sha256) identify the policy,
platform effects and unit definitions. The original backend invocations remain
unchanged, including their CUDA allocations and Pylon identities.

Focused qualification:

- Checks were written before implementation; the stub policy failed six checks.
- `coax test psionic/lib/ops/backend-watch.coax`: **38 checks passed**, including
  its shared window/JSON dependencies, exact units/ports, boundary times,
  cooldown, zero timestamps, corrupt state and byte/codec limits.
- `coax mutate ... --root-only --jobs 12`: **314 mutants killed, zero survived**,
  with no exemptions. [Machine report](data/oct30-backend-watch/mutation.txt).
  The first run exposed missing boundary/codec checks and redundant guards;
  those were corrected rather than marking any mutant equivalent.
- `backend-watch-build.sh OUT sanitized` and
  `python3 psionic/tests/backend_watch.py`: **five native effect tests passed**
  against the real generated policy/ASan/UBSan face. Isolated fake systemd/HTTP
  effects verify three paced failures, exact targets, health/busy/warmup no-ops,
  recovered confirmation, process/window reset and oversized file rejection.
- Real Linux installation then compiled the same generated C and exercised
  only status and readiness reads. It did not inject a production fault or
  deliberately restart a model to repeat fixture coverage.

The qualified compiler is the existing `coax0 0.24.0` binary. Unchanged model,
protocol, web, compiler and managed-controller gates were not rerun. A further targeted path-with-spaces case passed after the shell quoting
cleanup; the five unchanged scenarios were not rerun.

The [runbook](../../psionic/docs/serving/backend-watch.md) describes maintenance,
window expiry, busy429 behavior and the limited recovery scope. Existing Pylons
retain their own reconnect/restart settings; this timer handles local backend
readiness only. Unknown endpoint schemas are indeterminate rather than restart
instructions, and no GPU-reset or VM-reboot path exists.
