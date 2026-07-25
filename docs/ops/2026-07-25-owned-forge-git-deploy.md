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

The Filestore share is the only repository ref authority.
The service mounts the share at `/var/lib/forge/repositories`.
Push and fetch use the same bare repository on this share.

The GCS path is evidence and a mirror.
It is not ref authority.
Do not restore refs from GCS when Filestore is available.
Do not enable a June-era store as a second writer.

The service has a maximum of one Cloud Run instance.
Cloud Run mounts NFS without network file locks.
The one-instance limit keeps one stock Git write authority.

Cloudflare stays DNS-only.
The existing Google Cloud load balancer sends `/git` and `/git/*` to the
Forge Git service.
All other `openagents.com` paths continue to use the monolith.

## 3. Cost gate

`AUTHORITY.md` limits new recurring cloud cost to USD 100 each month.
A Basic HDD Filestore instance has a minimum capacity of 1 TiB.
Its current list cost can exceed that limit.

Do not apply this Terraform change until one of these conditions is true:

1. The owner gives an explicit budget grant for this resource.
2. An admitted, paid Filestore instance supplies compatible capacity.
3. A reviewed plan proves that the incremental recurring cost is at most
   USD 100 each month.

The production service cannot meet FORGE-02 acceptance before this gate opens.
Code review and a read-only Terraform plan do not create cloud cost.

## 4. Fixed values

Use these values:

```sh
export PROJECT="openagentsgemini"
export REGION="us-central1"
export ZONE="us-central1-a"
export SERVICE="forge-git"
export FILESTORE="forge-git-repositories"
export SHARE="forge_repositories"
export MOUNT="/var/lib/forge/repositories"
export NETWORK="default"
export SUBNETWORK="default"
export RUNTIME_SA="forge-git-runtime@openagentsgemini.iam.gserviceaccount.com"
export DATABASE_SECRET="openagents-monolith-database-url-prod"
export EVIDENCE_BUCKET="openagentsgemini-oa-artifacts"
export EVIDENCE_PREFIX="forge/git-packs"
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

1. One `BASIC_HDD` Filestore instance with a 1 TiB share.
2. One dedicated service account.
3. One Cloud Run service with Gen 2 execution and Direct VPC egress.
4. One writable NFS mount and a maximum of one service instance.
5. One restricted NFS egress rule.
6. One GCS object role for the evidence bucket.
7. One Secret Manager access role for the database URL.
8. One serverless NEG and one backend service.
9. One `/git` path rule on the current load balancer.

Stop if the plan changes the monolith default backend, DNS, or a certificate.
Stop if the plan replaces or deletes a data resource.

Do not apply the load-balancer route before the Git image is ready.
After the cost gate opens, create a separate bootstrap plan:

```sh
cd infra/prod
tofu plan -target=module.forge_git -out=forge-git-bootstrap.tfplan
tofu apply "forge-git-bootstrap.tfplan"
```

Terraform has `prevent_destroy` on the Filestore instance.
It also has deletion protection on the Cloud Run service.
The targeted bootstrap apply creates only the repository store and service
shell.
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
export FILESTORE_IP
FILESTORE_IP="$(
  gcloud filestore instances describe "$FILESTORE" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --format="value(networks.ipAddresses[0])"
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
  --add-volume="name=forge-repositories,type=nfs,location=${FILESTORE_IP}:/${SHARE}" \
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

## 9. Create a Filestore backup

Run this step after the marker push is durable:

```sh
export BACKUP="forge-git-drill-$(date -u +%Y%m%d-%H%M%S)"

gcloud filestore backups create "$BACKUP" \
  --project="$PROJECT" \
  --region="$REGION" \
  --instance="$FILESTORE" \
  --instance-zone="$ZONE" \
  --file-share="$SHARE" \
  --description="FORGE-02 new-instance restore drill"

gcloud filestore backups describe "$BACKUP" \
  --project="$PROJECT" \
  --region="$REGION" \
  --format="yaml(name,state,createTime,capacityGb,sourceInstance,sourceFileShare)"
```

Continue only when the backup state is `READY`.

## 10. Restore to a new instance

Use a new instance.
Do not restore over the production share:

```sh
export RESTORE_INSTANCE="forge-git-restore-$(date -u +%Y%m%d-%H%M%S)"

gcloud filestore instances create "$RESTORE_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --tier=BASIC_HDD \
  --network="name=${NETWORK}" \
  --file-share="name=${SHARE},capacity=1TB,source-backup=${BACKUP},source-backup-region=${REGION}"

export RESTORE_IP
RESTORE_IP="$(
  gcloud filestore instances describe "$RESTORE_INSTANCE" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --format="value(networks.ipAddresses[0])"
)"
```

The restored instance has temporary cost.
Complete the proof and cleanup in the same operator session.

## 11. Verify the restored repository through stock upload-pack

Deploy a temporary service from the same immutable image.
The service uses its dedicated `run.app` host and keeps the application
membership gate:

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
  --add-volume="name=forge-repositories,type=nfs,location=${RESTORE_IP}:/${SHARE}" \
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
The result proves that a new Filestore instance can serve the ref and objects
that the production push wrote.

## 12. Record the drill receipt

The receipt must contain:

1. The production Cloud Run revision.
2. The immutable image digest.
3. The backup name, state, and create time.
4. The restored Filestore instance name.
5. The production and restored commit IDs.
6. The clone, fetch, partial-clone, and `git fsck` results.
7. The start and finish times.
8. The cleanup result.

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

Delete only the exact temporary restore instance:

```sh
case "$RESTORE_INSTANCE" in
  forge-git-restore-*) ;;
  *) printf 'Refuse unexpected restore instance: %s\n' "$RESTORE_INSTANCE"; exit 1 ;;
esac

gcloud filestore instances delete "$RESTORE_INSTANCE" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --quiet
```

Keep the named backup according to the admitted backup policy.
Remove the local test directory after the receipt contains the required
public-safe values.
Unset all token variables.

## 14. Rollback

If clone or fetch fails after deployment, move `/git` traffic back to the
monolith only if the monolith has the compatible authenticated route.
Otherwise, return a service error and keep the repository share unchanged.

Do not move ref authority to GCS.
Do not start a second write service.
Do not delete the Filestore instance.
Deploy the prior immutable service image, then repeat sections 8 and 11.
