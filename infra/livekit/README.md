# OpenAgents LiveKit runtime bundle

This directory is the pinned Kubernetes runtime source for the first
self-hosted Sarah voice cohort. It renders the LiveKit server and Sarah agent
worker into one production manifest for the regional GKE Standard cluster. It
does not deploy recording, egress, ingress workers, SIP, or TURN/UDP.

The bundle has three authority boundaries:

- OpenAgents admission creates every room and participant grant. LiveKit
  rejects implicit room creation.
- Kubernetes carries only reference material from Google Secret Manager.
  This repository contains no LiveKit key, Redis password, private key, or
  secret payload.
- LiveKit carries media. It does not carry command, transcript, accounting,
  settlement, or OpenAgents identity authority.

## Pinned source

`pins.lock.json` pins the audited LiveKit Helm source archive and the exact
multi-platform server image index. `addons.lock.json` pins the cert-manager and
External Secrets chart archives and every controller, webhook, solver, and
startup image expected by the resources in this bundle. Google
Managed Service for Prometheus is a GKE-managed add-on; its `v1` resource API
is pinned here, while the cluster's STABLE release channel owns the collector
binary version.

`bundle.json.sourceBaseRevision` records the repository revision this source
was designed against. It is not the deployment revision and cannot
self-reference the commit that contains the bundle. Deployment automation and
receipts must record the actual `git rev-parse HEAD` separately.

The upstream Helm chart enables `--disable-strict-config` and mixes chart-only
TURN service fields into the server ConfigMap. The post-render step replaces
that ConfigMap with `production/livekit.yaml`, binds its digest to the
Deployment, and removes the strict-parser opt-out. `verify.sh` refuses a render
that restores it, uses an unpinned image, exposes TURN/UDP, or loses the
production placement and availability controls.

## Render

Requirements: Node.js 24, Helm 3, `kubectl` with Kustomize support, `curl`,
`tar`, and `shasum`.

```sh
infra/livekit/verify.sh
infra/livekit/render.sh --output /tmp/openagents-livekit-render
```

The second command writes only
`/tmp/openagents-livekit-render/livekit-production.yaml`. It fetches the exact
Git archive, verifies its SHA-256 digest, renders chart `1.11.0`, applies the
post-render policy, and appends the first-party resources.

The production runner validates the rendered inventory before it installs
addons or applies the runtime. Namespaced resources must declare an admitted
`cert-manager` or `livekit-system` namespace for their exact API version and
kind; admitted cluster-scoped resources must omit `metadata.namespace`, and
the bundle may create only the `livekit-system` Namespace. The server-side
apply does not supply a default namespace, so it cannot redirect a
multi-namespace manifest.

Production acceptance uses
`scripts/cloud/livekit-production-acceptance.mjs`. Its dry-run validates a
closed, ordered private probe plan without executing commands. Apply mode
spawns exact argv without a shell, hashes private command evidence into opaque
refs, writes the aggregate private observation outside the repository, and
emits a public receipt only after `livekit-ops-policy.mjs` accepts the complete
phase. Failure drills and rollback require a separate controlled-mutation
interlock in addition to the owner cost gate.

Use `scripts/cloud/livekit-production-plan.mjs` and
`scripts/cloud/livekit-acceptance-probe.mjs` for the non-destructive
connectivity, load, secret-scan, and cost phases. The plan generator refuses
drill and rollback phases. The probe adapter derives acceptance aggregates
from revision-bound packaged-client captures, session/telemetry samples, the
existing eight-scope privacy scanner output, and categorized gross billing
exports plus active budget policy. It does not query production by itself or
accept precomputed load/cost pass values.

`scripts/cloud/livekit-connectivity-inventory.mjs` is the narrow live
exception: with the owner gate it performs only read-only Kubernetes
inventory, public DNS resolution, and TLS authorization checks. It combines
those observations with a private packaged-Omega signature/launch attestation
and emits a topology-redacted private preflight capture outside the repository.

## Required cluster dependencies

Before applying the rendered manifest:

1. provision the exact GKE, node-pool, Redis, address, firewall, Workload
   Identity, and IAM resources named by `bundle.json`;
2. install the pinned cert-manager and External Secrets charts from
   `addons.lock.json`;
3. enable Google Managed Service for Prometheus managed collection;
4. grant the three bundle secret-reader service accounts only their documented
   Google Secret Manager containers;
5. add current secret versions to `oa-livekit-prod-server-keys`,
   `oa-livekit-prod-redis-auth`, `oa-livekit-prod-cloudflare-dns`,
   `oa-livekit-prod-openai-api-key`, and
   `oa-livekit-prod-sarah-control-root` in Google Secret Manager; and
6. map DNS-only Cloudflare records to the Google addresses after their
   corresponding Google load balancers are healthy.

`oa-livekit-prod-server-keys` is a structured secret with `api_key`,
`api_secret`, and `keys_yaml` properties. The runtime bundle consumes only the
LiveKit key-file form in `keys_yaml`; the separate fields support bounded
server-side minting without putting a key in Helm. `oa-livekit-prod-redis-auth`
has `host` and `ca_cert` properties. Its retained name identifies the Redis
connection-material boundary; it does not imply that Memorystore AUTH is
enabled. The Google provider exposes a computed AUTH credential in Terraform
state when AUTH is enabled, which violates the repository credential-state
boundary. Production therefore uses Standard Tier HA, in-transit TLS, and a
dedicated private VPC/Private Service Access path with AUTH disabled. The
private network and verified TLS certificate are both required; neither is
equivalent to application-layer authentication.
`oa-livekit-prod-cloudflare-dns` has only an `api_token` property for a
zone-scoped DNS-01 token. External Secrets materializes
`livekit-server-keys`, `livekit-redis-auth`, and `cloudflare-dns-token`.
The media and certificate secret readers use disjoint Workload Identity
service accounts. The Sarah reader can access only the server key container,
the copied OpenAI key container, and the Sarah control-root container. The
worker KSA maps to its own otherwise unprivileged GSA and has no database,
storage, signing, or Secret Manager IAM role.

