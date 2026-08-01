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
`TF_VAR_*` values. Before the LiveKit root activates the explicit-build-
identity organization policies, create the replacement generic builders from
the existing production baseline and review the plan for no unrelated change:

```sh
tofu -chdir=infra/prod init -input=false
tofu -chdir=infra/prod plan \
  -input=false -lock-timeout=60s \
  -out=/tmp/openagents-prod-build-identities.tfplan
tofu -chdir=infra/prod apply \
  -input=false -lock-timeout=60s \
  /tmp/openagents-prod-build-identities.tfplan
```

This ordering is mandatory: the next apply requires explicit build identities
project-wide, so the generic Cloud Run and image builders must already exist.
Then run:

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

### Phase 2: bootstrap addons, then apply the runtime through the fixed trigger

Production runtime mutation is admitted only through the server-side
`oa-livekit-prod-runtime` Cloud Build trigger. Do not submit
`livekit-gcp-ops.mjs` as an arbitrary local Cloud Build config and do not run
its production runtime operation directly. The trigger is Terraform-managed,
uses the dedicated `oa-livekit-prod-deployer` service account, fixes its source
at `refs/heads/main` through the regional Cloud Build v2 connection
`oa-livekit-github` and repository `openagents`, accepts no
caller substitutions, and runs an executor image that must be pinned by digest.
The trigger's source, service account, inline
build, receipt bucket, 90-minute worst-case timeout, and Connect Gateway
membership are part of the reviewed execution boundary.

Bootstrap the boundary in this order:

1. Apply `infra/prod` first. It creates two explicit, non-runtime build
   identities: `oa-cloud-run-source-builder` for Cloud Run source deploys and
   `oa-cloud-image-builder` for direct container builds. The latter can write
   only to `oa-cloud`, write build logs, and read the dedicated seven-day
   `openagentsgemini-cloud-build-source` staging bucket. The automation
   identity can act as these narrow builders; it receives no ability to act as
   the LiveKit runtime deployer. Every repository Cloud Run source-deploy and
   direct Cloud Build path supplies its corresponding identity explicitly.
2. Apply the LiveKit production root once with deployment control disabled. This
   creates `oa-livekit-image-builder`, whose only project grant is log-write
   and whose only data mutation grant is writer on the `oa-cloud` Artifact
   Registry repository. It can read only the dedicated
   `openagentsgemini-livekit-build-source` staging bucket, whose objects expire
   after seven days. Both Sarah and deployer image scripts select that bucket,
   pass the exact service account to Cloud Build, and force Cloud Logging;
   neither can fall back to the default Compute service account or default
   Cloud Build storage.
   The root also sets both `cloudbuild.useBuildServiceAccount` and
   `cloudbuild.useComputeServiceAccount` to not enforced. In combination,
   these Google Cloud constraints require each build to specify a service
   account and prevent fallback to the legacy Cloud Build or default Compute
   identities.
3. From a clean worktree whose `HEAD` exactly equals current `origin/main`
   (a detached HEAD or temporary worktree branch is valid), run
   `scripts/cloud/build-livekit-production-deployer.sh --apply`, resolve the
   resulting digest, and set `TF_VAR_deployment_executor_image` to the printed
   value. The Dockerfile, Cloud SDK, Node, Helm, kubectl, and Docker builder are
   all source-pinned, while the deployed trigger accepts only the final
   immutable executor digest.
4. Set the printed executor image and enable deployment control by rerunning
   the gated infrastructure apply. Before the apply, install the Google Cloud
   Build GitHub App on `OpenAgentsInc/openagents`, authorize the regional v2
   connection with an OAuth token tied to that app, and place the token in a
   dedicated Secret Manager secret without writing it to argv, Terraform, Git,
   or terminal output. Record the positive GitHub App installation ID and one
   immutable numbered secret version; `latest` is refused. Terraform grants
   only the Cloud Build service agent access to that secret, creates the
   deletion-protected connection and repository, and binds the manual trigger
   to the repository resource rather than a raw GitHub URI.

   ```sh
   export TF_VAR_deployment_executor_image='us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/livekit-production-deployer@sha256:<digest>'
   export TF_VAR_deployment_source_github_app_installation_id='<positive-installation-id>'
   export TF_VAR_deployment_source_github_authorizer_secret_version='projects/openagentsgemini/secrets/<cloud-build-github-authorizer-secret>/versions/<number>'
   export TF_VAR_enable_deployment_control=true
   OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
   node scripts/cloud/livekit-gcp-ops.mjs \
     --operation production-infra-apply \
     --bundle infra/livekit/bundle.json \
     --receipt docs/ops/receipts/livekit/production-deployment-control-<UTC>.json \
     --apply
   ```

   This creates the dedicated deployer, v2 source connection and repository,
   fixed trigger, fleet membership, and retention-locked receipt bucket. The
   `livekit-privileged-identity=protected` resource tag to the default Compute,
   LiveKit node, runtime, secret-reader, and production deployer service
   accounts. A conditioned project IAM deny blocks only
   `oa-mvp-automation`'s `iam.serviceAccounts.actAs` requests whose target has
   that tag. It does not deny `cloudbuild.builds.create` or affect untagged
   service accounts, so the automation identity retains build submission
   through the explicit narrow builders. Service-account resource tags are a
   Google Cloud Preview feature; do not apply the deny if the tag-binding API
   is unavailable, and do not admit production until the fail-closed
   impersonation preflight passes for every protected identity.

5. As a one-time cluster administrator on clean, current `main`, install and
   verify the pinned controllers, recording a separate receipt:

   ```sh
   OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
   node scripts/cloud/livekit-gcp-ops.mjs \
     --operation production-addon-bootstrap \
     --bundle infra/livekit/bundle.json \
     --receipt docs/ops/receipts/livekit/production-addon-bootstrap-<UTC>.json \
     --apply
   ```

6. Still using the one-time cluster administrator, apply
   `infra/livekit-production/deployer-gateway-rbac.yaml`. It grants the
   deployer Connect Gateway impersonation, mutation only in `livekit-system`
   and the admitted `cert-manager` resources, read-only access to controller
   Deployments and four CRDs, and mutation of only the named cluster-scoped
   LiveKit resources. It never grants cluster-role, webhook, or CRD mutation,
   and does not grant `cluster-admin`.

Cloud Build uses the same `cloudbuild.builds.create` permission for a fixed
trigger and an arbitrary build, so a project-wide denial would also disable
the project's other build workflows. Instead, the two effective organization
policies must conclusively require a user-specified build identity. A
pre-existing project owner starts this manual trigger. Before every runtime
mutation, the runner verifies those effective policies and Policy
Troubleshooter must return conclusively `NOT_GRANTED` for the legacy
automation identity's ability to act as both the default Compute and
every LiveKit node, runtime, secret-reader, and production deployer identity.
Unknown, conditional, or inherited policy results fail closed.

Start the fixed trigger from clean, current `main`:

```sh
node scripts/cloud/livekit-production-deploy.mjs start
```

The launcher prints the canonical build ID. Its local wait is bounded but does
not cancel a still-running build. Resume observation without starting a second
deployment:

```sh
node scripts/cloud/livekit-production-deploy.mjs status \
  --build-id <canonical-build-UUID> \
  --timeout-seconds 3600
```

After `SUCCESS`, retrieve the source-only receipt for review and commit:

```sh
node scripts/cloud/livekit-production-deploy.mjs retrieve \
  --build-id <canonical-build-UUID> \
  --receipt docs/ops/receipts/livekit/production-deployment-<UTC>.json
```

Before Kubernetes mutation, the build preflights an exclusive local receipt
path, writes and reads a no-clobber build-scoped object in the retention-locked
receipt bucket, and attests the live build, trigger, v2 repository resource,
exact GitHub remote, fixed `refs/heads/main` source ref, and resolved commit.
The launcher supplies the exact commit observed at remote `main` with `--sha`;
the build and receipt must resolve to that same commit. The tiny preflight
object remains under the same 30-day retention policy. The receipt binds the canonical build
ref, trigger ref, dedicated service-account ref, resolved source revision, and
reviewed execution-boundary digest. Retrieval refuses a non-successful build,
an existing target path, or mismatched provenance. This gives start, bounded
wait, resume, and retrieve the same build-ID contract even when an operator
terminal or network disappears.

The runner first compares each DNS answer set to its exact reserved address,
requires all four latest target versions to be enabled, verifies the three
structured secret schemas, compares the Redis host and CA to the live
`STANDARD_HA` TLS/no-AUTH instance, and verifies the target OpenAI value is
byte-for-byte the existing production Sarah value. Payloads are held only in
process memory and never printed or retained in the public receipt.

