# LiveKit production read-only acceptance audit

Observed at `2026-07-31T12:02:25Z` against
`gke_openagentsgemini_us-central1_oa-livekit-prod` from repository revision
`612c6a0fd9c029519365d4cb165d14680cecf537`.

This is a public-safe audit record, not a production acceptance receipt. No
production resource was created, changed, restarted, drained, or deleted. No
Kubernetes Secret, provider credential, log payload, media, transcript, or
private acceptance capture was read.

## Observed controls

- The regional GKE cluster reported `RUNNING` on control-plane and node version
  `1.34.8-gke.1278000`.
- `livekit-server` and `sarah-livekit-agent` each reported three desired,
  updated, ready, and available replicas. Both images were digest-pinned and
  matched `infra/livekit/bundle.json`.
- Three ready SFU nodes were present, one in each of `us-central1-a`,
  `us-central1-b`, and `us-central1-c`. Each had an external candidate address.
  The server Deployment used host networking and the dedicated SFU node pool.
- The live ConfigMap enabled external-IP advertisement, WebRTC TCP on `7881`,
  the UDP range `50000-60000`, embedded TURN, and the expected TURN domain.
- The signaling and TURN load balancer addresses were provisioned. Public A
  records resolved to those respective addresses.
- The signaling endpoint returned HTTP 200 over TLS. Its Google-managed
  certificate reported `Active` and expires on 2026-10-28. The TURN certificate
  reported `Ready`, was trusted by the local TLS client, and expires on
  2026-10-28.
- Memorystore `oa-livekit-redis` reported `READY`, `STANDARD_HA`, 5 GiB,
  Redis 7.2, and server-authenticated transit encryption.
- Both server and worker PodDisruptionBudgets reported three healthy, two
  required, and one allowed disruption. Both HorizontalPodAutoscalers reported
  active metrics and desired three replicas.
- The three ExternalSecrets reported `Ready=True` with `SecretSynced`. Only
  metadata and conditions were queried.
- The `LiveKit production` Google budget was active at USD 5,000 per month,
  filtered to the project and `service=livekit`, with 50% and 80% current-spend
  thresholds, a 100% forecast threshold, and an enabled notification channel.
- Twelve enabled LiveKit alert policies were observed, including server scrape
  quorum, room and participant capacity, signaling availability and
  certificate expiry, TURN certificate expiry, Redis availability, memory and
  rejected connections, server CPU, workload errors, and participant join
  failures.

## Acceptance status

| Gate | Status | Evidence boundary |
| --- | --- | --- |
| Topology and inventory | Partially observed | Live infrastructure checks passed, but `livekit-connectivity-inventory.mjs` requires a current, independently verified packaged-Omega signature and launch attestation. None was supplied, so no inventory acceptance capture or receipt was emitted. |
| Direct UDP | Blocked | No current `openagents.livekit_connectivity_capture.v1` from packaged Omega with UDP available was present. DNS, TLS, and an earlier single selected ICE path do not prove direct UDP acceptance. |
| WebRTC TCP fallback | Blocked | No current packaged-Omega capture with UDP forcibly blocked was present. |
| TURN/TLS | Blocked | No current packaged-Omega capture with UDP and non-TLS TCP forcibly blocked was present. A trusted TURN certificate does not prove relayed media. |
| Load and capacity | Blocked | No current 20-room session capture, cap-plus-one refusal, bounded telemetry series, or settlement set was present. Running a load test would create production rooms and was outside this read-only audit. |
| Privacy scan | Blocked | The required same-window, complete eight-scope exports and private comparison inputs were not present. Secret metadata readiness is not a privacy scan. |
| Cost reconciliation | Blocked | Budget policy was observed, but the project exposed no Cloud Billing export table for the six required LiveKit categories, measured gross daily cost, or current gross forecast. The only listed BigQuery table was unrelated FinOps price-profile data. |
| Failure drills and rollback | Not run | These require the separate controlled-mutation interlock and were outside this read-only audit. |

The checked-in adapters therefore emitted no acceptance receipt. The existing
headless Sarah receipt proves a private and community session over one selected
ICE path; its stated limitation is one selected path per scenario, so it cannot
be promoted into the three-row forced connectivity matrix.

## Commands

The following read-only commands supplied the observations above:

```sh
kubectl config current-context
gcloud container clusters describe oa-livekit-prod --region us-central1 --project openagentsgemini --format='yaml(name,location,status,currentMasterVersion,currentNodeVersion,endpoint,network,subnetwork)'
kubectl -n livekit-system get deployment livekit-server sarah-livekit-agent -o json
kubectl get nodes -l openagents.com/livekit-workload=sfu -o json
kubectl -n livekit-system get ingress livekit-server -o json
kubectl -n livekit-system get service livekit-server-turn -o json
kubectl -n livekit-system get managedcertificate livekit-signal -o json
kubectl -n livekit-system get certificate livekit-turn -o json
kubectl -n livekit-system get configmap livekit-server -o json
kubectl -n livekit-system get hpa,pdb -o json
kubectl -n livekit-system get externalsecrets -o json
gcloud redis instances describe oa-livekit-redis --region us-central1 --project openagentsgemini --format='json(name,region,tier,memorySizeGb,state,redisVersion,transitEncryptionMode,authEnabled)'
dig +short livekit.openagents.com A
dig +short turn.livekit.openagents.com A
curl --silent --show-error --head --max-time 15 https://livekit.openagents.com/
openssl s_client -connect turn.livekit.openagents.com:443 -servername turn.livekit.openagents.com
gcloud billing budgets list --billing-account=<active-account> --format='json(displayName,amount,budgetFilter,thresholdRules,notificationsRule)'
gcloud beta monitoring channels list --project openagentsgemini --filter='displayName="LiveKit Production On-call"' --format='json(displayName,type,enabled,verificationStatus)'
gcloud monitoring policies list --project openagentsgemini --filter='displayName~LiveKit OR displayName~livekit' --format='json(name,displayName,enabled,conditions,notificationChannels)'
bq ls --project_id=openagentsgemini --format=prettyjson
bq ls --project_id=openagentsgemini --format=prettyjson psion_training_finops
```

The immutable bundle contracts and collector logic were checked without live
mutation:

```sh
node scripts/cloud/livekit-acceptance.mjs --phase connectivity --bundle infra/livekit/bundle.json
node scripts/cloud/livekit-acceptance.mjs --phase load --bundle infra/livekit/bundle.json
node scripts/cloud/livekit-acceptance.mjs --phase secret_scan --bundle infra/livekit/bundle.json
node scripts/cloud/livekit-acceptance.mjs --phase cost --bundle infra/livekit/bundle.json
node --test scripts/cloud/livekit-acceptance-probe.test.mjs scripts/cloud/livekit-production-acceptance.test.mjs scripts/cloud/livekit-privacy-scan.test.mjs scripts/cloud/livekit-privacy-scan-cli.test.mjs
```

The focused adapter suite passed 18 of 18 tests. Each acceptance contract
reported `liveProbeExecuted: false` and `receiptWritten: false`, as required for
dry-run mode.