The `livekit-public-dns01` ClusterIssuer uses that DNS-only token. The TURN
`Certificate` writes `livekit-turn-tls`. LiveKit terminates TURN/TLS on `5349`;
the dedicated external passthrough service publishes TCP `443`. TURN/UDP is
deliberately absent.

## Placement and disruption

LiveKit uses `hostNetwork` because clients must reach each SFU node's public
ICE address. The chart is constrained to the tainted
`oa-livekit-prod-sfu` pool. Required hostname anti-affinity enforces one SFU
pod per node, zone spread avoids concentration, the PDB preserves two ready
pods, and rolling updates permit no planned unavailability. The signaling
backend drains for 1,800 seconds, matching the admitted maximum room lifetime,
while the pod retains the longer five-hour termination grace for LiveKit's
own room drain. A room is still owned by one SFU and cannot migrate after
abrupt node loss.

Kubernetes NetworkPolicy is not used for the SFU pod. GKE does not provide a
portable NetworkPolicy boundary for `hostNetwork` media, and an apparent
deny-list would be misleading because RTC and TURN need arbitrary client
addresses. The dedicated node pool, GCP firewall targets, load-balancer
backends, and service-account boundaries are the applicable controls.

The Sarah worker is a non-host-network application pod. It runs three replicas
on distinct `oa-livekit-prod-app` nodes across three zones. It has deny-all
pod ingress, no Service, digest-only execution, a read-only root filesystem,
CPU and memory bounds, a three-to-four replica HPA, and a PDB that preserves two
ready replicas. LiveKit Agents JS production health is probed on port `8081`;
the endpoint is ready only while the worker has a live server WebSocket.

### Private Nostr signer projection configuration

Community-room Nostr projection uses the private Cloud Run signer and the
owned relay. The worker obtains a Google identity token from the metadata
server for the exact Cloud Run audience; it never receives
`principal.sarah` key material.

After `tofu apply` creates the signer, create the public runtime ConfigMap
from the Terraform output and the admitted relay endpoint:

```sh
SIGNER_AUDIENCE="$(tofu -chdir=../livekit-production output -json sarah_nostr_signer | jq -r .audience)"
kubectl --context=oa-livekit-prod -n livekit-system create configmap sarah-nostr-projection \
  --from-literal=SARAH_NOSTR_SIGNER_URL="${SIGNER_AUDIENCE}" \
  --from-literal=SARAH_NOSTR_SIGNER_AUDIENCE="${SIGNER_AUDIENCE}" \
  --from-literal=SARAH_NOSTR_EXPECTED_PUBKEY="<principal.sarah lowercase hex pubkey>" \
  --from-literal=SARAH_NOSTR_RELAY_URL="wss://relay.openagents.com" \
  --dry-run=client -o yaml | kubectl --context=oa-livekit-prod apply -f -
```

The worker Deployment carries this ConfigMap as an optional disabled-state
reference, but a community job fails closed if any of the four values is
absent. The signer URL and audience must remain the exact HTTPS Cloud Run URI;
the relay must use WSS. Set the same public Sarah key on the API as
`SARAH_NOSTR_EXPECTED_PUBKEY` and set the admitted community media-key digest
as `SARAH_LIVEKIT_E2EE_KEY_REVISION`; malformed or missing values prevent the
API from returning a presence lease. Restart both revisions and wait for
rollout status before dispatching the acceptance room.

## Sarah worker image pin

Source carries an explicit all-zero digest placeholder and
`workerImage.pinState=build_required`. Dry-run validation accepts that state so
infrastructure can create the new secret container. A live runtime apply
refuses it.

After Cloud Build publishes the Dockerfile from the repository root:

1. read the immutable Artifact Registry digest without pulling secret data;
2. replace the exact image in
   `production/resources/sarah-agent-runtime.yaml`;
3. set the same reference and digest in `bundle.json.workerImage`, then set
   `pinState` to `pinned`;
4. update the changed manifest digests in `bundle.json`;
5. render once and update `bundle.json.renderedManifestDigest`; and
6. run `infra/livekit/verify.sh` and the LiveKit policy tests before the
   production runtime apply.

The admitted repository is
`us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent`.
No mutable tag is accepted.

## Capacity limits

The limits in `bundle.json` are admission limits, not source-only capacity
claims. Twenty concurrent Sarah rooms become admissible only after live load,
failure, TURN, and settlement acceptance. The server itself limits each room
to three participants, permits Opus only, and refuses automatic room creation.
The OpenAgents control plane owns per-owner, per-community, idle, and maximum
lifetime enforcement.

## Canary seam

`canary/livekit.yaml` is the strict server configuration for the disposable
single-node GCE connectivity canary. The canary launcher supplies
`LIVEKIT_KEYS` and the TURN certificate paths at runtime. Redis is deliberately
absent because the canary is not a distributed deployment. It uses the same
server image digest and policy but is not a production availability target.
