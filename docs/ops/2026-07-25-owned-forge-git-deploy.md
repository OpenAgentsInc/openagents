# Owned Forge Git deploy and recovery runbook (FORGE-02)

- Date: 2026-07-25
- Class: procedure
- OpenAgents issue: [OpenAgentsInc/openagents#9244](https://github.com/OpenAgentsInc/openagents/issues/9244)
- Parent issue: [OpenAgentsInc/openagents#9242](https://github.com/OpenAgentsInc/openagents/issues/9242)
- Design authority: [2026-07-25-nostr-git-forge-invite-only-analysis.md](../fable/2026-07-25-nostr-git-forge-invite-only-analysis.md)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: operator runbook

## 1. Purpose

This runbook tells an operator how to deploy and verify the owned Forge Git
service.

The service runs stock `git upload-pack` and `git receive-pack`.
Effect owns request routing, authentication, admission, and operation results.
Do not add a TypeScript packfile engine.
Do not use CGI.

## 2. Authority

The `forge-git-repositories` persistent disk is the only repository ref
authority.
The `forge-git-nfs` GCE host exports this disk through NFS.
The service mounts the share at `/var/lib/forge/repositories`.
Push and fetch use the same bare repository on this share.

The GCS path is evidence and a mirror.
It is not ref authority.
Do not restore refs from GCS when the repository disk is available.
Do not enable a June-era store as a second writer.

The service has a maximum of one Cloud Run instance.
Cloud Run mounts NFS without network file locks.
The one-instance limit keeps one stock Git write authority.

Cloudflare stays DNS-only.
The existing Google Cloud load balancer sends `/git` and `/git/*` to the
Forge Git service.
All other `openagents.com` paths continue to use the monolith.

Security and availability limits:

1. The NFS traffic does not have transport encryption.
2. The dedicated subnet and firewall isolate this traffic.
3. The NFS export maps all clients to UID and GID 1000.
4. The NFS host and data disk are zonal.
5. A zone or host failure stops Git until recovery completes.
6. Daily snapshots and the restore drill give the recovery path.
7. Cloud Run does not support NFS network locks.
8. The one-instance limit is a required write-safety control.

## 3. Cost estimate

`AUTHORITY.md` limits new recurring cloud cost to USD 100 each month.
Use 730 hours for this conservative estimate:

| Resource | Calculation | Monthly estimate |
| --- | --- | ---: |
| `e2-small` NFS host | USD 0.016752855 each hour | USD 12.23 |
| 100 GiB balanced data disk | USD 0.000136986 per GiB-hour | USD 10.00 |
| 10 GiB balanced boot disk | USD 0.000136986 per GiB-hour | USD 1.00 |
| Four full 100 GiB regional snapshots | USD 0.000068493 per GiB-hour | USD 20.00 |
| Public NAT, one VM and one address | USD 0.0014 + USD 0.005 each hour | USD 4.67 |
| Fixed subtotal | | USD 47.90 |

Cloud Run use, logs, NAT data, and network egress are variable.
The fixed subtotal leaves USD 52.10 for these variable costs.
Stop before apply if the reviewed estimate can exceed USD 100 each month.
Record actual cost after the first complete month.
Use the current Google Cloud VM, disk, snapshot, and NAT pricing pages for each
review.

## 4. Fixed values

Use these values:

```sh
export PROJECT="openagentsgemini"
export REGION="us-central1"
export ZONE="us-central1-a"
export SERVICE="forge-git"
export NFS_INSTANCE="forge-git-nfs"
export REPOSITORY_DISK="forge-git-repositories"
export NFS_EXPORT="/srv/forge/repositories"
export MOUNT="/var/lib/forge/repositories"
export NETWORK="default"
export SUBNETWORK="forge-git"
export SUBNETWORK_CIDR="10.42.24.0/26"
export NFS_SA="forge-git-nfs@openagentsgemini.iam.gserviceaccount.com"
export RUNTIME_SA="forge-git-runtime@openagentsgemini.iam.gserviceaccount.com"
export DATABASE_SECRET="openagents-monolith-database-url-prod"
export DATABASE_INSTANCE="openagentsgemini:us-central1:khala-sync-pg"
export EVIDENCE_BUCKET="openagentsgemini-oa-artifacts"
export EVIDENCE_PREFIX="forge/git-packs"
export OPENAGENTS_CHECKOUT="$(git rev-parse --show-toplevel)"
```

Do not print a database URL or a Forge Git token.
Do not enable shell command tracing while a token is present.

## 5. Terraform plan and apply

Run the plan from a clean checkout of current `main`:

```sh
cd infra
make init
make validate
make plan
```

Review these planned resources:

1. One dedicated `/26` Direct VPC subnet.
2. One `e2-small` NFS host with no external address.
3. One 100 GiB balanced persistent disk with `prevent_destroy`.
4. One daily snapshot schedule with three-day retention.
5. One restricted NFS ingress rule and one IAP SSH rule.
6. One Public NAT gateway for NFS host security updates.
7. One Cloud Run service with one maximum instance.
8. One writable NFS mount and stable service identities.
9. One Cloud SQL socket mount and client role.
10. One serverless NEG, backend, and `/git` path rule.

Stop if the plan changes the monolith default backend, DNS, or a certificate.
Stop if the plan replaces or deletes a data resource.

Do not apply the load-balancer route before the Git image is ready.
After the cost review passes, create the NFS host first:

```sh
cd infra/prod
tofu plan \
  -target=module.forge_git.google_compute_instance.nfs \
  -out=forge-git-nfs-bootstrap.tfplan
tofu apply "forge-git-nfs-bootstrap.tfplan"

gcloud compute ssh "$NFS_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --tunnel-through-iap \
  --command="sudo systemctl is-active nfs-kernel-server && findmnt ${NFS_EXPORT}"

tofu plan -target=module.forge_git -out=forge-git-service-bootstrap.tfplan
tofu apply "forge-git-service-bootstrap.tfplan"
```

Terraform has `prevent_destroy` on the repository disk and subnet.
It also has deletion protection on the Cloud Run service.
The first bootstrap creates the network, disk, NAT, and NFS host.
The second bootstrap creates the service shell after NFS is ready.
It does not move `/git` traffic from the monolith.

## 6. Deploy the service revision

Build and push the service image through the repository release process.
Use an immutable image digest:

```sh
export IMAGE="us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/forge-git@sha256:<digest>"
```

Deploy the image.
Repeat all storage and network flags on each deploy:

```sh
export NFS_IP
NFS_IP="$(
  gcloud compute instances describe "$NFS_INSTANCE" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --format="value(networkInterfaces[0].networkIP)"
)"

gcloud run deploy "$SERVICE" \
  --project="$PROJECT" \
  --region="$REGION" \
  --image="$IMAGE" \
  --execution-environment=gen2 \
  --service-account="$RUNTIME_SA" \
  --ingress=internal-and-cloud-load-balancing \
  --allow-unauthenticated \
  --min=0 \
  --max=1 \
  --concurrency=80 \
  --timeout=3600 \
  --network="$NETWORK" \
  --subnet="$SUBNETWORK" \
  --vpc-egress=private-ranges-only \
  --network-tags=forge-git \
  --add-cloudsql-instances="$DATABASE_INSTANCE" \
  --add-volume="name=forge-repositories,type=nfs,location=${NFS_IP}:${NFS_EXPORT}" \
  --add-volume-mount="volume=forge-repositories,mount-path=${MOUNT}" \
  --update-env-vars="FORGE_GIT_REPOSITORY_ROOT=${MOUNT},OA_INFRA_GCS_BUCKET=${EVIDENCE_BUCKET},OA_INFRA_GCS_PREFIX=${EVIDENCE_PREFIX}" \
  --set-secrets="FORGE_GIT_DATABASE_URL=${DATABASE_SECRET}:latest"
```

`--allow-unauthenticated` lets the load balancer invoke the Effect
application.
The application must reject each Git request that has no admitted membership
credential.
The `run.app` endpoint does not accept external traffic because the ingress
setting restricts it.

Confirm the stable service controls:

```sh
gcloud run services describe "$SERVICE" \
  --project="$PROJECT" \
  --region="$REGION" \
  --format="yaml(spec.template.metadata.annotations,spec.template.spec.serviceAccountName,spec.template.spec.volumes,spec.template.spec.containerConcurrency,spec.template.spec.timeoutSeconds)"
```

Create and review a new full plan.
This plan must add the Forge Git NEG, backend, and path rule:

```sh
cd infra/prod
tofu plan -out=forge-git-cutover.tfplan
tofu apply "forge-git-cutover.tfplan"
```

Confirm the route:

```sh
gcloud compute url-maps describe openagents-url-map \
  --project="$PROJECT" \
  --global \
  --format="yaml(pathMatchers)"
```

The output must show the NFS mount, Direct VPC configuration, one maximum
instance, and the `/git` path rule.

## 7. Prepare an authentication-safe Git shell

Create an invite-only test repository through the admitted Forge create
action.
Set its path:

```sh
export FORGE_OWNER="<owner>"
export FORGE_REPO="<repository>"
export FORGE_URL="https://openagents.com/git/${FORGE_OWNER}/${FORGE_REPO}.git"
```

Read a short-lived token without terminal output:

```sh
read -r -s -p "Forge Git token: " FORGE_GIT_TOKEN
printf '\n'
export GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0="http.extraHeader"
export GIT_CONFIG_VALUE_0="Authorization: Bearer ${FORGE_GIT_TOKEN}"
unset FORGE_GIT_TOKEN
```

Do not put the token in a URL or a command argument.
Unset the Git variables when the test ends:

```sh
unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0
```

FORGE-02 validates the token hash, tenant, repository, expiry, and Git scope.
It does not validate live `forge_actor_bindings` membership or tombstones.
Issue #9246 owns that FORGE-04 integration and its revoked-member tests.
Do not use a FORGE-02 drill as full identity or revocation acceptance.

## 8. Verify clone, push, fetch, and partial clone

Use a temporary directory:

```sh
export FORGE_DRILL_DIR
FORGE_DRILL_DIR="$(mktemp -d)"
```

Clone the repository:

```sh
git clone "$FORGE_URL" "$FORGE_DRILL_DIR/clone"
```

Create and push a marker commit:

```sh
cd "$FORGE_DRILL_DIR/clone"
git config user.name "OpenAgents Forge drill"
git config user.email "forge-drill@openagents.com"
git commit --allow-empty -m "FORGE-02 round-trip drill"
git push origin HEAD:refs/heads/forge-02-drill
export EXPECTED_COMMIT
EXPECTED_COMMIT="$(git rev-parse HEAD)"
```

Fetch the ref into a new object database:

```sh
git clone --no-checkout "$FORGE_URL" "$FORGE_DRILL_DIR/fetch"
git -C "$FORGE_DRILL_DIR/fetch" fetch origin \
  refs/heads/forge-02-drill:refs/remotes/origin/forge-02-drill
test "$(
  git -C "$FORGE_DRILL_DIR/fetch" rev-parse refs/remotes/origin/forge-02-drill
)" = "$EXPECTED_COMMIT"
```

Verify partial-clone support:

```sh
GIT_TRACE_PACKET=1 git clone \
  --filter=blob:none \
  --no-checkout \
  "$FORGE_URL" \
  "$FORGE_DRILL_DIR/partial" \
  2>"$FORGE_DRILL_DIR/partial-clone.trace"

grep "fetch=.*filter" "$FORGE_DRILL_DIR/partial-clone.trace"
git -C "$FORGE_DRILL_DIR/partial" config --get remote.origin.promisor
git -C "$FORGE_DRILL_DIR/partial" fsck --full
```

The upload-pack advertisement must include filter support.
The repository configuration must also permit reachable and tip SHA-1 wants.
Record the service revision, repository path, expected commit, and test times.
Do not record the token or full request headers.

## 9. Create a disk snapshot

Run this step after the marker push is durable.
Freeze the exported file system only for the snapshot request:

```sh
export SNAPSHOT="forge-git-drill-$(date -u +%Y%m%d-%H%M%S)"
export NFS_FROZEN=0
thaw_nfs() {
  if [[ "$NFS_FROZEN" = "1" ]]; then
    gcloud compute ssh "$NFS_INSTANCE" \
      --project="$PROJECT" \
      --zone="$ZONE" \
      --tunnel-through-iap \
      --command="sudo fsfreeze -u ${NFS_EXPORT}"
  fi
}
trap thaw_nfs EXIT

gcloud compute ssh "$NFS_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --tunnel-through-iap \
  --command="sudo sync && sudo fsfreeze -f ${NFS_EXPORT}"
NFS_FROZEN=1

gcloud compute snapshots create "$SNAPSHOT" \
  --project="$PROJECT" \
  --source-disk="$REPOSITORY_DISK" \
  --source-disk-zone="$ZONE" \
  --storage-location="$REGION" \
  --description="FORGE-02 new-disk restore drill" \
  --async

thaw_nfs
NFS_FROZEN=0
trap - EXIT

while true; do
  SNAPSHOT_STATUS="$(
    gcloud compute snapshots describe "$SNAPSHOT" \
      --project="$PROJECT" \
      --format="value(status)"
  )"
  case "$SNAPSHOT_STATUS" in
    READY) break ;;
    FAILED) printf 'Snapshot failed: %s\n' "$SNAPSHOT"; exit 1 ;;
    *) sleep 10 ;;
  esac
done

gcloud compute snapshots describe "$SNAPSHOT" \
  --project="$PROJECT" \
  --format="yaml(name,status,creationTimestamp,diskSizeGb,sourceDisk)"
```

Wait until the snapshot status is `READY`.
The freeze causes a short Git service pause.
Stop the drill if the thaw command fails.

## 10. Restore to a new disk and verifier

Do not restore over the production disk:

```sh
export RESTORE_DISK="forge-git-restore-$(date -u +%Y%m%d-%H%M%S)"
export RESTORE_INSTANCE="${RESTORE_DISK}-nfs"

gcloud compute disks create "$RESTORE_DISK" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --type=pd-balanced \
  --source-snapshot="$SNAPSHOT"

gcloud compute instances create "$RESTORE_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --machine-type=e2-small \
  --subnet="$SUBNETWORK" \
  --private-network-ip="10.42.24.3" \
  --no-address \
  --service-account="$NFS_SA" \
  --scopes=logging-write \
  --tags=forge-git-nfs \
  --shielded-secure-boot \
  --shielded-vtpm \
  --shielded-integrity-monitoring \
  --metadata="enable-oslogin=TRUE,block-project-ssh-keys=TRUE,repository-disk-name=${RESTORE_DISK},export-path=${NFS_EXPORT},allowed-cidr=${SUBNETWORK_CIDR}" \
  --metadata-from-file="startup-script=${OPENAGENTS_CHECKOUT}/infra/modules/forge-git-service/nfs-startup.sh" \
  --disk="name=${RESTORE_DISK},device-name=${RESTORE_DISK},mode=rw,boot=no"

until gcloud compute ssh "$RESTORE_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --tunnel-through-iap \
  --command="sudo systemctl is-active --quiet nfs-kernel-server"; do
  sleep 5
done

export RESTORE_IP
RESTORE_IP="$(
  gcloud compute instances describe "$RESTORE_INSTANCE" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --format="value(networkInterfaces[0].networkIP)"
)"
```

The verifier has temporary cost.
Complete proof and cleanup in the same operator session.

## 11. Verify the restored repository through stock upload-pack

Deploy a temporary service from the same immutable image.
It uses a dedicated `run.app` host and the application membership gate:

```sh
export RESTORE_SERVICE="forge-git-restore-drill"

gcloud run deploy "$RESTORE_SERVICE" \
  --project="$PROJECT" \
  --region="$REGION" \
  --image="$IMAGE" \
  --execution-environment=gen2 \
  --service-account="$RUNTIME_SA" \
  --ingress=all \
  --allow-unauthenticated \
  --min=0 \
  --max=1 \
  --concurrency=1 \
  --timeout=3600 \
  --network="$NETWORK" \
  --subnet="$SUBNETWORK" \
  --vpc-egress=private-ranges-only \
  --network-tags=forge-git \
  --add-cloudsql-instances="$DATABASE_INSTANCE" \
  --add-volume="name=forge-repositories,type=nfs,location=${RESTORE_IP}:${NFS_EXPORT}" \
  --add-volume-mount="volume=forge-repositories,mount-path=${MOUNT}" \
  --update-env-vars="FORGE_GIT_REPOSITORY_ROOT=${MOUNT},OA_INFRA_GCS_BUCKET=${EVIDENCE_BUCKET},OA_INFRA_GCS_PREFIX=${EVIDENCE_PREFIX}" \
  --set-secrets="FORGE_GIT_DATABASE_URL=${DATABASE_SECRET}:latest"

export RESTORE_ORIGIN
RESTORE_ORIGIN="$(
  gcloud run services describe "$RESTORE_SERVICE" \
    --project="$PROJECT" \
    --region="$REGION" \
    --format="value(status.url)"
)"
```

Read a new short-lived membership token as shown in section 7.
Clone from the restored service:

```sh
git clone \
  "${RESTORE_ORIGIN}/git/${FORGE_OWNER}/${FORGE_REPO}.git" \
  "$FORGE_DRILL_DIR/restored"

test "$(
  git -C "$FORGE_DRILL_DIR/restored" rev-parse refs/remotes/origin/forge-02-drill
)" = "$EXPECTED_COMMIT"
git -C "$FORGE_DRILL_DIR/restored" fsck --full
```

Do not push to the restore service.
The result proves that a new disk and NFS verifier can serve the ref and
objects that the production push wrote.

## 12. Record the drill receipt

The receipt must contain:

1. The production Cloud Run revision.
2. The immutable image digest.
3. The snapshot name, state, and create time.
4. The restored disk and verifier names.
5. The production and restored commit IDs.
6. The clone, fetch, partial-clone, and `git fsck` results.
7. The start and finish times.
8. The cleanup result.
9. The FORGE-04 identity and revocation limitation.

Put public-safe evidence in the FORGE-02 receipt.
Do not put a token, database URL, private IP address, or request header in the
receipt.

## 13. Cleanup

Remove the temporary service first:

```sh
gcloud run services delete "$RESTORE_SERVICE" \
  --project="$PROJECT" \
  --region="$REGION" \
  --quiet
```

Delete only the exact temporary verifier and disk:

```sh
case "$RESTORE_INSTANCE" in
  forge-git-restore-*-nfs) ;;
  *) printf 'Refuse unexpected restore instance: %s\n' "$RESTORE_INSTANCE"; exit 1 ;;
esac

gcloud compute instances delete "$RESTORE_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --quiet

case "$RESTORE_DISK" in
  forge-git-restore-*) ;;
  *) printf 'Refuse unexpected restore disk: %s\n' "$RESTORE_DISK"; exit 1 ;;
esac

gcloud compute disks delete "$RESTORE_DISK" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --quiet
```

Keep the named snapshot according to the admitted backup policy.
Remove the local test directory after the receipt contains the required
public-safe values.
Unset all token variables.

## 14. Rollback

If clone or fetch fails after deployment, move `/git` traffic back to the
monolith only if the monolith has the compatible authenticated route.
Otherwise, return a service error and keep the repository share unchanged.

Do not move ref authority to GCS.
Do not start a second write service.
Do not delete the repository disk.
Deploy the prior immutable service image, then repeat sections 8 and 11.
