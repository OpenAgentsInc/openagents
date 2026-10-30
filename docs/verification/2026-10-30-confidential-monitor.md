# Bounded confidential Pylon health probes

The confidential lane is the existing immutable Clef workload on
`oct30-att-tdx-clef`, `openagentsgemini/us-central1-a`. It runs CPU inference
inside Google Confidential Space / Intel TDX. It remains separate from the
ordinary Coax `/api/v1/systemone` model gateway.

The optional historical oracle verifies the current publisher head and endpoint,
Google's certificate chain, the admitted workload measurement, and the
endpoint-key nonce before sealing a public request. It then verifies the sealed
response receipt. The new shell wrapper summarizes that existing verifier; it
does not introduce another signature, measurement, release or admission policy.
The fleet controller's recovery decisions remain in Coax.

## Run against a cached oracle

```sh
scripts/probe-confidential-pylon.sh \
  --oracle /tmp/oct30-confidential-oracle/target/debug/oa-att \
  --evidence-dir /tmp/confidential-proofs --timeout-seconds 120
```

The wrapper requires `sh`, `jq`, `timeout` (or macOS `gtimeout`) and ordinary
core utilities. It never builds the oracle. `OA_ATT_ORACLE_BIN` also selects the
cached executable; a packaged `oa-att` beside the wrapper is discovered
otherwise. With neither override, the historical helper's source-keyed cache
is used. The caller must choose the qualified oracle binary or component;
this is an operator tool, not an endpoint accepting user-supplied executables.

Every invocation uses a new disposable caller and a unique, public sunny-weather
sample. It passes no operator key and reads no operator signing credentials.
The request is intentionally public test data. A repeated fixed weather sentence
can hit the historical inference result cache; this wrapper changes the state
and requires positive reported inference time.

The deadline is at most 120 seconds, with at most two seconds of kill grace.
Cancellation terminates the owned oracle. There are no internal retries.
Top-level `refused`, nonzero exit, absent receipt, malformed fields and
inconsistent endpoint/model/receipt fields return nonzero and `ok:false`.
A score must be finite and between zero and one; this is a liveness check,
not a numerical-accuracy threshold or a replacement for the model's references.

Success prints one JSON object, including:

- `ok`, `checked_at` (Unix seconds), `provider`, `endpoint`, `pylon` and `identity`;
- verified `level`, `hwmodel`, `image`, `release`, `model_digest` and `receipt`;
- `request`, `request_ciphertext_digest`, `round_ms`, `inference_ms` and `score`;
- the public `proof` filename, backend, execution mode and vendor support fields.

Each invocation creates a private directory containing the public request,
complete successful/refused oracle report and sanitized summary. Oracle stderr
is discarded; no private caller key is persisted. Invalid partial output is
removed. The controller owns retention/upload, scheduling and recovery; a
successful dated observation is not a guarantee of future availability.

## Portable Linux component

```sh
scripts/prepare-confidential-oracle.sh /tmp/confidential-oracle-context
timeout 840 docker build --platform linux/amd64 \
  -t oa-confidential-oracle:monitor /tmp/confidential-oracle-context
```

Preparation requires a full-history checkout. It exports only the six reviewed
historical crates, their schema inputs and license into the new temporary
context, then adds the already-pinned reduced Cargo lock. No historical Rust
workspace is restored in the product repository. No legacy deployment or
publisher-key command is executed.

The builder is Rust 1.97.1 Alpine pinned at
`sha256:3c38f3f82c2f3d73da3b38e18d279393a04cb43ddded0e35088a8c3324d40900`.
The target is `x86_64-unknown-linux-musl`. The component is a `scratch` image
containing `/opt/oa-confidential/`: the static `oa-att`, wrapper, summary filter,
binary SHA-256, source/lock provenance and license. It has no model, Python,
Cargo, Rust toolchain, SDK, operator key or service credential at runtime.
The unchanged oracle is explicitly an optional verification tool, not a
substitute product runtime or production admission implementation.