The one-time addon bootstrap requires Helm 3 and rejects Helm 4. It reads
`infra/livekit/addons.lock.json`, downloads the exact chart archives, verifies
their SHA-256 digests, renders every execution image by immutable OCI digest,
rejects tag-only output, and installs:

| Addon            | Version   | Archive SHA-256                                                    | Namespace          |
| ---------------- | --------- | ------------------------------------------------------------------ | ------------------ |
| cert-manager     | `v1.21.1` | `c27101f3f3e2349fb4a9e704316105bf7b52ad73b8c8257d3498ef7f2f6a4adc` | `cert-manager`     |
| External Secrets | `2.8.0`   | `251e4615013c6d2f9ade5cedf1cd8615613f286bfc381e44fb005f197e611ecd` | `external-secrets` |

CRD installation is explicit. Every addon component is selected onto
`oa-livekit-prod-app`; the lock includes the ACME solver and startup check
because they execute outside the long-running Deployments. The bootstrap waits
for the required CRDs to become Established and for all controller deployments
to roll out before applying any `Certificate`, `ClusterIssuer`, `SecretStore`,
or `ExternalSecret`. Receipts retain only opaque chart and image refs,
versions, and digests. Recurring runtime deployments do not receive the
cluster-wide Helm permissions needed to repeat this bootstrap. Instead, they
fail before mutation unless all six long-running controller Deployments use
the locked image digests, the ACME solver argument is locked, and all four
required CRDs are established.

The runner uses a temporary `KUBECONFIG`, validates the exact cluster, node
pools, Redis tier, addresses, Kubernetes namespace, Workload Identity
annotation, and named secret metadata, renders the pinned bundle, verifies
manifest digests, and inventories every rendered object before addon or runtime
mutation. Every namespaced object must carry an explicit namespace, and its
exact API version and kind must admit that namespace. The closed policy permits
only the expected `cert-manager` and `livekit-system` objects, permits only the
`livekit-system` Namespace object, and rejects cluster-scoped resources that
acquire a namespace. The final server-side apply therefore has no default
namespace flag and preserves each admitted object's explicit scope. It uses
the fixed `openagents-livekit-ops` field manager with conflict takeover so the
reviewed manifest reclaims its closed resource inventory after an explicitly
recorded break-glass `kubectl set` operation; no competing manager can leave a
runtime image or configuration field outside the pinned bundle. It never
prints a Secret, a credential-bearing ConfigMap payload, an external IP, or a
provider response.

The worker image must be built from a clean worktree whose `HEAD` exactly
equals current `origin/main`; the branch name is intentionally irrelevant so
an isolated or detached worktree can preserve unrelated user changes. Build
the worker from
`apps/sarah-livekit-agent/Dockerfile` with the repository root as its context
by `scripts/cloud/build-sarah-livekit-agent.sh --apply` and published to the
existing
`us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent`
Artifact Registry path. The script always supplies
`oa-livekit-image-builder`; it cannot fall back to the default Compute service
account, and its Docker build step is pinned by digest. Replace the
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

The runtime is fail-closed on this key: an absent or malformed
`SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED` is disabled. Since 2026-07-31 the
committed production value is the alpha's intended **steady state**, and a
fail-closed first bring-up is expressed by the deploy rather than by a stale
committed value. This makes a fresh cluster rollout two explicit phases:

The monolith deploy script also passes the full
`oa-cloud-run-source-builder` resource through `--build-service-account`.
`infra/prod` grants that identity only the documented Cloud Run Builder role.
The repository's other source deploy scripts use the same explicit builder;
direct image builds use `oa-cloud-image-builder` or the even narrower LiveKit
image builder. Apply `infra/prod` before enabling the explicit-identity
policies in the LiveKit production root. `oa-mvp-automation` may submit builds
and act as these two reviewed build-only identities, but cannot act as the
default Compute or production LiveKit deployer identities. Build and Cloud Run
runtime identities remain different; neither builder receives Secret Manager,
Cloud Run deploy, GKE, or runtime service-account impersonation grants from
this change. Run the following Cloud Run deploys as a current owner account,
not with the `oa-mvp-automation` gcloud configuration. Once the protected tag
bindings are active, the deny policy intentionally refuses the automation
identity's attempt to act as the default Compute runtime identity; the explicit
`oa-cloud-run-source-builder` remains the narrowly scoped image build identity.

1. deploy the API revision with admission explicitly disabled, then wait for
   SFU readiness, Redis health, signaling and TURN load-balancer health,
   certificates, all three Sarah worker replicas Ready and registered,
   dashboards, alerts, and the emergency dispatch disable:

   ```bash
   SARAH_LIVEKIT_ADMISSIONS=off \
     scripts/deploy-cloudrun.sh production
   ```

2. arm only after preserving those observations, by deploying the same
   candidate with the committed steady state:

   ```bash
   scripts/deploy-cloudrun.sh production
   ```

