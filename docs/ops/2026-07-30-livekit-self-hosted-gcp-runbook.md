# Self-hosted LiveKit on Google Cloud

Date: 2026-07-30
Issue: [EP263-LK-02 #9284](https://github.com/OpenAgentsInc/openagents/issues/9284)
Parent: [EP263-LK-00 #9282](https://github.com/OpenAgentsInc/openagents/issues/9282)

## Status and evidence boundary

This runbook is the operator contract for the disposable connectivity canary
and the production candidate. The checked-in automation and source-only
receipts prove only local validation. They do not prove that Google resources,
DNS, certificates, LiveKit, TURN, Redis, Sarah workers, or packaged Omega
sessions exist or work.

The owner granted the EP263 LiveKit program permission on 2026-07-30 to
provision all required infrastructure regardless of cost. That current owner
direction supersedes the standing profile's default dollar ceiling for this
program only. It does not remove redaction, rollback, exact-target, receipt,
secret, or Google-only controls. The operator must still record measured and
forecast cost, keep Google budget alerts active, and delete the canary when its
proof is complete.

Production remains Google Cloud only. Cloudflare publishes DNS-only records.
It does not proxy signaling, WebSocket, TCP, UDP, or TURN traffic.

## Fixed first-release shape

| Property             | Exact value                                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Google project       | `openagentsgemini`                                                                                                 |
| Region               | `us-central1`                                                                                                      |
| Zones                | `us-central1-a`, `us-central1-b`, `us-central1-c`                                                                  |
| Canary VM            | `oa-livekit-canary`                                                                                                |
| Production cluster   | `oa-livekit-prod`, regional GKE Standard                                                                           |
| SFU pool             | `oa-livekit-prod-sfu`, public nodes, one host-network SFU per node                                                 |
| Application pool     | `oa-livekit-prod-app`, Sarah workers and cluster services                                                          |
| Redis                | `oa-livekit-redis`, `STANDARD_HA`, private PSA, TLS, `auth_enabled=false`                                          |
| Namespace / release  | `livekit-system` / `livekit-server`                                                                                |
| Signaling address    | `oa-livekit-prod-signal`                                                                                           |
| TURN address         | `oa-livekit-prod-turn`                                                                                             |
| Signaling / TURN DNS | `livekit.openagents.com` / `turn.livekit.openagents.com`                                                           |
| LiveKit chart        | `1.11.0`, source commit `8f0ad0809c2be8cbed375a6f8bef10625e5e8a2b`                                                 |
| Server image         | `docker.io/livekit/livekit-server:v1.13.4@sha256:189f7c81b704a36642bc5c7e2d3e1ae83744627c11978a23a251bf19fbec64e0` |
| Server source        | `0b3fd288e3ef3263ec475ba0d78cf3ad77459981`                                                                         |
| Sarah workers        | three replicas on distinct `oa-livekit-prod-app` nodes; HPA range three through four                               |
| Worker health        | LiveKit Agents JS production endpoint on pod port `8081`                                                           |
| Alpha room cap       | 20 concurrent Sarah rooms                                                                                          |
| Private cap          | one owner-private generation                                                                                       |
| Community cap        | two Sarah rooms per community                                                                                      |
| Idle / lifetime cap  | 120 seconds idle / 1,800 seconds per room generation                                                               |

The room cap is an application refusal limit, not a capacity claim. It becomes
an admitted capacity only after the matching load observation passes with at
least 20% spare capacity. The production topology starts with three SFU nodes,
one per zone, and retains enough capacity for a zonal failure. An SFU node
failure terminates its in-flight rooms; it does not migrate their media state.

## Authority boundaries

LiveKit owns signaling, SFU media routing, ICE, direct WebRTC paths, embedded
TURN, room presence, and delivery-only room data. It does not own OpenAgents
identity, room membership, Sarah authority, commands, credit, usage, or
settlement.

The existing Cloud Run authority plane issues short-lived, room-scoped grants
after admission and hold reservation. It explicitly dispatches Sarah. A
LiveKit participant name, attribute, RPC, data message, transcript, or media
track is not identity or command authority.

The first release keeps raw media out of:

- Redis values;
- Cloud SQL and Cloud Storage;
- Khala Sync and Runtime Gateway projections;
- application, SFU, worker, load-balancer, and provider logs;
- metrics, traces, crash artifacts, and support bundles.

Provider and LiveKit transcript text is delivery/proposal data only. The
LiveKit server has no OpenAI credential. The Sarah worker has its own LiveKit
key and OpenAI secret but no OpenAgents database or Sarah Nostr signing key.
Cloud Run has its own LiveKit server key. Workload Identity supplies Google API
authority. No static service-account JSON is admitted.

## Immutable deployment bundle

`infra/livekit/bundle.json` is the machine-readable handoff between
infrastructure, Kubernetes, and operator automation. Before any live action:

```sh
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation validate-source \
  --bundle infra/livekit/bundle.json
```

The validator refuses a different project, region, zone set, cluster, node
pool, Redis instance, load-balancer address, namespace, release, Workload
Identity binding, secret-ref set, chart version/source commit, image version
or digest, configuration digest, manifest digest, or cap.

The bundle itself remains `source_only`. Its `pendingDependencies` remain
`packaged_omega_acceptance` and `sarah_worker_acceptance`; live observations
produce separate receipts and never rewrite source metadata into a proof
claim.

The default is always dry-run. Every command that reads or mutates live state
requires `--apply` and:

```sh
export OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST
```

That string is an operator interlock, not a credential or substitute for
current owner authority.

## Preconditions

1. Work from a clean `main` whose `HEAD` exactly equals remote
   `origin/main`. The bundle's `sourceBaseRevision` identifies the
   EP263-LK-01 contract base; it is not a deployment claim. Live ops record
   the actual 40-hex `HEAD` separately as `deployedRevision`.
2. Use the documented Google automation identity without exporting its
   credential material.
3. Install pinned-compatible `gcloud`, OpenTofu, `kubectl`, Helm 3, `yq`,
   OpenSSL, `dig`, and the LiveKit CLI. Helm 4 is rejected because the pinned
   render and addon path has not admitted it. Record versions by digest or
   exact version in private operator evidence.
4. Confirm the active project is exactly `openagentsgemini`. Every command
   still passes `--project openagentsgemini`; ambient CLI configuration is not
   authority.
5. Confirm regional GKE, public IPv4, forwarding-rule, CPU, and Memorystore
   quota with spare room for one SFU surge node.
6. Confirm the production infrastructure phase is the only owner of the exact
   Secret Manager containers
   `oa-livekit-prod-server-keys`, `oa-livekit-prod-redis-auth`,
   `oa-livekit-prod-cloudflare-dns`, `oa-livekit-prod-openai-api-key`, and
   `oa-livekit-prod-sarah-control-root`. Operators create versions only after
   that phase. The runtime materializes only the named Kubernetes secret refs.
   Public automation output may inspect container metadata and IAM, never
   secret payloads.
7. Confirm the two public addresses before changing DNS. Cloudflare records
   must remain DNS-only.
8. Confirm Google budget alerts and the application admission kill switch are
   active before cohort traffic.

Never pass secret values through command arguments, Terraform variables, Helm
values, ConfigMaps, checked-in manifests, or receipts.

### Redis provider limitation

`oa-livekit-redis` is Memorystore `STANDARD_HA` with in-transit TLS on the
dedicated private VPC through Private Services Access. Its exact first-release
configuration uses `auth_enabled=false`.

This is deliberate: the Google Terraform provider exposes the computed Redis
AUTH value in Terraform state when AUTH is enabled. That violates the
repository rule that runtime secrets stay out of state, even when the output
is marked sensitive. Network isolation, TLS certificate verification,
least-privilege clients, Redis monitoring, and room-generation failure rules
remain mandatory, but they are not described as Redis authentication.

The structured `oa-livekit-prod-redis-auth` Secret Manager payload therefore
contains exactly `{host,ca_cert}`. The retained resource name is compatibility
terminology; it does not contain a password. The Kubernetes
`livekit-redis-auth` projection likewise has no `password` key and the LiveKit
pod has no `REDIS_PASSWORD` environment variable. Do not claim Redis AUTH in a
receipt or re-enable it until a provider path keeps the value out of
Terraform state.

The production preflight derives region from the authoritative full live
resource name because `gcloud redis instances describe` does not reliably
emit a separate `region` field. Its JSON projection can also omit the
proto-default `authEnabled=false` value. That omission is admitted only for
`projects/openagentsgemini/locations/us-central1/instances/oa-livekit-redis`
when the active `google_redis_instance.livekit` Terraform source has one
literal `auth_enabled=false` assignment. Explicit `true`, short or
wrong-location names, ambiguous source, and host/CA drift all fail closed.

The production alert notification channel is present at the opaque operational
resource `projects/openagentsgemini/notificationChannels/1554456325732494481`
with display name `LiveKit Production On-call`. Its destination is private and
must not enter Git or terminal logs. Pass that channel through the documented
untracked production OpenTofu variable. The redacted prerequisite receipt is
[`receipts/livekit/monitoring-channel-prerequisite.json`](receipts/livekit/monitoring-channel-prerequisite.json).
That receipt proves channel presence only, not policy attachment or delivery.

The isolated rollout also has the exact required Google API set enabled in
`openagentsgemini`: Service Networking, Billing Budgets, Network Management,
IAM Credentials, and Security Token Service. The production Google provider
sets both `billing_project` and `user_project_override` to that same project so
quota-bearing requests, including the Billing Budgets API call, cannot fall
back to an unrelated OAuth credential consumer project. Preserve only the five
service refs and success state in public evidence; see
[`receipts/livekit/google-api-prerequisites.json`](receipts/livekit/google-api-prerequisites.json).
API enablement is a prerequisite, not a deployment or connectivity result.

Keep these production OpenTofu inputs untracked:

- `TF_VAR_master_authorized_networks`, a bounded JSON list with no
  `0.0.0.0/0`;
- `TF_VAR_notification_channel_ids`, exactly the redacted channel resource
  above;
- `TF_VAR_billing_account_id`.

The operator runner validates their shape and exact notification-channel ref
without printing their values.

## Stage A: disposable GCE canary

The canary exists to prove last-mile connectivity. It is not a production
fallback.

### Plan

```sh
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation canary-plan \
  --bundle infra/livekit/bundle.json
```

The plan must show only `oa-livekit-canary` resources in the isolated canary
state at `infra/livekit-staging` (remote-state prefix `livekit/staging`) and
must pass `enable_canary_instance=false`. The first phase creates the network,
reserved address, firewall, service account, IAM, and four empty Secret
Manager containers. It must not create the VM. The eventual VM has deletion
protection off and an expiry no later than six hours after creation. Its
container/runtime configuration is image- and digest-pinned. SSH is through
the admitted operator path; the VM does not have an open administrative port.

The canary permits only the required surface:

- trusted signaling and certificate traffic on `443`;
- WebRTC TCP fallback on `7881/TCP`;
- direct media on the pinned UDP range;
- TURN/TLS on the separate TURN name and address;
- IAP or the narrow operator path needed for lifecycle control.

Ingress, Egress, recording, SIP, telephony, public rooms, and retained media
are disabled.

### Phase 1: apply infrastructure with the VM disabled

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation canary-infra-apply \
  --bundle infra/livekit/bundle.json \
  --receipt docs/ops/receipts/livekit/canary-infra-<UTC>.json \
  --apply
```

Do not override `enable_canary_instance`. The runner applies it as `false`.
Confirm the reserved `oa-livekit-canary` address and the exact four containers:

- `oa-livekit-canary-api-key`;
- `oa-livekit-canary-api-secret`;
- `oa-livekit-canary-tls-certificate`;
- `oa-livekit-canary-tls-private-key`.

Add one enabled latest version to each container through a secret-safe
operator path. Create DNS-only A records for
`livekit-staging.openagents.com` and
`turn-livekit-staging.openagents.com`, both pointing only to the reserved
canary address. The certificate version must be currently trusted, remain
valid for the six-hour canary window, match the private key, and cover both
names as SANs. Do not put payloads in command arguments, shell history,
Terraform variables, or receipts.

### Phase 2: preflight, enable, and verify the TTL

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation canary-apply \
  --bundle infra/livekit/bundle.json \
  --receipt docs/ops/receipts/livekit/canary-deployment-<UTC>.json \
  --apply
```

Before OpenTofu can enable the VM, the runner fails closed unless all four
latest versions are enabled, no `oa-livekit-canary` VM exists, both A-record
answer sets contain only the reserved address, the certificate is current and
trusted for server use, both SANs validate, and the private key matches. It
uses the certificate and key only in a mode-`0600` temporary directory that is
deleted on exit and never prints either payload.

After apply, the runner refuses if the VM lacks the exact expiry
label/metadata, exceeds the six-hour maximum, enables deletion protection, or
resolves outside the canary state. The public receipt records an opaque
preflight ref and digests, not DNS answers, the public IP, certificate
contents, or secret values.

### Connectivity proof

Produce one private observation with the packaged Omega candidate. Force and
observe each path independently:

1. direct UDP;
2. WebRTC TCP fallback;
3. TURN/TLS.

Each path must prove room join, microphone publication, Sarah audio
subscription, selected ICE path, trusted certificate, advertised public
candidate, clean session end, provider accounting, and hold settlement.
Tests from inside Google Cloud do not prove last-mile connectivity. Use a
normal network, a VPN/restricted network, and a corporate-style TCP/TLS-only
network.

Validate and project the observation:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase connectivity \
  --bundle infra/livekit/bundle.json \
  --input <private-observation.json> \
  --receipt docs/ops/receipts/livekit/canary-connectivity-<UTC>.json \
  --apply
```

The input is private operator evidence. Only the redacted receipt path belongs
in Git.

### Mandatory destroy

Remove both staging DNS records from every authoritative Cloudflare name
server first. Then destroy the canary after equivalent production
connectivity passes or when the six-hour TTL expires:

```sh
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation canary-destroy \
  --bundle infra/livekit/bundle.json

OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation canary-destroy \
  --bundle infra/livekit/bundle.json \
  --receipt docs/ops/receipts/livekit/canary-destroy-<UTC>.json \
  --apply
```

The runner refuses to destroy while either staging hostname has any recursive
or authoritative `A`, `AAAA`, `CNAME`, `HTTPS`, or `SVCB` answer. The destroy
target is the exact isolated canary state. It verifies empty OpenTofu state
and zero canary VM, boot disk, address, two firewalls, VPC, subnet, runtime
service account, four secret containers, and DNS residue before emitting a
receipt. Production resources are outside this destroy graph.

## Stage B: production candidate

### Plan and source verification

```sh
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation production-plan \
  --bundle infra/livekit/bundle.json
```

The plan must preserve:

- regional GKE Standard, never Autopilot;
- public SFU nodes with `hostNetwork: true`;
- one LiveKit pod per SFU node, taint/toleration, required anti-affinity, and
  zone topology spread;
- a three-replica minimum and a PodDisruptionBudget retaining two ready SFUs;
- five-hour termination grace and explicit LiveKit drain;
- separate application nodes for Sarah workers;
- Memorystore Standard Tier with private connectivity;
- separate signaling and TURN addresses; the regional TURN address is
  `EXTERNAL` with Premium Network Tier and has no internal-address `purpose`;
- direct node UDP/TCP reachability and TURN/TLS on public `443`;
- internal-only Prometheus on `6789`;
- Secret Manager/Workload Identity references with no secret payload;
- disabled Ingress, Egress, recording, SIP, and TURN/UDP `443` unless its
  separate-address proof is admitted.

The production OpenTofu root is exactly `infra/livekit-production` with remote
state prefix `livekit/production`; it does not share the canary state.

### Phase 1: apply production infrastructure

Provide the bounded master-authorized networks, the exact redacted LiveKit
notification-channel ref, and the billing account through untracked
`TF_VAR_*` values. Then run:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation production-infra-apply \
  --bundle infra/livekit/bundle.json \
  --receipt docs/ops/receipts/livekit/production-infra-<UTC>.json \
  --apply
```

This phase creates and verifies the regional cluster, node pools, private
`STANDARD_HA` TLS Redis instance with AUTH disabled, reserved signaling/TURN
addresses, Workload Identity bindings, observability resources, and empty
Secret Manager containers. It does not install controllers or the LiveKit
runtime.

If an infrastructure apply stops after a subset of resources succeeds, do not
destroy the completed resources. Run `production-plan` from the corrected,
clean `origin/main`, require zero destroys and only the expected unfinished
resources, and then resume the same idempotent infrastructure apply. In
particular, a Google regional external address must not set
`SHARED_LOADBALANCER_VIP`; Google admits that purpose only on an internal
address.

After the infrastructure receipt passes:

1. Add `oa-livekit-prod-server-keys` as structured JSON with exactly
   `{api_key,api_secret,keys_yaml}`. `keys_yaml` is the key map consumed by the
   pinned runtime; the other two properties preserve explicit rotation
   authority.
2. Add `oa-livekit-prod-redis-auth` as structured JSON with exactly
   `{host,ca_cert}`, using the observed private Redis host and server CA. There
   is no password.
3. Add `oa-livekit-prod-cloudflare-dns` as structured JSON with exactly
   `{api_token}`.
4. Copy the current payload of the production Sarah secret
   `sarah-openai-api-key` used by Cloud Run revision `sarah-00003-jq8` into
   `oa-livekit-prod-openai-api-key`. Do not rotate Sarah by inventing a second
   value for this rollout.
5. Generate `oa-livekit-prod-sarah-control-root` with at least 384 bits of
   randomness encoded as 64-128 base64url characters. The OpenAgents API and
   Sarah worker receive that same root through their separate runtime secret
   mounts; it never enters Terraform, a command argument, or Git.
6. Make every target's `latest` version enabled. Copy values through a
   secret-safe pipe or operator UI; never through argv, a checked-in file,
   Terraform, or terminal output.
7. Publish DNS-only A records for `livekit.openagents.com` and
   `turn.livekit.openagents.com` to their separate exact reserved addresses.

### Phase 2: preflight, install addons, and apply the runtime

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation production-runtime-apply \
  --bundle infra/livekit/bundle.json \
  --receipt docs/ops/receipts/livekit/production-deployment-<UTC>.json \
  --apply
```

The runner first compares each DNS answer set to its exact reserved address,
requires all four latest target versions to be enabled, verifies the three
structured secret schemas, compares the Redis host and CA to the live
`STANDARD_HA` TLS/no-AUTH instance, and verifies the target OpenAI value is
byte-for-byte the existing production Sarah value. Payloads are held only in
process memory and never printed or retained in the public receipt.

Runtime apply requires Helm 3 and rejects Helm 4. It reads
`infra/livekit/addons.lock.json`, downloads the exact chart archives, verifies
their SHA-256 digests, renders every execution image by immutable OCI digest,
rejects tag-only output, and installs:

| Addon            | Version   | Archive SHA-256                                                    | Namespace          |
| ---------------- | --------- | ------------------------------------------------------------------ | ------------------ |
| cert-manager     | `v1.21.1` | `c27101f3f3e2349fb4a9e704316105bf7b52ad73b8c8257d3498ef7f2f6a4adc` | `cert-manager`     |
| External Secrets | `2.8.0`   | `251e4615013c6d2f9ade5cedf1cd8615613f286bfc381e44fb005f197e611ecd` | `external-secrets` |

CRD installation is explicit. Every addon component is selected onto
`oa-livekit-prod-app`; the lock includes the ACME solver and startup check
because they execute outside the long-running Deployments. The runner waits
for the required CRDs to become Established and for all controller deployments
to roll out before applying any `Certificate`, `ClusterIssuer`, `SecretStore`,
or `ExternalSecret`. Receipts retain only opaque chart and image refs,
versions, and digests.

The runner uses a temporary `KUBECONFIG`, validates the exact cluster, node
pools, Redis tier, addresses, Kubernetes namespace, Workload Identity
annotation, and named secret metadata, renders the pinned bundle, verifies
manifest digests, and inventories every rendered object before addon or runtime
mutation. Every namespaced object must carry an explicit namespace, and its
exact API version and kind must admit that namespace. The closed policy permits
only the expected `cert-manager` and `livekit-system` objects, permits only the
`livekit-system` Namespace object, and rejects cluster-scoped resources that
acquire a namespace. The final server-side apply therefore has no default
namespace flag and preserves each admitted object's explicit scope. It never
prints a Secret, a credential-bearing ConfigMap payload, an external IP, or a
provider response.

The worker image must be built from
`apps/sarah-livekit-agent/Dockerfile` with the repository root as its context
by `scripts/cloud/build-sarah-livekit-agent.sh --apply` and published to the
existing
`us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent`
Artifact Registry path. Replace the
source-only zero-digest placeholder with the observed digest in both the
worker manifest and `bundle.json`, set `workerImage.pinState=pinned`, refresh
the manifest and rendered-manifest digests, and run `infra/livekit/verify.sh`.
The runtime runner refuses a mutable tag, an image outside the admitted
repository, mismatched bundle/manifests, or the zero-digest placeholder.

The applied worker runs three replicas across distinct application-pool nodes
and zones. Its startup, readiness, and liveness probes use the Agents JS
production health endpoint on `8081`, which returns healthy only after the
worker WebSocket is connected to LiveKit. The PDB preserves two replicas,
the HPA scales from three through four, and the pod termination allowance is
longer than the worker drain and child-process shutdown bounds. The pod has no
Service and a deny-all ingress policy; its outbound LiveKit, OpenAI, and
OpenAgents control connections remain hostname-based and cannot be represented
honestly by a static Kubernetes CIDR allow-list.

Do not enable `livekit_room_v1` admission yet. First wait for SFU readiness,
Redis health, signaling and TURN load-balancer health, certificates, worker
registration, dashboards, alerts, and the emergency dispatch disable.

### DNS and certificate observation

The A records were provisioned between the two phases so certificate
controllers never start against stale DNS. After runtime apply, observe:

- `livekit.openagents.com` still resolves only to the exact signaling
  frontend and the Google-managed certificate is active;
- `turn.livekit.openagents.com` still resolves only to the exact TURN frontend
  and cert-manager's certificate is Ready and currently trusted.

Both records are DNS-only. Preserve a private change receipt with the prior and
new provider record identifiers; put only opaque refs and digests in the public
receipt. Never make signaling and TURN share an address/port collision.

## Acceptance

### Connectivity matrix

Repeat the canary matrix against production with the packaged Omega candidate.
All three paths must pass:

| Path         | Required observation                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| Direct UDP   | node external candidate selected; room/audio works; exact terminal accounting                                |
| TCP fallback | UDP blocked; WebRTC TCP selected; room/audio works; exact terminal accounting                                |
| TURN/TLS     | UDP and non-TLS TCP blocked; TURN/TLS selected on the TURN name; room/audio works; exact terminal accounting |

An endpoint responding to HTTPS is not media acceptance.

### Load

Drive at least 20 simultaneous two-participant rooms with one Sarah worker job
and one OpenAI Realtime generation per room. Keep private and community room
metrics separate and use opaque refs. The pass condition is:

- the 20-room hard cap is observed and a request above it refuses;
- at least 20% spare measured capacity remains;
- SFU and worker CPU stay at or below 80%;
- packet loss stays at or below 5%;
- p95 first audio is at or below five seconds;
- every session has one terminal provider/accounting outcome;
- no raw media or transcript enters a retained system.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase load \
  --bundle infra/livekit/bundle.json \
  --input <private-load-observation.json> \
  --receipt docs/ops/receipts/livekit/production-load-<UTC>.json \
  --apply
```

Do not infer many-small-room capacity from LiveKit's large-room benchmark.

### Failure drills

Run every drill from the exact pinned candidate:

1. SFU pod drain and replacement;
2. abrupt SFU node loss;
3. one-zone loss;
4. Redis failover and transient disconnect;
5. signaling backend removal;
6. certificate renewal;
7. TURN backend loss from a TURN-only client;
8. Sarah worker crash;
9. OpenAI/provider disconnect;
10. quota exhaustion during scale-up;
11. server/chart rollback.

Some drills must end active speech. A pass means visible bounded failure, no
overlapping provider generation, terminal usage and hold settlement, and fresh
admission before another billable generation. Never claim uninterrupted
speech unless it was actually observed and the architecture supports it.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase drills \
  --bundle infra/livekit/bundle.json \
  --input <private-drill-observation.json> \
  --receipt docs/ops/receipts/livekit/production-drills-<UTC>.json \
  --apply
```

### Secret, log, and retention scan

Seed a private forbidden-pattern corpus with synthetic markers. Scan:

- pod environment/file inventory without reading secret payloads into logs;
- SFU, worker, load-balancer, Redis, and provider-safe logs;
- Redis key metadata and bounded values through a purpose-built redaction
  checker;
- Cloud Storage object names and retention inventory;
- metrics and traces;
- crash and support artifacts.

The public result records only the number of patterns tested and zero/nonzero
findings. It never records a matched value.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase secret_scan \
  --bundle infra/livekit/bundle.json \
  --input <private-scan-observation.json> \
  --receipt docs/ops/receipts/livekit/production-secret-scan-<UTC>.json \
  --apply
```

## Observability and alerts

Before cohort admission, dashboards and alerts must cover:

- ready, draining, and unavailable SFUs;
- rooms, participants, worker jobs, provider generations, and room-start
  failures;
- CPU, memory, direct/TURN bandwidth, packet loss, jitter, retransmit, NACK,
  reconnect, and stream-start latency;
- direct UDP, TCP fallback, TURN/TLS selection, and TURN allocation failure;
- signaling/TURN frontend health and certificate expiry;
- Redis availability, failover, connection count, memory, eviction, latency,
  and rejection;
- Sarah worker load, event-loop delay, job count, OpenAI failures, first audio,
  interruption, tool round-trip, and exact usage projection;
- admission-to-room, mouth-to-first-audio, settlement, and teardown latency;
- room cap, provider budget, Google forecast/budget, quota, and emergency
  admission disable.

Labels use stage, transport, opaque room/owner/community refs, model,
generation, and cohort. They do not use email, Nostr keys, participant names,
transcript text, workspace paths, or raw media.

## Cost posture

The initial production topology has an estimated **approximately $1,500 per
month fixed floor** before meaningful conversation traffic. This is a planning
floor, not an invoice or credit-adjusted amount. It covers the regional GKE
control plane, three dedicated compute-optimized SFU nodes, application/worker
capacity, Memorystore Standard Tier, load balancers/addresses, observability,
and required spare capacity. Direct egress, TURN-relayed egress, autoscaling,
logs/metrics volume, NAT, certificates, and OpenAI Realtime usage are variable
and can materially increase it.

Google credit is not zero cost and cannot pay OpenAI. OpenAI credit cannot pay
Google. Reconcile daily:

- allocated SFU, worker, Redis, load-balancer, address, NAT, log, and metric
  cost;
- direct and TURN bytes;
- room idle/listening/speaking minutes;
- exact OpenAI response/transcription usage from the existing settlement
  plane;
- observed p95 cost per settled room-hour and useful turn.

The owner's cost approval permits required infrastructure, but budget alerts
remain mandatory. A forecast or quota alarm disables new admissions before it
causes an unbounded or partially admitted cohort.

The Terraform budget filter covers resources that preserve the
`service=livekit` label. Kubernetes-created load balancers and network egress
can be unlabelled, so that alert is a guardrail rather than complete cost
evidence. Daily billing-export reconciliation must include those unlabelled
charges before the cost gate can pass.

Project a measured cost observation only after billing export and the active
alerts have been inspected:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase cost \
  --bundle infra/livekit/bundle.json \
  --input <private-cost-observation.json> \
  --receipt docs/ops/receipts/livekit/production-cost-<UTC>.json \
  --apply
```

The validator refuses a fixed floor below $1,500, a room cap that differs from
the deployment bundle, inactive alerts, zero-cost treatment of Google credits,
or a cost record that collapses OpenAI into Google Cloud.

## Upgrade and drain

1. Disable scale-down on the SFU pool.
2. Add a pinned surge node and wait for readiness.
3. Mark one old SFU draining so it accepts no new rooms.
4. Wait for its room count to reach zero or the five-hour termination bound.
5. Apply one pinned image/chart/config change.
6. Repeat connectivity and a bounded load slice.
7. Continue one node at a time across zones.
8. Preserve the prior Helm revision, manifest digest, image digest, and config
   digest until full acceptance passes.

A PodDisruptionBudget does not migrate room media and is not a drain receipt.

## Scoped rollback

Rollback order is fixed:

1. disable issuance of new `livekit_room_v1` grants;
2. stop new Sarah dispatch;
3. drain existing rooms or present explicit failure;
4. close each OpenAI generation, record exact usage, and settle every hold;
5. restore the last pinned healthy LiveKit/worker revision;
6. repeat signaling, direct UDP, TCP fallback, TURN/TLS, Redis, worker, and
   accounting smoke checks;
7. verify unrelated OpenAgents Cloud Run, database, relay, update, and managed
   sandbox services are unchanged;
8. leave `custom_wss_v1` available only for a newly admitted generation.

Never move an active generation silently between transports.

Plan first:

```sh
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation production-rollback \
  --bundle infra/livekit/bundle.json \
  --current-manifest <current-rendered-manifest.yaml> \
  --previous-bundle <last-healthy-bundle.json> \
  --previous-manifest <last-healthy-rendered-manifest.yaml> \
  --previous-deployment-receipt <last-healthy-deployment-receipt.json> \
  --admission-receipt <private-admission-disable-receipt.json>
```

Then apply with the owner interlock, the exact previous bundle, and a new
public receipt path. The runner refuses unless the admission receipt proves
new LiveKit admission disabled and names the same environment/generation
boundary. It also requires the prior successful deployment receipt, exact
current and target inventories, and the same pinned cert-manager and External
Secrets addons:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-gcp-ops.mjs \
  --operation production-rollback \
  --bundle infra/livekit/bundle.json \
  --current-manifest <current-rendered-manifest.yaml> \
  --previous-bundle <last-healthy-bundle.json> \
  --previous-manifest <last-healthy-rendered-manifest.yaml> \
  --previous-deployment-receipt <last-healthy-deployment-receipt.json> \
  --admission-receipt <private-admission-disable-receipt.json> \
  --receipt docs/ops/receipts/livekit/production-runtime-rollback-<UTC>.json \
  --apply
```

Rollback server-side applies the target, deletes only the closed set of
current-only manifest resources, and then proves the exact target inventory,
Deployment convergence, ready pod count, and one target container image
digest. A changed addon pin is rejected instead of being rounded into a
runtime rollback claim.

Project the post-rollback observation:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase rollback \
  --bundle <last-healthy-bundle.json> \
  --input <private-rollback-observation.json> \
  --receipt docs/ops/receipts/livekit/production-rollback-<UTC>.json \
  --apply
```

Rollback does not close #9284 by itself. The close gate also needs production
connectivity, load, failure, secret-scan, cost, and canary-destroy receipts.

## Receipt handling

`scripts/cloud/livekit-acceptance.mjs` accepts a private observation and emits
one public receipt under `docs/ops/receipts/livekit/`. Its parser rejects:

- endpoints and IP addresses;
- secret, credential, token, private-key, prompt, transcript, and audio fields;
- unbounded or unknown fields;
- a source-only fixture that claims live proof;
- missing connectivity modes, load cap/spare capacity, failure drills, scan
  scope, settlement, or rollback constraints.

The source-only fixtures in this repository have `evidenceTier:
"source_only"`, `outcome: "planned"`, and `liveProof: false`. Never edit them
to say that live work passed. A live action creates a new timestamped receipt
from a matching private observation.

## Issue close gate

Do not close #9284 until all are linked from #9282:

- exact deployed `origin/main` revision, bundle source base, and digests;
- canary deploy, three-path packaged Omega connectivity, and zero-residue
  destroy;
- production deployment and DNS/certificate observations;
- production direct UDP, TCP fallback, and TURN/TLS connectivity;
- 20-room load with spare capacity and enforced caps;
- all eleven bounded failure drills;
- secret/log/raw-media/transcript scan;
- measured cost and active budget alerts;
- scoped rollback rehearsal;
- Sarah worker readiness from EP263-LK-03.

Until then, the honest status is “deployment automation and source contracts
ready; live production proof incomplete.”
