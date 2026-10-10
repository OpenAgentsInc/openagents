# NIP-ATT — Attested Workloads and Sealed Jobs

`draft` `optional` — v1, 2026-10-10. **Implemented for Google Confidential
Space on Intel TDX** (see [Implementation status](#implementation-status)):
`crates/nostr` (`att`), `crates/oa-att` (the client verifier, native and
WebAssembly), `crates/pylon` (`serve --attested`), and the live demo at
<https://openagents.com/att>.
The [shared contracts](contracts.md) are normative. The designs this profile
serves are the [sensitive data vault](../../docs/security/sensitive-data-vault.md)
and [private inference](../../docs/security/private-inference.md).

This NIP lets a client check, before it sends anything, that a remote program
is an exact, publicly logged build running inside a hardware trusted execution
environment (TEE), and then send work encrypted to a key that only that program
holds. It defines:

- a public **release** of a workload image, with its measurements and source;
- a public **release head** listing which releases are admitted, and from when;
- an **attested endpoint**, a running instance's key with the hardware evidence
  that binds the key to a measured release;
- a **sealed job** feature for CJ and DEC requests, sent to an attested key, with
  receipts that name the measurement that answered.

A signature here establishes who published a claim. Hardware evidence
establishes what ran, within the limits of its vendor's trust chain. Neither
grants permission, and an attested endpoint does not admit a job by itself.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [CJ](NIP-CJ.md) and [DEC](NIP-DEC.md) | Unchanged transport. A sealed job is an ordinary request whose `p` is an attested endpoint key and whose `requires` lists `openagents.attested.v1`. A worker without the feature refuses with `unsupported_feature`. |
| [PYLON](NIP-PYLON.md) | A beacon service MAY name an attested endpoint. Receipts for sealed jobs carry the measurement and use ciphertext digests (below). The beacon's class stays a self-claim; only evidence counts. |
| [EXT](NIP-EXT.md) and [EVAL](NIP-EVAL.md) | Adopting a new release is an explicit act with a notice delay, like EXT updates; running work keeps its pinned release. |
| [POL](NIP-POL.md) | Disclosure policy decides which data may go to which attestation level. This NIP supplies the level; it does not decide. |
| [SOV](NIP-SOV.md) custody | "An isolated runtime and remote attestation are different assurances with different evidence." This NIP is that evidence for one class of runtime. |
| [VAULT](NIP-VAULT.md) | The vault's key-release workload is an ATT workload; its sessions use the endpoint key defined here. |
| NIP-44, NIP-59 | Sealed job content is NIP-44 v2 to the attested key. A client MAY gift-wrap (NIP-59) a request to hide its own identity from relays. |

## Kinds

These are OpenAgents draft assignments, not upstream registrations.

| Kind | Class | Record |
| --- | --- | --- |
| `3202` | Regular | Attested workload release |
| `30202` | Addressable | Release head: the admitted releases of one workload |
| `30203` | Addressable | Attested endpoint: one instance's key and evidence |

## Attestation levels

Every endpoint has exactly one level, computed by the **client** from
evidence, never taken from a claim:

| Level | Evidence | Who can still read the plaintext |
| --- | --- | --- |
| `open` | None. | The provider operator and anyone with access to the machine. |
| `hardened` | Apple App Attest or a Secure Enclave-bound key chain, and code identity. | A provider with root or physical access (process memory is not hardware-encrypted). Not private. |
| `tee` | A verified TEE report (Intel TDX, AMD SEV-SNP) binding the key, with a measurement in an admitted release; GPU confidential-computing evidence when a GPU is used. Hardware held by the provider. | Physical attackers on that machine (memory-bus interposers have broken both TDX and SEV-SNP), side channels, and the vendors' roots of trust. |
| `tee-cloud` | As `tee`, and the evidence proves the machine is in a named cloud provider's data center (for example a Google Confidential Space token). | The cloud provider as a hardware root (its attestation service and physical security), side channels, and the vendors. |

`tee` and `tee-cloud` differ because physical-access attacks are cheap for
whoever holds the box. Data whose policy requires `tee-cloud` MUST NOT go to
`tee`.

## Attested workload release — kind `3202`

A release declares one exact image that may run as an attested workload.
It is signed by the workload's **publisher** key.

Tags: `d` absent (regular event); `t oa:att-release:v1`; `x` the SHA-256 of
the content bytes; one `w` tag with the workload slug.

Content (JCS JSON):

```json
{
  "v": "openagents.att-release.v1",
  "requires": [],
  "workload": "vault",
  "publisher": "<64-hex pubkey>",
  "image": {
    "reference": "us-docker.pkg.dev/openagents-vault/vault/vault-workload",
    "digest": "sha256:<64 hex>"
  },
  "platforms": [
    {
      "kind": "gcp-confidential-space",
      "hwmodel": "GCP_INTEL_TDX",
      "support": "STABLE"
    }
  ],
  "measurements": [],
  "gpu": null,
  "models": [],
  "components": [],
  "source": {
    "repo": "https://github.com/OpenAgentsInc/openagents",
    "commit": "<40 hex>",
    "recipe": "<path to the reproducible build recipe>",
    "recipe_digest": "sha256:<64 hex>"
  },
  "rebuilds": [
    { "builder": "<64-hex pubkey>", "digest": "sha256:<64 hex>" }
  ],
  "transparency": [
    { "log": "rekor", "url": "https://rekor.sigstore.dev", "index": 0 }
  ],
  "changes": "One paragraph for people: what changed and why.",
  "published_at": 0
}
```

- `platforms` lists where the image is admitted. Kinds:
  - `gcp-confidential-space`: the measurement is the container `image.digest`,
    as reported in the Confidential Space token, with `hwmodel` and `support`.
  - `tdx`: `measurements` lists `{register, value}` for `MRTD` and `RTMR0`–`RTMR2`.
  - `sev-snp`: `measurements` lists `{register: "MEASUREMENT", value}`.
  - `apple-app-attest`: the code identity (team, bundle, CodeDirectory hash).
    The release is then only good for `hardened`.
- `gpu` is null or `{vendor: "nvidia", mode: "cc", models: ["H100"]}`. If it
  is set, an endpoint must present GPU evidence.
- `models` lists the weights the image serves, as `{id, digest}`, where `digest` is
  the SHA-256 of the weight manifest. The image MUST refuse to load weights
  that do not match.
- `components` lists the programs inside the image as `{name, digest}`
  (SHA-256 of the binary). The image digest already covers them; the list
  lets a reader match a rebuilt binary, such as the inference engine,
  without unpacking the image. Absent means empty.
- `rebuilds` are independent builders' results. A client policy MAY require
  at least one independent rebuild with an equal digest.
- `changes` is shown to people when they are notified.

## Release head — kind `30202`

The publisher's current list of admitted releases for one workload.

Tags: `d` the workload slug; `t oa:att-head:v1`; one `e` tag for each listed
release.

```json
{
  "v": "openagents.att-head.v1",
  "requires": [],
  "workload": "vault",
  "generation": 7,
  "notice_seconds": 604800,
  "admitted": [
    { "release": "<event id of a 3202>", "effective_at": 0, "retire_at": null }
  ],
  "emergency": null
}
```

- A release is **admitted** for a client at time `t` when it is listed,
  `effective_at <= t`, `retire_at` is null or later than `t`, and
  `effective_at` is at least `notice_seconds` after the release's
  `published_at`. The exception is when `emergency` names that release with
  a reason, in which case the minimum is 86400 seconds.
- `generation` only grows. A client keeps the highest generation it has seen
  for each workload and refuses a lower one (rollback).
- A client MAY keep a **personal approval set**. With it on, a release is
  admitted only if it is also in that set. The set is private, stored as a
  self-addressed `3188` artifact or on the device.
- Clients SHOULD notify people when a new release appears for a workload
  they rely on, using `changes`.

## Attested endpoint — kind `30203`

One running instance of an admitted release, with its key.

Tags: `d` a random 64-hex instance ID; `t oa:att-endpoint:v1`; `a` the
release head (`30202:<publisher>:<slug>`); `e` the release; `expiration`.

The event is signed by the **endpoint key**, a secp256k1 key generated inside
the TEE when the instance starts, never exported. The same key receives NIP-44
sealed jobs.

```json
{
  "v": "openagents.att-endpoint.v1",
  "requires": [],
  "release": "<event id of a 3202>",
  "endpoint": "<64-hex pubkey, equal to the event pubkey>",
  "hpke": "<base64 X25519 public key, or null>",
  "binding": "<64 hex>",
  "evidence": [
    { "kind": "gcp-confidential-space-token", "format": "pki", "token": "<JWT>" },
    { "kind": "nvidia-gpu", "format": "nras-jwt", "token": "<JWT>" }
  ],
  "operator": "<64-hex pubkey of whoever runs the machine>",
  "issued_at": 0,
  "valid_until": 0
}
```

- **Binding.** `binding = SHA-256("openagents.att.v1\0" || endpoint (32 bytes)
  || hpke (32 bytes, or empty) || release event id (32 bytes))`. The binding
  MUST appear in the evidence as follows:
  - Confidential Space: as one of the token's `eat_nonce` values, requested
    by the workload through the launcher's token endpoint with an audience
    of `openagents.att.v1`.
  - Raw TDX or SEV-SNP: the first 32 bytes of the report's REPORTDATA.
  - App Attest: as the `clientDataHash` challenge.
  - GPU evidence: as the verifier nonce, when the GPU verifier accepts one.
    Otherwise, the GPU evidence MUST be gathered by the attested CPU
    workload at start, and the workload MUST refuse to start if the GPU is
    not in CC mode.
  - Confidential Space with a GPU: Google verifies the GPU's NVIDIA
    evidence itself and adds `submods.nvidia_gpu` to the same PKI token
    (`cc_mode`, `cc_feature`, and per GPU `hwmodel` such as
    `GCP_NVIDIA_H100`, `driver_version`, `vbios_version`, `ueid`). The
    token's `eat_nonce` binding then covers the CPU and the GPU at once,
    and no separate `nvidia-gpu` evidence entry is needed. A client
    checks `cc_mode` is `ON` and every listed GPU is one of the release's
    `gpu.models` (`H100` matches `GCP_NVIDIA_H100`); `DEVTOOLS` and `OFF`
    are refused.
- `valid_until` is at most 3600 seconds after `issued_at`, and no later than
  the evidence's own expiry. Endpoints republish before expiry with fresh
  evidence. The `d` tag stays the same while the instance lives.
- `operator` is informational. Evidence, not operator identity, sets the
  level.

### Client verification

A client MUST do all of the following before it encrypts anything to an
endpoint, and MUST refuse on any failure:

1. Verify the `30203` event signature, and that `endpoint` equals the event
   pubkey.
2. Fetch the named release and the current head. The release is admitted
   (above), and the head generation is not a rollback.
3. Verify each piece of evidence against its vendor root:
   - Confidential Space PKI tokens: against Google's attestation root, with
     issuer `https://confidentialcomputing.googleapis.com`.
   - TDX quotes: Intel's DCAP collateral.
   - SEV-SNP reports: AMD's ARK/ASK/VCEK chain.
   - NVIDIA: NVIDIA's attestation service or its published certificates.
4. Check the evidence against the release:
   - The image digest, or the registers, match.
   - `hwmodel` and `support` match. `STABLE` is required, so debug images
     always fail.
   - GPU mode and model match.
5. Recompute `binding` and find it in the evidence.
6. Compute the level. Compare it with the data's required level from the
   caller's POL disclosure policy.

Verification runs on the client. A server that verifies on the client's
behalf is trusted for the result, so a client that delegates verification
MUST treat the endpoint as no better than its trust in that verifier. This
matters on the web, where the verifier script is served by the same party.

## Sealed jobs — feature `openagents.attested.v1`

A CJ or DEC request with `openagents.attested.v1` in `requires`:

- Has its `p` tag equal to an attested endpoint key. Its content is NIP-44 v2
  to that key, as in CJ.
- Carries in its body
  `attested: {endpoint: "<30203 address>", release: "<id>", level: "tee-cloud"}`,
  the client's view at send time. The worker refuses if it is not that
  endpoint or not that release.
- Asks for streamed output sealed per chunk, as NIP-44 to the caller's key,
  inside CJ progress events. Chunk boundaries are visible; their content is
  not.
- MAY be gift-wrapped (NIP-59 `1059`, recipient `p` = the endpoint key). The
  relay then sees the endpoint and the size, but not the caller.
- MUST NOT carry caller-paid model keys. An attested worker runs its own
  pinned model and calls no outside provider, because a call to an outside
  model leaves the TEE. A release whose image can call out MUST declare
  `requires: ["openagents.att-egress.v1"]` and list the destinations in
  `meta`, and clients requiring `tee`/`tee-cloud` for private data MUST
  refuse it.

**Inside the worker.** It decrypts only inside the TEE, keeps no plaintext on
disk unless the disk is encrypted with a key that never leaves the TEE, and
zeroes request buffers when the job ends. It writes no prompt or output to
any log. The release's source is the evidence for these claims; the
attestation proves only that this source is what runs.

### Sealed answers

The worker's `26910` result for a sealed job is signed by the endpoint key
and, inside its NIP-44 content, its `response` carries
`attested: {endpoint, release, level, measurement, request_ciphertext_digest,
model, model_digest}`, where `request_ciphertext_digest` is the SHA-256 of
the request event's content. The execution receipt in the same result sets
`result_digest` to the SHA-256 of the response's JCS bytes, so the block is
covered by the receipt's seal, and `served.artifact_signature` to the
model digest. The client checks every one of these against its request
and the release before it shows the answer as sealed.

A NIP-PYLON `30200` beacon from an attested worker MAY carry
`meta: {attested_endpoint, claimed_level}`. It is inert: the level a
reader shows comes only from the endpoint's evidence.

### Receipts

A NIP-PYLON `3201` receipt for a sealed job:

- sets `request_digest` and `result_digest` to SHA-256 of the **ciphertext**
  event contents, never the plaintext, because a plaintext digest of a short
  or templated prompt can be guessed;
- adds `attested: {endpoint, release, level, measurement}`, where `measurement`
  is the image digest or the primary register value.

A gateway or broker that pays for a job sees only these receipts and the
event metadata. Billing uses `units` from the signed result.

## Validation

- Refuse unknown versions, features or fields, per [contracts](contracts.md).
- Refuse a `30203` whose `valid_until` has passed, is more than 3600 seconds
  after `issued_at`, or is later than any evidence expiry.
- Refuse a release head whose generation is lower than one already seen.
- Refuse evidence from a debug or experimental platform image.
- Refuse a sealed job to an endpoint whose level is below the data's
  required level. Never fall back silently to a lower level. The client
  shows the choice to the person instead.

## Privacy and disclosure

**What each party sees, at each level:**

| Party | `open` | `hardened` | `tee` / `tee-cloud` |
| --- | --- | --- | --- |
| Relay | Endpoint key, sizes, timing; caller unless gift-wrapped | Same | Same |
| Gateway or broker routing the job | Metadata and receipts | Same | Same |
| Provider operator | Plaintext | Plaintext, with root or physical access | Metadata only, except physical and side-channel attacks |
| Cloud provider | Plaintext, if hosted there | Same | Hardware-root trust only (`tee-cloud`) |

**Metadata that leaks at every level:** request and response sizes (NIP-44
pads to a power-of-two band), timing, which endpoint and release, and the
model, from the release. A client that needs to hide these must pad and
batch on its own.

**What does not hold:**

- Attestation does not prove that the model's answer is correct.
- Attestation does not prove that the source is free of a leak. Read the
  source; that is what the public log is for.
- Evidence is only as good as the vendor roots and the verifier code that
  checks it.

## Security considerations

- **Image control.** Whoever publishes releases decides what may run. The
  notice delay, independent rebuilds, Rekor entries and personal approval
  sets turn a malicious release into a public event before it is admitted.
  They do not prevent one.
- **Side channels and physical attacks.** TDX and SEV-SNP do not claim
  resistance to all microarchitectural or physical attacks. Published
  memory-bus interposer attacks extract secrets from both. This is the
  reason `tee` and `tee-cloud` are separate levels.
- **Freshness.** Evidence older than its expiry is refused. A client clock
  that is wrong can admit stale evidence, so clients SHOULD use the
  relay-observed time as a cross-check.
- **Key lifetime.** The endpoint key lives as long as the instance. There is
  no forward secrecy across jobs to one instance, beyond NIP-44's per-message
  keys. An attacker who extracts the instance key can read recorded jobs to
  that instance. Instances SHOULD rotate at least daily.

## Implementation status

Implemented (2026-10-10, #11241), for the `gcp-confidential-space` platform,
on Intel TDX alone and on Intel TDX with an NVIDIA H100 in
confidential-computing mode (GPU evidence as `submods.nvidia_gpu` in the
Confidential Space token, above):

- `crates/nostr` `att`: the three records, the binding, admission under the
  notice delay and emergencies, the rollback check, and the sealed-job
  fields and answer block.
- `crates/oa-att`: Confidential Space PKI tokens verified to Google's root
  (pinned by its bytes and SHA-256), the measurement and support level
  against the release, the binding in `eat_nonce`, the sealed request, and
  the answer and receipt checks. The same code runs in the gateway and, as
  WebAssembly, in the browser. The `oa-att` CLI publishes releases and
  heads and runs a verified round from a terminal.
- `crates/pylon` `serve --attested`: the worker inside the release image.
- `deploy/att/`: the release image (Psionic serving Clef on the CPU), and
  `scripts/deploy/att-provider.sh` for the build, the release and the VM.

Not yet: raw TDX quotes, SEV-SNP, NVIDIA evidence outside Confidential
Space (an NRAS token gathered by the workload), HPKE keys, Rekor
entries and independent rebuilds in the release, gift-wrapped requests,
per-chunk streamed output, the personal approval set, and the public `3201`
receipt for sealed jobs. The [private inference](../../docs/security/private-inference.md)
milestones P2–P4 track them.

## Conformance

Fixtures must cover:

- a valid release, head and endpoint for each platform kind;
- a debug-image token;
- a wrong digest;
- a binding that does not match;
- expired evidence;
- a head rollback;
- a release inside its notice delay, an emergency release, and a release
  missing from a personal approval set;
- a sealed job to an endpoint whose level is too low;
- a receipt carrying a plaintext digest, which must be refused as malformed
  for a sealed job.

Advertise `nip-att-v1` in NIP-11 `supported_extensions` only for a relay that
stores `3202`, `30202` and `30203`. Keep this draft name out of numeric
`supported_nips`.