`SARAH_LIVEKIT_ADMISSIONS` accepts exactly `on` or `off`. It rewrites a
temporary rendered copy of the committed environment before
`gcloud run deploy`, so the deployed revision is the revision the script smokes
and reports. Do not patch these keys with a follow-up
`gcloud run services update`: that costs a second revision on every deployment
and is exactly how production came to serve `true` while `main` committed
`false` (EP263-LK H2/H3, #9282).

Confirm the revision is Ready and serving 100 percent of traffic before issuing
a private-room acceptance request. A routine subsequent deployment now
preserves the committed steady state in one revision instead of closing
admissions and requiring a re-arm. The immediate emergency stop is a deploy
with `SARAH_LIVEKIT_ADMISSIONS=off`; it stops new rooms without removing worker
close, usage, or cleanup routes.

**After any out-of-band change to these keys, reconcile before you walk away.**
The committed environment and the serving revision must agree, or `main`
describes a production state that does not exist:

```bash
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
  node scripts/cloudrun/check-livekit-admission-drift.mjs
```

It exits non-zero on divergence and prints only booleans and revision names.
If it reports drift, either commit the intended state and redeploy, or withdraw
the override — never leave the two disagreeing.

The 0.2.0 gate admits only `clientProfile=omega_editor` to
`livekit_room_v1`. Mobile profiles are rejected before reservation or
dispatch; mobile remains deferred rather than inheriting editor tools from a
private-room mapping.

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

### Executable evidence collection

Do not assemble production acceptance observations by hand. Use
`scripts/cloud/livekit-production-acceptance.mjs` to execute the probe commands
and project their results through the same policy validators used by
`livekit-acceptance.mjs`.

The private plan has a closed schema:

```json
{
  "schemaVersion": "openagents.livekit_production_acceptance_plan.v1",
  "phase": "connectivity",
  "stage": "production",
  "sourceBaseRevision": "<bundle sourceBaseRevision>",
  "deployedRevision": "<exact deployed 40-hex revision>",
  "resourceRefs": ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
  "steps": [
    {
      "id": "production_preflight",
      "command": ["<probe executable>", "preflight"],
      "timeoutSeconds": 300
    },
    {
      "id": "direct_udp",
      "command": ["<packaged Omega probe>", "direct_udp"],
      "timeoutSeconds": 600
    },
    {
      "id": "tcp_fallback",
      "command": ["<packaged Omega probe>", "tcp_fallback"],
      "timeoutSeconds": 600
    },
    {
      "id": "turn_tls",
      "command": ["<packaged Omega probe>", "turn_tls"],
      "timeoutSeconds": 600
    }
  ]
}
```

Keep the plan outside Git when its argv names private evidence paths. The
runner rejects shells, reordered or missing steps, source drift, mutable
revision identities, unknown fields, and command timeouts outside one second
through one hour. Each command is spawned as exact argv without a shell and
must emit exactly one JSON object on stdout:

```json
{
  "schemaVersion": "openagents.livekit_probe_result.v1",
  "phase": "connectivity",
  "stepId": "direct_udp",
  "observedAt": "2026-07-31T12:00:00.000Z",
  "result": {
    "roomJoined": true,
    "microphonePublished": true,
    "sarahAudioSubscribed": true,
    "selectedPathObserved": true,
    "sessionSettled": true,
    "p95JoinMs": 500,
    "p95FirstAudioMs": 900
  }
}
```

The phase determines the exact ordered steps:

| Phase        | Ordered step IDs                                                                                                                                                                                                       |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| connectivity | `production_preflight`, `direct_udp`, `tcp_fallback`, `turn_tls`                                                                                                                                                       |
| load         | `alpha_load`                                                                                                                                                                                                           |
| drills       | `sfu_pod_drain`, `sfu_node_loss`, `zone_loss`, `redis_failover`, `signaling_backend_removal`, `certificate_renewal`, `turn_backend_loss`, `worker_crash`, `provider_disconnect`, `quota_exhaustion`, `server_rollback` |
| secret scan  | `runtime_secret_scan`                                                                                                                                                                                                  |
| cost         | `billing_reconciliation`                                                                                                                                                                                               |
| rollback     | `scoped_rollback`                                                                                                                                                                                                      |

The `result` for each step is the corresponding object documented below and
enforced by `livekit-ops-policy.mjs`. A drill step emits one drill row including
its matching `scenario`; the runner supplies the outer `drills` array. A
connectivity mode omits `mode`; the runner binds it from the required step ID.
The other phases emit their complete `results` object.

Validate the plan without executing it:

```sh
node scripts/cloud/livekit-production-acceptance.mjs \
  --plan <private-plan.json> \
  --bundle infra/livekit/bundle.json
```

Execute read-only connectivity, load, secret-scan, or cost probes and write a
mode-0600 private observation outside the repository plus the policy-valid
public receipt:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-production-acceptance.mjs \
  --plan <private-plan.json> \
  --bundle infra/livekit/bundle.json \
  --private-observation <outside-repository/private-observation.json> \
  --receipt docs/ops/receipts/livekit/production-<phase>-<UTC>.json \
  --apply
```

Drills and rollback require the additional
`--allow-controlled-mutation` interlock. Dry-run never accepts that flag. The
runner captures stdout and stderr in memory, does not echo them, and adds an
opaque evidence ref hashing the step ID, exact argv, stdout, and stderr. The
public receipt contains the evidence refs and aggregate result digest, not the
commands, provider diagnostics, topology, secrets, media, or transcripts. A
failed command, invalid timestamp, malformed probe result, or failed policy
condition writes neither observation nor receipt.

Generate drill and rollback plans with the controlled adapter after the live
actions and restoration checks have written their private captures:

```sh
node scripts/cloud/livekit-controlled-plan.mjs \
  --phase drills \
  --bundle infra/livekit/bundle.json \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --resource-ref gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod \
  --input sfu_pod_drain=<private-capture.json> \
  --input sfu_node_loss=<private-capture.json> \
  --input zone_loss=<private-capture.json> \
  --input redis_failover=<private-capture.json> \
  --input signaling_backend_removal=<private-capture.json> \
  --input certificate_renewal=<private-capture.json> \
  --input turn_backend_loss=<private-capture.json> \
  --input worker_crash=<private-capture.json> \
  --input provider_disconnect=<private-capture.json> \
  --input quota_exhaustion=<private-capture.json> \
  --input server_rollback=<private-capture.json> \
  --output <private-drill-plan.json>
```

The drill capture schema is
`openagents.livekit_controlled_drill_capture.v1`. It binds the source and
deployed revisions, scenario, observation time, SHA-256 precondition/action/
restoration receipt digests, scoped-target and restoration observations,
worker-generation and provider-session maxima, terminal accounting state,
old-generation rejection, fresh-generation admission, and the literal speech
continuity disposition `not_claimed`. The adapter derives the acceptance row;
there is no input field that says the drill passed. These captures do not
replace the live fault driver. A capture without an actually observed scoped
mutation and restoration is invalid.

#### Non-destructive probe adapters

Collect the preflight inventory directly from the live read-only Kubernetes
API, public DNS, and TLS handshakes. First write a private packaged-candidate
attestation after independently verifying the installed release signature and
launch:

```json
{
  "schemaVersion": "openagents.omega_packaged_attestation.v1",
  "observedAt": "<RFC3339>",
  "releaseSigned": true,
  "launchSucceeded": true,
  "artifactDigest": "sha256:<64 lowercase hex>"
}
```

Then run:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-connectivity-inventory.mjs \
  --bundle infra/livekit/bundle.json \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --packaged-omega-attestation <private-packaged-attestation.json> \
  --output <private-inventory.json> \
  --apply
```

The collector reads only the LiveKit server and Sarah worker Deployments,
server ConfigMap, SFU Nodes, signaling Ingress, TURN Service, and signaling/TURN
certificate status. It resolves the two public names and performs authorized
TLS handshakes. It does not read Secrets, logs, media, transcripts, bearer
tokens, or provider credentials. The output contains no address, hostname,
certificate body, workload environment, or secret: only readiness counts,
digest-pin/config booleans, address-equality booleans, and certificate
authorization/expiry booleans. Keep the exclusive mode-0600 output outside the
repository.

`scripts/cloud/livekit-production-plan.mjs` creates the exact collector plan
for `connectivity`, `load`, `secret_scan`, or `cost`. It refuses drill and
rollback plans. Give it one private capture per ordered step:

```sh
node scripts/cloud/livekit-production-plan.mjs \
  --phase connectivity \
  --bundle infra/livekit/bundle.json \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --resource-ref gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod \
  --input production_preflight=<private-inventory.json> \
  --input direct_udp=<private-direct-udp.json> \
  --input tcp_fallback=<private-tcp-fallback.json> \
  --input turn_tls=<private-turn-tls.json> \
  --output <private-plan.json>
```

The exclusive mode-0600 plan invokes
`scripts/cloud/livekit-acceptance-probe.mjs`. That adapter makes no network
request and mutates no provider state. It converts only the following closed,
revision-bound captures:

- `openagents.livekit_connectivity_inventory.v1`: release-signature and launch
  status for the installed Omega artifact; desired and ready server/worker
  replicas; digest pinning; host networking; `use_external_ip`; count of SFU
  node external addresses; provisioned signaling/TURN addresses; DNS equality;
  and independently trusted, unexpired signaling and TURN certificates.
- `openagents.livekit_connectivity_capture.v1`: one forced network mode and 3
  through 100 packaged-Omega sessions. Every sample records only selected path,
  join/publication/subscription/settlement booleans, join latency, and first
  audio latency. Direct UDP requires UDP available; TCP fallback requires UDP
  blocked; TURN/TLS requires UDP and non-TLS TCP blocked.
- `openagents.livekit_load_capture.v1`: opaque session refs with start/end,
  terminal status, and first-audio latency; the timestamped exact cap-plus-one
  HTTP `409` `sarah_voice_livekit_capacity_limit` refusal; and at least three
  bounded telemetry samples containing
  active/capacity rooms, SFU/worker CPU, and packet loss. Cap and telemetry
  timestamps must fall inside the session window, and the capture must be
  sealed within five minutes of the last settlement. The adapter derives peak
  overlap, p95, settlement counts, minimum spare capacity, and maxima. It does
  not accept those aggregate pass values from the operator.
- `openagents.sarah.livekit_privacy_scan.v1`: the output written by
  `livekit-privacy-scan.mjs`. The adapter requires a passing, source-bound
  eight-scope scan and projects only the six runtime scopes required by the
  standalone secret-scan receipt.
- `openagents.livekit_cost_capture.v1`: categorized gross Google billing-export
  rows and credits, current gross monthly forecast, fixed planning floor, and
  the normalized active budget policy. The six required categories are GKE
  control plane, SFU compute, worker compute, Redis, load
  balancing/networking, and observability. The budget must retain the
  50%/80%-current and 100%-forecast thresholds, at least one notification
  channel, project filter, and `service=livekit` filter. The adapter computes
  gross daily cost and never subtracts credits or folds OpenAI into Google
  cost.

Every capture includes the bundle source revision, exact deployed revision
(except the existing privacy scan, whose source binding is preserved), and a
real observation timestamp. Capture files stay outside Git and must be regular
mode-0600-or-stricter files, never symlinks. The adapters fail on missing
categories/scopes, duplicate load sessions, non-overlapping load, incorrect
forced-path controls, stale revision identities, nonterminal sessions, or
policy limits. They emit only the aggregate probe JSON consumed by the
collector.

### Connectivity matrix

Repeat the canary matrix against production with the packaged Omega candidate.
All three paths must pass:

| Path         | Required observation                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| Direct UDP   | node external candidate selected; room/audio works; exact terminal accounting                                |
| TCP fallback | UDP blocked; WebRTC TCP selected; room/audio works; exact terminal accounting                                |
| TURN/TLS     | UDP and non-TLS TCP blocked; TURN/TLS selected on the TURN name; room/audio works; exact terminal accounting |

An endpoint responding to HTTPS is not media acceptance.

### Headless Sarah worker proof

The production-gated Node harness supplements the packaged Omega matrix with a
repeatable worker-level proof. Its dry-run mode is the default and performs no
network request:

```sh
pnpm --dir apps/sarah-livekit-agent acceptance
```

A live run uses the production API and `@livekit/rtc-node` to perform the real
bearer-authenticated admission and session requests with
`requestedTransport=livekit_room_v1`. It joins with the returned participant
grant, publishes a microphone track, observes non-silent Sarah audio and the
ephemeral `lk.transcription` stream, samples the selected publisher and
subscriber ICE paths, disconnects, and waits for the terminal settlement. The
private and community scenarios start concurrently and must overlap.

Production accounting allows one active Sarah voice generation per owner.
Consequently, the concurrent matrix requires two distinct authenticated owners:
one active Sarah alpha or owner-entitled identity for the private scenario and
one active member of the named community/channel for the community scenario.
Using one owner twice is a harness preflight error, not a reason to weaken the
ledger invariant.

Before a community run, verify that the monolith's
`SARAH_NOSTR_EXPECTED_PUBKEY` equals the worker
`sarah-nostr-projection` ConfigMap value and that the owned relay advertises
kind `30382` in its exact NIP-29 group allowlist:

```sh
curl -fsS -H 'Accept: application/nostr+json' \
  https://relay.openagents.com/ |
  jq -e '.supported_kinds | index(30382) != null'
```

Do not replace the allowlist with a wildcard. A worker close, including a
provider or projection error, must stop the corresponding Sarah authority and
retire its community-room rendezvous **and its room members** so the next
admitted generation is not blocked by a stale active room.

Room cleanup is bounded (EP263-LK H4, #9282). The terminal-room reconciler
retries a failed `sarah_livekit_room_bindings` cleanup at most
`SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS` (8) times, backing off exponentially from
15 seconds to a 900 second cap, then moves the row to the terminal
`cleanup_abandoned` state with `cleanup_abandoned_at` set. Giving up is
reported as its own `sarah_livekit_terminal_rooms_abandoned` scheduled-tick
event and counted in `sarah_livekit_terminal_rooms_reconciled`. A row that
reaches the cap without a terminal mark — a process that died between the claim
and the mark — is dead-lettered lazily on the next claim, so no row can sit at
or above the cap in a retryable state. `cleanup_abandoned` is excluded from the
`binding.state IN ('prepared','active')` authority predicates exactly as
`cleanup_failed` already was; this bounds resource use and does not widen
authority. To inspect what was abandoned:

```sql
SELECT session_ref, room_ref, cleanup_attempt_count, cleanup_abandoned_at
  FROM sarah_livekit_room_bindings
 WHERE state = 'cleanup_abandoned'
 ORDER BY cleanup_abandoned_at DESC;
```

An abandoned row means the broker could not delete that room. Confirm the room
is genuinely gone at the SFU before dismissing it.

Mint the two acceptance bearers with the checked-in operator tool. Do not
hand-build a one-shot minting helper for a run and delete it afterwards: that
made every acceptance a slightly different, unreviewable procedure, and a
deleted helper is not evidence (EP263-LK H1, #9282).

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
pnpm exec tsx \
  apps/openagents.com/workers/api/scripts/mint-livekit-acceptance-bearer.ts \
  --role both --credential-file /tmp/oa-livekit-acceptance.env

set -a; . /tmp/oa-livekit-acceptance.env; set +a
```

It reads `oa-livekit-acceptance-private-nostr-key` and
`oa-livekit-acceptance-community-nostr-key` from Secret Manager in memory only,
signs one NIP-98 kind-27235 event per identity against
`POST /api/omega/auth/session`, and writes the four
`OA_SARAH_LIVEKIT_ACCEPTANCE_*` variables to a mode-0600 file outside the
repository. It refuses a credential path inside the working tree, refuses two
identities that resolve to the same owner, prints only the owner ref, expiry,
and a SHA-256 digest of each bearer, and creates no durable identity row. The
output uses an exclusive create with mode `0600`: an existing file or final
symlink is refused instead of truncated with whatever permissions it already
had. The repository path guard decodes file-URL escapes before comparison.
Bearers expire in 15 minutes, so mint immediately before the run. For another
run, use a fresh path or explicitly delete the expired file first; the tool will
not overwrite it.

Keep the bearer tokens in environment variables and keep the two 24 kHz mono
signed-16-bit PCM prompts outside Git. The prompts must contain an innocuous
spoken request followed by enough silence for the admitted semantic VAD to
finish the turn.

#### Producing the two PCM prompts

The prompts are deliberately not in Git, so a fresh machine has none and every
`--apply` run refuses before its first network call. That absence blocked the
whole failure-drill lane once (EP263-LK, 2026-07-31). Regenerate them locally in
about ten seconds; the harness never sends the trailing silence it appends
itself, so a short natural utterance is enough.

```sh
mkdir -p ~/work/.secrets/livekit-acceptance-prompts
cd ~/work/.secrets/livekit-acceptance-prompts

say -v Samantha -o private.aiff \
  "Hi Sarah, this is the private acceptance drill. Please tell me briefly what you are working on right now."
say -v Samantha -o community.aiff \
  "Hi Sarah, this is the community acceptance drill. Please give the channel a short status update."

for n in private community; do
  afconvert -f WAVE -d LEI16@24000 -c 1 "$n.aiff" "$n.wav"
  # Strip the 44-byte WAVE header: the harness reads headerless little-endian s16.
  python3 -c "import wave,sys;n=sys.argv[1];w=wave.open(n+'.wav','rb');open(n+'.pcm','wb').write(w.readframes(w.getnframes()))" "$n"
  rm -f "$n.aiff" "$n.wav"
done
```

Verify before use. The harness reads the file as headerless little-endian s16 at
24 kHz mono and will happily play noise if the format is wrong, so check the
byte count rather than trusting the converter:

```sh
for n in private community; do
  python3 -c "import os,sys;n=sys.argv[1];b=os.path.getsize(n+'.pcm');print(f'{n}: {b} bytes, {b//2} samples, {b/2/24000:.2f}s, even={b%2==0}')" "$n"
done
# A WAVE header would leave an odd remainder or a 22-sample offset; both show up here.
ffprobe -f s16le -ar 24000 -ac 1 -i private.pcm      # optional second opinion
```

Store them outside the repository — `acceptance-cli.ts` refuses a PCM path under
the working tree — and never commit them. The pinned location above keeps them
next to the other machine-local operator secrets.

Then run:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER='<private bearer>' \
OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF='<private owner ref>' \
OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER='<community bearer>' \
OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF='<community owner ref>' \
pnpm --dir apps/sarah-livekit-agent acceptance -- \
  --private-pcm <private-prompt.pcm> \
  --community-pcm <community-prompt.pcm> \
  --community-ref <community-ref> \
  --channel-ref <channel-ref> \
  --receipt docs/ops/receipts/livekit/production-sarah-headless-<UTC>.json \
  --apply
```

The harness writes no receipt unless both scenarios pass. The public receipt
contains timing, boolean media/transcription/ICE observations, exact charge,
and digests of settlement refs. It never contains bearer tokens, grants,
owner/device/room/community refs, PCM, media, or transcript text. The supplied
source revision is labeled as operator-supplied; bind it independently to the
deployment receipt before using the headless receipt for closeout.
`retainedMedia=false` and `retainedTranscript=false` describe only the harness
process and public receipt. They do not prove privacy in the cluster, provider,
Redis, object storage, traces, crash artifacts, or logs; the separate production
secret and raw-media/transcript scan receipt remains mandatory.

This one naturally selected ICE path per scenario does not replace the forced
direct-UDP, TCP-fallback, and TURN/TLS packaged Omega matrix. It also does not
prove UI behavior or uninterrupted media through SFU failure.

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

The cap-plus-one request must receive HTTP `409` with
`sarah_voice_livekit_capacity_limit`. That response is emitted only after the
refused reservation reaches the normal zero-charge terminal release. If the
release fails, the API returns
`sarah_voice_livekit_capacity_release_failed` with HTTP `503`; that is a failed
load run with pending accounting, not cap evidence. A generic storage or
LiveKit-unavailable response cannot prove the hard cap.

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

> **Closed 2026-07-31: the single-session driver exists.**
> `apps/sarah-livekit-agent/src/drill-driver.ts` (`runSarahLiveKitDrill`) admits
> one scenario, completes a turn, holds the session open, injects the fault at a
> recorded instant, and watches settlement over HTTP so the reading survives the
> loss of the transport. `drill-cli.ts` is the operator surface and is dry-run by
> default. The earlier blocker — that `runSarahLiveKitAcceptance` requires both a
> private and a community scenario, asserts their overlap, and can therefore
> never produce `concurrentBillableSessionCount: 1` — is no longer the reason any
> observation is unobserved.
>
> **Two live blockers replaced it, and both are recorded from real API calls.**
>
> - **`pods/exec` is Forbidden for the drill automation identity.** The granted
>   `Role/sarah-livekit-drill-automation` carries `pods: delete` and
>   `pods/log: get, list` and nothing else, exactly as the permissions table
>   below states. A 2026-07-31 receipt recorded exec as "granted in practice";
>   that reading came through a `gke-gcloud-auth-plugin` cache poisoned by the
>   owner identity, and a real `kubectl exec` after clearing the cache returns
>   `Forbidden`. Do not run a drill as the owner identity to route around this:
>   the narrow namespace Role is what keeps a pod deletion scoped, and the owner
>   identity discards that scoping for the destructive step as well as the read.
>   Use `--gauge-source managed_prometheus`, which is the substitution this
>   runbook already names. Its stated insufficiency — a 30 s scrape against a
>   ~21 s session — is resolved by the held-open session, not by new authority,
>   so that source requires `--hold-ms` of at least 90 s.
> - **Source fix: accounting uncertainty no longer owns the voice slot
>   indefinitely.** Migration `0128_sarah_voice_accounting_escalation.sql` and
>   scheduled maintenance preserve the full hold and uncertain state for 15
>   minutes, then record a durable escalation and remove only the per-owner
>   voice-concurrency lock. The unresolved hold still reduces spendable credit
>   and still requires the provider-export reconciliation procedure. Never
>   guess usage or release the hold to unblock a drill. Treat the old behavior
>   as a live blocker until the migration and API revision are deployed and the
>   escalation event is observed on that revision.

> **Closed source and live, 2026-07-31: community bootstrap carries the
> authenticated device ref.** Commit `c76d2af6a4` threads the admitted device
> ref through the internal summon boundary. The subsequent concurrent private
> and community acceptance passed and is retained at
> `docs/ops/receipts/livekit/2026-07-31-ep263lk-community-join-acceptance.json`.
> A future `device_ref_required` response is a regression or deployment-drift
> signal, not the standing reason to omit the community drill.

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

The provider-disconnect drill uses the generation-scoped acceptance control
documented under **Sarah production matrix evidence** below. Do not implement
item 9 by changing shared egress, a firewall, NetworkPolicy, Secret, provider
key, LiveKit configuration, or a Deployment during the drill.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase drills \
  --bundle infra/livekit/bundle.json \
  --input <private-drill-observation.json> \
  --receipt docs/ops/receipts/livekit/production-drills-<UTC>.json \
  --apply
```

#### The SFU-loss drill (`sfu_loss`)

Added 2026-07-31. The rc29 evidence manifest recorded `sfu_loss_bounded` as
_"no such scenario is even defined"_. This is that definition. It is drill item
2, abrupt SFU loss, expressed as one scenario the failure matrix and the gate
recorder both enforce.

**The claim under test.** An SFU instance is destroyed beneath a live,
admitted Sarah generation. Within a bounded window the session reaches a
terminal accounting state, that state reconciles exactly, no room binding,
worker generation, or provider session is left behind, and the loss is not
reported as a clean finish.

**The fault.** `delete_exact_sfu_pod`: delete the one `livekit-server` pod
hosting the drill session's room. Deliberately _not_ a graceful drain, which is
item 1 and is not loss. Deliberately _not_ a node deletion, which would take the
co-located Sarah worker with it and make the result indistinguishable from
`planned_worker_crash`. Never a shared firewall, NetworkPolicy, ConfigMap,
Secret, or Deployment change — the same restriction the provider-disconnect
drill carries, and for the same reason: those faults are not scoped to one
generation, so nothing observed after them can be attributed to it.

**The bound.** Thirty seconds
(`SARAH_LIVEKIT_SFU_LOSS_BOUND_MS` in
`apps/sarah-livekit-agent/src/failure-matrix.ts`), measured from pod deletion to
terminal accounting. The ceiling is not `max_session_seconds`: a session that
survives until the 300 second expiry sweep has demonstrated the `timeout`
scenario, not bounded SFU-loss handling, and has held a credit hold and an
orphaned room for the intervening minutes. The worker publishes `lease_check`
every five seconds, so thirty is a small multiple of the cadence the authority
already runs at.

**Admitted terminal reasons.** `worker_shutdown`, `participant_left`, or
`worker_error`. The worker protocol has no media-transport-loss reason, so which
one appears depends on which watchdog noticed the room vanish first; all three
are bounded deterministic settlements. `session_expired` and `completed` are
refused: the first proves only the deadline, and the second is the authority
recording a clean finish for a session whose transport was destroyed underneath
it, which is the silent-loss failure this row exists to catch. A run producing
either is recorded `contradicted` — it is a finding about the system, not a run
to repeat until it looks better.

**Run exactly one billable session.** The drill requires
`concurrentBillableSessionCount: 1`. Deleting an SFU pod takes every room on
that instance, so a second live session would be collateral damage to another
owner. It is also what makes the target identifiable: with one room in the
cluster, exactly one server pod reports a nonzero room gauge.

Steps:

1. Confirm the cluster is idle, then start one private acceptance session and
   complete at least one turn, so the scenario has real provider usage to
   settle. Leave it live.

2. Find the pod hosting the room. `livekit_room_total` is a per-instance gauge
   (`Namespace: livekit`, `Subsystem: room`, `Name: total`,
   `ConstLabels: node_id, node_type`), exposed on `prometheus_port: 6789`:

   ```sh
   for pod in $(kubectl -n livekit-system get pods \
       -l app.kubernetes.io/name=livekit-server -o name); do
     printf '%s ' "$pod"
     kubectl -n livekit-system exec "$pod" -- \
       wget -qO- http://127.0.0.1:6789/metrics | grep '^livekit_room_total'
   done
   ```

   Exactly one pod must report a nonzero total. If two do, another session is
   live: stop and start over rather than injecting an unattributable fault.

3. Record the fault instant and delete that one pod. `--grace-period=0` makes
   this a loss rather than a drain:

   ```sh
   date +%s%3N                       # faultInjectedAtMs
   kubectl -n livekit-system delete pod <the-one-pod> --grace-period=0 --force
   ```

4. Record `mediaLossObservedAtMs` from the packaged client or the worker's own
   log, and `terminalAtMs` from the settlement event.

5. After settlement, read the room binding and confirm nothing was orphaned:

   ```sql
   SELECT state, cleanup_attempt_count, cleanup_abandoned_at
     FROM sarah_livekit_room_bindings
    WHERE session_ref = '<the drill session>';
   ```

   `cleaned` or `cleanup_failed` is terminal. `cleanup_abandoned` means the
   reconciler gave up and the room is orphaned at the SFU: that fails the row.
   Confirm no row for this session remains `prepared` or `active`, and that no
   worker generation or provider session survives it.

6. Record it. Both paths read the same constant and the same fault name, so the
   matrix and the recorder cannot drift:

   ```sh
   # As one cell of the full matrix, alongside the other seven scenarios.
   OA_SARAH_LIVEKIT_FAILURE_MATRIX_OWNER_GATE=I_ACCEPT_EP263_SARAH_FAILURE_MATRIX \
   pnpm --dir apps/sarah-livekit-agent failure-matrix -- \
     --input <private-observation.json> \
     --receipt docs/ops/receipts/livekit/production-drills-<UTC>.json --apply

   # And as the sfu_loss_bounded observation on the release-gate row.
   pnpm --dir apps/sarah-livekit-agent gate-observation -- \
     --row sarah-livekit-failure \
     --observations <private-observations.json> \
     --binding <private-binding.json> \
     --operator-ref "<operator identity>" \
     --receipt docs/ops/receipts/livekit/gate/<name>.json --apply
   ```

   The matrix scenario carries `sfuLoss` with `sfuInstanceDigest`,
   `workerInstanceDigest` (which must differ — a fault that landed on the worker
   is a different drill), `faultInjectedAtMs`, `mediaLossObservedAtMs`,
   `roomBindingTerminalState`, `roomBindingObservedAtMs`, the three residual
   counts, and `concurrentBillableSessionCount`. The gate observation carries
   `faultInjected: "delete_exact_sfu_pod"` and `boundedWithinMs`.

**Permissions.** Step 2 needs `pods/exec` and step 3 needs `delete pods` in
`livekit-system`. Check before scheduling a window, because the fault instant is
a poor time to discover a missing role.

> **`kubectl auth can-i` is not sufficient evidence here, and its false `yes`
> already cost this lane a wrong conclusion (2026-07-31).** On GKE the check is
> answered optimistically for subresources: `auth can-i get pods/log` returned
> `yes` for the automation identity at a moment when the real
> `kubectl logs` call returned `Forbidden`. **Verify every permission with a real
> API call**, using `--dry-run=server` for the destructive one so the check
> proves authorization without deleting anything:
>
> ```sh
> export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
> POD=$(kubectl -n livekit-system get pods -o name | head -1); POD=${POD#pod/}
> kubectl -n livekit-system logs "$POD" --tail=1                 # pods/log
> kubectl -n livekit-system delete pod "$POD" --dry-run=server   # delete pods
> ```
>
> **Also clear the shared credential cache before trusting any result.** The
> `gke-gcloud-auth-plugin` caches its token in `~/.kube/gke_gcloud_auth_plugin_cache`,
> which is **outside** `CLOUDSDK_CONFIG`. Running one `kubectl` as the
> interactive owner account poisons that cache, and every later command that
> _sets_ `CLOUDSDK_CONFIG` still authenticates as the owner. That makes a
> namespace-scoped grant look far broader than it is. Run
> `rm -f ~/.kube/gke_gcloud_auth_plugin_cache` after any identity switch and
> confirm with `kubectl auth whoami` before recording a permission finding.

**The granted authority (2026-07-31).** The automation identity holds
namespace-scoped RBAC in `livekit-system` only, under owner authority, so the
pod-deleting drills and the privacy scan's `pods` scope stop being blocked:

| Object                                                      | Grant                                                                          |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `Role/sarah-livekit-drill-automation` (ns `livekit-system`) | `pods: delete`; `pods/log: get, list`                                          |
| `RoleBinding/sarah-livekit-drill-automation`                | the above to user `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com` |

Deliberately excluded, and verified `Forbidden` by real calls: `pods/exec` (it
is arbitrary command execution inside production pods), `pods: create`, and
anything at all in another namespace. A namespace `Role` was chosen over a
project-level custom IAM role because IAM conditions cannot scope a GKE
permission to one namespace, so the IAM route would have granted pod deletion
across every cluster in the project.

**Step 2 without `pods/exec`.** Because exec is not granted, read the same gauge
from Managed Prometheus instead — `pod-monitoring.yaml` already exports
`livekit_room_total` with `pod` and `node` labels:

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
curl -sG https://monitoring.googleapis.com/v3/projects/openagentsgemini/timeSeries \
  -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  --data-urlencode 'filter=metric.type="prometheus.googleapis.com/livekit_room_total/gauge"' \
  --data-urlencode "interval.startTime=$(date -u -v-10M +%Y-%m-%dT%H:%M:%SZ)" \
  --data-urlencode "interval.endTime=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
```

**This substitution became sufficient on 2026-07-31, by the held-open session
rather than by new authority.** The `PodMonitoring` scrape interval is 30 s,
which could not identify the hosting pod inside a ~21 s acceptance session — the
condition this runbook set was "until either the drill runs against a held-open
session or the scrape interval is lowered". The single-session driver holds the
session open, so the first branch is now satisfied and the drill no longer needs
`pods/exec`.

Use `--gauge-source managed_prometheus`, which performs this read inside the
drill and is the source the drill automation identity can actually reach. Two
constraints come with it and are enforced in code:

- `--hold-ms` must be at least 90 s. A sample is up to one 30 s scrape plus the
  measured ~14 s ingestion latency old, so a shorter hold can read a gauge taken
  before the drill's room existed.
- Only the latest sample per instance is read, and a running instance with no
  sample in the window is a refusal rather than a zero. A maximum over the window
  would let a room that ended a minute ago keep naming its old host, and treating
  an unread instance as idle is how the pod carrying the room gets skipped and a
  healthy one gets destroyed.

Do not record `sfu_loss_bounded` from a pod guessed without a nonzero gauge
reading, and do not run the drill as the owner identity to obtain `pods/exec`:
the namespace-scoped Role is what keeps the deletion bounded, and running as
owner discards that scoping for the destructive step too.

### Secret, log, and retention scan

#### Retention canaries: injected 2026-07-31

The collector requires at least one retention canary, and **a canary generated
at scan time is worthless**: material that was never introduced into the system
can never be found, so that axis would report clean while having tested nothing.
The other five axes (OpenAI key, Sarah identity, PEM material, raw media,
transcripts) are meaningful without injection; the retention axis is not.

These canaries were spoken into real production Sarah sessions through the
normal media path, so they genuinely entered audio, transcription, and anything
downstream that retains either. They are deliberately non-secret and are
published here so the scan can search for them.

| Journey   | Spoken canary phrase                     |
| --------- | ---------------------------------------- |
| private   | `marmalade obelisk seven three one nine` |
| community | `juniper caravan four eight two six`     |

**Injection window: `2026-07-31T18:41:26Z` to `2026-07-31T18:41:51Z`.** Evidence:
`docs/ops/receipts/livekit/production-sarah-canary-20260731T184800Z.json`,
outcome `passed`, with `sarahTranscriptionObserved: true` on both journeys, which
is what proves the phrases reached the transcription path rather than only the
wire.

Search for the distinctive two-word head first — `marmalade obelisk` and
`juniper caravan` — because those bigrams will never occur naturally in this
system and, unlike the digits, are rendered consistently. Then search the digit
tails in both word and numeral form (`seven three one nine` and `7319`,
`four eight two six` and `4826`), because a transcriber may emit either.

Two honest limits on what a clean result proves. The public-safe acceptance
receipt contains no transcript text by design, so the exact rendering the
transcriber produced is not verifiable from it; search the phonetic variants, not
one canonical string. And these canaries are bounded by the retention horizon of
whatever they entered — a scan run long after the window may find nothing because
the material aged out rather than because nothing retained it. Record the scan
time against the window above.

> **This scan is executable.** All four blockers are cleared and the three
> prose-only scopes — `packaged_clients`, `object_storage`, and `traces` — are
> defined, with the read-only Redis path built and proven, in
> [`2026-07-31-sarah-livekit-privacy-scan-scope-definitions.md`](2026-07-31-sarah-livekit-privacy-scan-scope-definitions.md).
> Read it before attempting a run. It carries the exact in-scope artifact and
> bucket sets, the measured per-scope collection cost for planning the two-hour
> window, and one integrity requirement: the retention canaries must be injected
> through a live session before collection, or that axis reports clean while
> having tested nothing.
> [`2026-07-31-sarah-livekit-privacy-scan-executability.md`](2026-07-31-sarah-livekit-privacy-scan-executability.md)
> is the superseded finding, retained for its sequencing argument.

Seal each completed read-only export with the executable manifest builder.
Use the real collection start/end timestamps for that scope:

```bash
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-privacy-export.mjs \
  --scope logs \
  --source-base-revision "$(git rev-parse HEAD)" \
  --started-at <ISO-8601> \
  --completed-at <ISO-8601> \
  --input <private-logs-export-directory> \
  --apply
```

Repeat for every required scope. The builder refuses incomplete evidence
shapes and emits only aggregate counts; it never prints object names or
contents. The executable collector is
`scripts/cloud/livekit-privacy-scan.mjs`. It scans
eight exact scopes: packaged Omega, every other packaged client, Sarah worker
pods, logs, Redis, object storage, traces, and crash artifacts. It compares the
real production OpenAI key against every scope except the server-side pod
export, compares the real `principal.sarah` private identity against every
scope, and compares private synthetic retention canaries against every runtime
persistence scope. It also detects private-key material, named transcript
objects and payloads, and common retained media formats and signatures.

- the fully unpacked, release-signed Omega artifact and fully unpacked artifacts
  for every other shipped client;
- every current Sarah worker pod's environment and readable filesystem,
  including `/tmp`, plus current and previous-container output;
- SFU, worker, load-balancer, Redis, and provider-safe logs;
- Redis keys and values through a read-only TLS client;
- every in-scope Cloud Storage object name and content;
- complete bounded trace exports;
- every in-scope crash, Error Reporting, core, and support artifact.

Capture all eight scopes over the same bounded acceptance window into private
directories outside Git. Each directory must contain at least one payload file
and `_scope-manifest.json` with this closed schema:

```json
{
  "schemaVersion": "openagents.sarah.livekit_privacy_scope_export.v1",
  "scope": "logs",
  "sourceBaseRevision": "<exact-deployed-40-hex-revision>",
  "collectionMode": "read_only",
  "complete": true,
  "startedAt": "<RFC3339>",
  "completedAt": "<RFC3339>",
  "objectCount": 1,
  "byteCount": 1234
}
```

`objectCount` and `byteCount` cover payload files recursively and exclude the
manifest. The collector rejects missing or additional scopes, incomplete or
stale manifests, a capture longer than two hours, source-revision drift,
count/byte mismatches, empty evidence, symlinks, special files, and objects
larger than 256 MiB. Split a larger read-only export into smaller payload
files. An unavailable backend is a failed gate, not a zero-finding scan.

Inject one or more unique synthetic retention canaries through the admitted
session, then keep only those synthetic values in mode-0600 files. Do not
export the two production secrets. The scanner reads the exact
`sarah-nostr-identity-secret` and `oa-livekit-prod-openai-api-key` versions
directly from Secret Manager into bounded process memory, scans the eight
exports, overwrites its in-memory buffers, and emits only counts and digests.
The executing identity must already have access to those exact resources; the
scanner does not grant or broaden IAM.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-privacy-scan.mjs \
  --source-base-revision <exact-deployed-40-hex-revision> \
  --gcp-secret-manager \
  --retention-canary-file <private-canary-file> \
  --scope packaged_omega=<private-export-directory> \
  --scope packaged_clients=<private-export-directory> \
  --scope pods=<private-export-directory> \
  --scope logs=<private-export-directory> \
  --scope redis=<private-export-directory> \
  --scope object_storage=<private-export-directory> \
  --scope traces=<private-export-directory> \
  --scope crash_artifacts=<private-export-directory> \
  --output <private-privacy-observation.json> \
  --apply
```

The private result contains only per-scope counts, byte counts, completion
states, and SHA-256 evidence digests. It never contains a matched value, object
name, secret, transcript, media, or source path. A passing result's `results`
object can be embedded as the `privacyScan` field in the private Sarah matrix
observation. The matrix and standalone secret-scan validators now require
complete per-scope evidence and reject aggregate counts that do not equal the
scope totals.

Secret-file inputs remain fixture-only and require the separate
`OA_LIVEKIT_PRIVACY_FIXTURE_FILES=I_ACCEPT_TEST_FIXTURES_ONLY` gate. They are
not a production fallback. If Secret Manager access is unavailable, the scan
is unavailable; do not copy the values to make the gate pass.

For the six runtime persistence scopes, the same private evidence can also be
projected through the standalone gate:

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

Generate the private cost capture from the detailed BigQuery billing export
and the live Billing Budget API. The collector queries the exact project and a
bounded 1-through-31-day window, deterministically categorizes exact LiveKit
labels and resource identities, and fails on an identified row it cannot
categorize. Kubernetes-created network charges and project-level observability
charges are often unlabelled, so it conservatively includes every matching
load-balancer, address, NAT, network-egress, Logging, and Monitoring SKU in the
project window. That can overstate LiveKit cost but cannot silently understate
it. Gross cost and negative credits stay separate.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-cost-collector.mjs \
  --billing-export-table <project.dataset.gcp_billing_export_v1_table> \
  --billing-account <billing-account-id> \
  --window-start <YYYY-MM-DD> \
  --window-end <exclusive-YYYY-MM-DD> \
  --source-base-revision <exact-deployed-40-hex-revision> \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --fixed-floor-monthly-usd 1500 \
  --output <private-cost-capture.json> \
  --apply
```

The command writes nothing unless both read-only queries succeed, exactly one
`LiveKit production` budget has the admitted USD/project/label/threshold/
notification policy, at least one LiveKit billing row is attributable, and
every attributable row has a closed category. The gross monthly forecast is
derived from the observed daily gross run rate and never subtracts credits; it
cannot fall below the fixed planning floor. As of the last verified billing
inventory, the billing account had no detailed BigQuery billing export and the
automation identity could not read billing-account data. Those are external
data/IAM blockers, not reasons to hand-author rows. Enable a detailed usage
export and grant only the required BigQuery data-viewer plus Billing Budget
viewer permissions before executing this gate. Billing exports are not
retroactive, so preserve the first complete post-enable window as the earliest
eligible cost observation.

Project a measured cost observation only after that generated capture exists:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase cost \
  --bundle infra/livekit/bundle.json \
  --input <private-cost-capture.json> \
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

After deploying the API with `SARAH_LIVEKIT_ADMISSIONS=off`, create the private
drained-boundary receipt. Use a Cloud SQL Auth Proxy connection and standard
libpq variables. Keep `PGPASSWORD` in the environment; do not put a database
URL or password in the command line.

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
PGHOST=127.0.0.1 PGPORT=<proxy-port> PGUSER=<reader> \
PGPASSWORD='<from the admitted runtime secret path>' PGDATABASE=khala_sync_prod \
node scripts/cloud/livekit-admission-disable.mjs \
  --bundle infra/livekit/bundle.json \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --output <outside-repository/private-admission-disable-receipt.json> \
  --apply
```

The tool is read-only. It requires the latest Ready Cloud Run revision to
receive 100% of traffic with
`SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED=false`. It then requires zero active
LiveKit bindings and zero `reserved`, `connected`, or
`accounting_uncertain` LiveKit settlements. An uncertain hold is pending; it
cannot be omitted or rounded into a drained boundary.

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

First generate the single-step controlled plan from the private rollback
capture:

```sh
node scripts/cloud/livekit-controlled-plan.mjs \
  --phase rollback \
  --bundle <last-healthy-bundle.json> \
  --deployed-revision <exact-deployed-40-hex-revision> \
  --resource-ref gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod \
  --input scoped_rollback=<private-controlled-rollback-capture.json> \
  --output <private-rollback-plan.json>

OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-production-acceptance.mjs \
  --plan <private-rollback-plan.json> \
  --bundle <last-healthy-bundle.json> \
  --private-observation <outside-repository/private-rollback-observation.json> \
  --receipt docs/ops/receipts/livekit/production-rollback-<UTC>.json \
  --allow-controlled-mutation \
  --apply
```

The `openagents.livekit_controlled_rollback_capture.v1` input embeds the exact
admission-disable receipt and a bounded projection of the live runtime rollback
receipt. Its postcheck records the restored bundle, configuration, server
image, and worker image digests; active-room, pending-settlement, and silent
transport-switch counts; continued admission disable; and an ordered set of
before/after digests for unrelated services. The adapter derives all six
rollback acceptance booleans and refuses a changed pin, unrelated-service
digest, active room, pending settlement, or transport switch.

The older direct projection remains available for a complete private
observation produced by another independently validated harness:

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

### Sarah production matrix evidence

The headless Sarah runner no longer accepts a revision typed by the operator. On
`--apply` it reads the converged `sarah-livekit-agent` Deployment, requires one
digest-pinned container image, resolves that digest to exactly one Artifact
Registry `source-<40-hex>` tag, and binds the receipt to that revision. A stale
rollout, mutable image, missing source tag, or ambiguous source tags fails the
run before either bearer is used.

An authenticated settlement read can request
`x-openagents-sarah-livekit-acceptance: live-observation-v1`. The response is
still owner- and session-scoped and is available only for a LiveKit generation
admitted through the metered alpha acceptance cohort. It exposes no room,
owner, job, provider, hold, or
usage refs. It returns the literal `principal.sarah`, SHA-256 projections for
the job, provider session, provider configuration, room context, capability,
hold, usage set, and settlement, exact aggregate provider usage, and provider
admission/worker-close boundaries. The runner requires all eight identity
digests to differ, one worker job, one provider session, exact accounting,
nonzero response and transcription usage, and a positive charge equal to the
terminal settlement. This is an evidence projection, not a failure-injection
or general inspection API.

Project the complete private matrix only after live collection:

```sh
OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
node scripts/cloud/livekit-acceptance.mjs \
  --phase sarah_matrix \
  --bundle infra/livekit/bundle.json \
  --input <private-sarah-matrix-observation.json> \
  --receipt docs/ops/receipts/livekit/production-sarah-matrix-<UTC>.json \
  --apply
```

The private observation must include successful metered traffic plus
cancellation, explicit interruption, timeout, planned worker crash, provider
disconnect, hold exhaustion, and reconnect. Every scenario must have a
terminal observation, exact accounting, a correctly released or preserved
hold, no worker/provider overlap, and a required fresh generation. Reconnect is
collected only after the prior worker close boundary and before the new
provider admission boundary. The same observation includes audible-frame
fanout to at least two simultaneous subscribers and a zero-finding privacy scan
over packaged Omega, every other packaged client, pods, logs, Redis, object
storage, traces, and crash artifacts.

The validator cannot create these facts. Planned worker crash is driven by a
scoped pod deletion after the target generation is identified; provider
disconnect is driven by a generation-scoped provider egress drill; timeout is
the configured session deadline; hold exhaustion is the normal metering stop;
and cancellation/explicit interruption use the existing gateway control
protocol. Subscriber grants are minted by the scoped acceptance operator and
remain outside the repository. Until those live actions and scans have
completed, do not create a `sarah_matrix` receipt and do not claim this gate is
green.

Validate the terminal matrix with the narrower worker contract before
projecting it into the aggregate `sarah_matrix` receipt:

```sh
pnpm --dir apps/sarah-livekit-agent failure-matrix

OA_SARAH_LIVEKIT_FAILURE_MATRIX_OWNER_GATE=I_ACCEPT_EP263_SARAH_FAILURE_MATRIX \
pnpm --dir apps/sarah-livekit-agent failure-matrix -- \
  --input <private-terminal-matrix-observation.json> \
  --receipt docs/ops/receipts/livekit/production-sarah-terminal-matrix-<UTC>.json \
  --apply
```

The first command is the required preflight. It performs no network request,
fault injection, pod mutation, provider disconnect, or receipt write. The
second command is also non-mutating: the owner gate authorizes validation and
public receipt projection only. Keep the private input outside the repository.
The public receipt is mode 0600 and contains no room, owner, job, provider,
hold, usage, or settlement identifier.

The private input has exactly eight scenario rows in this order: `success`,
`cancellation`, `timeout`, `planned_worker_crash`, `sfu_loss`,
`provider_disconnect`, `hold_exhaustion`, and `reconnect`. `sfu_loss` was
inserted after `planned_worker_crash` by openagents `af65458919`; this paragraph
said seven until 2026-07-31, so an input written from it failed validation with
"failure matrix must contain every scenario exactly once". Each row must
include:

- the exact scenario-specific fault action and terminal reason;
- distinct SHA-256 projections for job, provider session, generation, hold,
  usage, and settlement authorities;
- exact input, output, cached-input, audio-input, and audio-output token
  counts, response/transcription/cancellation counts, and provider charge;
- reserved, charged, and released hold amounts where
  `reserved = charged + released`;
- terminal settlement charge exactly equal to provider charge and hold charge;
- one terminal event, maximum worker-generation count one, maximum provider
  session count one, and fresh admission required;
- SHA-256 accounting, fault, and privacy evidence projections; and
- zero secret, raw-media, and transcript findings.

The reconnect row additionally carries distinct previous/fresh generation
digests, the previous terminal time, the later fresh-generation start time,
and `settledGenerationRevived=false`. Identity projections must be distinct
across the entire matrix, not only within one row.

Do not use a broad worker deletion as a planned-crash drill. First disable new
admissions, prove the target worker pod has only the named acceptance
generation, record its immutable pod UID and generation evidence privately,
and delete that exact pod. Re-enable admissions only after accounting becomes
terminal and the replacement is Ready. The harness records
`delete_exact_worker_pod`; it never runs `kubectl`.

The worker deliberately disables proactive provider reconnect and exposes one
production-unavailable-by-default acceptance boundary for
`close_exact_provider_socket`. The checked-in production environment sets
`SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED=false`; while false the
route returns `404` before authentication or request parsing. Arming it requires
a separately approved, bounded acceptance revision with the value exactly
`true`. Arming is not evidence that the drill passed.

Arm and withdraw the window with the deploy, so the armed value belongs to a
revision that was built and smoked as a unit:

```bash
# Arm for one approved window.
SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE=on \
  scripts/deploy-cloudrun.sh production

# Withdraw. The committed value is "false", so a plain deploy closes it again.
scripts/deploy-cloudrun.sh production
```

Never commit `true` for this key. After withdrawing, run
`check-livekit-admission-drift.mjs` and confirm the serving revision reports
`false` again.

Run the provider-disconnect row as follows:

1. Confirm migration
   `0124_sarah_livekit_provider_disconnect_acceptance.sql` is applied and the
   API and Sarah worker run the exact candidate revision.
2. Stop new unrelated Sarah acceptance admissions. Identify one connected
   alpha-cohort session and record privately its exact `sessionRef`,
   `generation`, worker job projection, and the 64-lowercase-hex
   `providerSessionRefDigest` durably admitted for that generation. Do not use a
   raw OpenAI provider session identifier.
3. Prove the binding is active, the worker has neither stopped nor closed,
   provider accounting is pending, and no provider-disconnect directive already
   exists for the session and generation. Begin an audible response so the
   observation exercises a live provider socket.
4. With the route armed, send one administrator-authenticated request:

   ```sh
   curl --fail-with-body --silent --show-error \
     -X POST "$OPENAGENTS_API_ORIGIN/api/operator/sarah/livekit/provider-disconnect" \
     -H "authorization: Bearer $OPENAGENTS_ADMIN_BEARER" \
     -H "content-type: application/json" \
     -H "x-openagents-livekit-owner-gate: I_ACCEPT_EXACT_SARAH_PROVIDER_DISCONNECT" \
     --data '{
       "schema": "openagents.sarah.livekit-provider-disconnect-acceptance.v1",
       "requestRef": "<unique-private-request-ref>",
       "sessionRef": "<exact-session-ref>",
       "generation": 1,
       "providerSessionRefDigest": "<64-lowercase-hex-digest>",
       "acknowledgement": "disconnect_exact_provider_socket"
     }'
   ```

   Keep bearer tokens, refs, and the complete response outside Git. The
   successful response must name the same request, session, generation, and
   provider digest, report `state` as `requested` or `applied`, and report
   `sharedInfrastructureMutated: false`. An exact retry is idempotent; changing
   any authority under the request ref, or issuing a second request for the
   generation, must conflict.

5. Within the normal five-second worker lease interval, observe a durable
   `provider_disconnect_fault_applied` event bound to the same request,
   generation, worker job, and provider digest. The worker then fences the
   generation with terminal reason `provider_disconnect`, drains available
   response and transcription usage, closes the exact `AgentSession`, and
   shuts down.
6. Prove that the generation has at most one worker job and one provider
   session, no provider retry occurred, and no old-generation activity appears
   after the terminal boundary. Sarah's provider connection uses
   `maxRetry: 0` and `retryIntervalMs: 0`; a subsequent voice session must be a
   fresh admission with a different generation and provider projection.
7. If all terminal provider usage was durably delivered, require exact usage,
   charge, released hold, and settlement. Otherwise require
   `accounting_uncertain` with the full hold preserved, bind a complete provider
   export to the same provider-session digest, and follow the accounting
   reconciliation runbook before treating the row as terminal.
8. Remove the acceptance-only arming revision (return the effective environment
   to `false`), re-enable normal admissions, and project only public-safe
   digests, counts, amounts, and timings into the terminal matrix receipt.

The endpoint persists a one-generation directive; it never edits a firewall,
route, NetworkPolicy, Secret, LiveKit server, shared provider credential, or
Deployment. Owner acknowledgement and administrator authentication do not
relax target validation: the store accepts only the exact active alpha-cohort
generation and exact admitted provider digest. The same fail-closed rule
applies if any drill cannot identify its single session and terminal boundary
before the action.

Cancellation uses the exact gateway generation. Timeout waits for its
configured deadline. Hold exhaustion consumes only the target session's
admitted hold. Reconnect happens only after the previous terminal boundary and
uses a fresh admission. Never hand-author a passing row from expectations or
unit-test output.

## Issue close gate

Do not close #9284 until all are linked from #9282:

- exact deployed `origin/main` revision, bundle source base, and digests;
- canary deploy, three-path packaged Omega connectivity, and zero-residue
  destroy;
- production deployment and DNS/certificate observations;
- production direct UDP, TCP fallback, and TURN/TLS connectivity;
- overlapping private/community headless Sarah audio, transcription, ICE, and
  settlement receipt bound to the deployment revision;
- 20-room load with spare capacity and enforced caps;
- all eleven bounded failure drills;
- secret/log/raw-media/transcript scan;
- measured cost and active budget alerts;
- scoped rollback rehearsal;
- Sarah worker readiness from EP263-LK-03.

Until then, the honest status is “deployment automation and source contracts
ready; live production proof incomplete.”