A fleet-watch image with `sh`, `jq`, `timeout` and core utilities can copy that
directory from the component and invoke:

```sh
OA_ATT_ORACLE_BIN=/opt/oa-confidential/oa-att \
/opt/oa-confidential/probe-confidential-pylon.sh \
  --evidence-dir /tmp/confidential-proofs --timeout-seconds 120
```

Set that explicit oracle path in the watcher environment. The current source
also handles an absent HOME before its default-cache lookup; the published
component was qualified with HOME absent and the explicit oracle environment.

The image itself defaults to the bare oracle executable, so direct
`docker run --rm IMAGE verify --publisher HEX` remains available. Scheduling,
GCloud credentials and VM operations belong to the external controller.

## Existing deployment and recovery boundary

The worker's image remains
`sha256:896b90ac19e38c879782cbf39274c8d440925bfeb62a14f925faf204102f71d0`,
with admitted release
`28ac5b5295bd20af4b7649188dc70c6f05cfde3d85b4995148996cbdaf7a74ee`.
Its current endpoint key is
`eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0`.
The endpoint key is generated inside the TEE and rotates on workload restart;
the oracle therefore discovers it through the verified publisher/workload head.

The VM is STANDARD `c3-standard-8`, with `automaticRestart:true`, maintenance
policy `TERMINATE`, and Confidential Space container restart policy `Always`.
Those settings do not restart a deliberately stopped VM. The scoped fleet
controller must observe and recover that case and verify a new endpoint before
calling it healthy. This change adds no IAM grants, controller or VM mutation.

The immutable worker already publishes a current signed v1 beacon named
`att-tdx`, label `Sealed Clef (Intel TDX, Confidential Space)`, exact Clef
checkpoint and `meta.attested_endpoint`. The production homepage admitted it
and observed fresh renewals. A signed board row remains a provider claim;
the independent sealed probe supplies the vendor-verification evidence.

## Focused qualification

- `python3 scripts/tests/test_confidential_probe.py`: six checks of this wrapper,
  including timeout/cancellation and refusal/receipt failures.
- `python3 scripts/tests/test_confidential_oracle_context.py`: pinned source and
  lock closure, portable component permissions, and refusal to overwrite output.
- A fresh Mac shell-wrapper round passed: 8,506 ms native CPU inference,
  8,898 ms relay round, score `0.9681498647087364`, and a checked sealed receipt.
- The additional unset-HOME check was red before the one-line default-cache
  guard and green afterward; the earlier six unchanged paths were not rerun.
- Component build `68785d7f-b551-4ba2-99f5-2f6677b9b122` succeeded. Its immutable
  image is
  `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-confidential-oracle@sha256:e3b3f619c73fea64f30e0a2caea9e21d0898d00aa313f8151ed527e8a1319a3f`.
- One actual Linux static-component round passed in Cloud Build
  `1be2b122-e403-480a-bfe5-8470162c6286`, with the packaged wrapper, HOME absent
  and explicit oracle path. It reported 8,530 ms CPU inference, 8,714 ms relay
  round, score `0.967501235641538` and a checked sealed receipt. No component
  rebuild or oracle regression suite was run.
- The static binary SHA-256 is
  `33cd41b1021ef81e248e76ca52cc3e134319d9594f387bbc7005e0fe7dba31e2`.

Public reports: [Mac round](data/oct30-confidential-monitor/mac-round.json),
[Linux round](data/oct30-confidential-monitor/linux-round.json),
[Linux summary](data/oct30-confidential-monitor/linux-summary.json), and
[source/lock provenance](data/oct30-confidential-monitor/oracle-source.json).
The source and binary pins describe the optional historical oracle; the
current shell monitor/controller remains separately versioned.

The existing oracle's unchanged cryptographic suite, ordinary API, model kernels,
web release and unrelated compiler gates were not rerun for this tooling change.
