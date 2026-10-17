#!/bin/sh
# A keyless signer that may read only the production Lab file prefix.
set -eu
project=openagentsgemini
signer=openagents-labs-files@$project.iam.gserviceaccount.com
runtime=157437760789-compute@developer.gserviceaccount.com
bucket=openagentsgemini-web-chats-prod
scratch=$(openagents scratch)
work=$(mktemp -d "$scratch/labs-production-signing.XXXXXX")
if ! gcloud iam service-accounts describe "$signer" --project "$project" > "$work/account.json" 2> "$work/account.log"; then
    if ! grep -qi 'NOT_FOUND\|does not exist' "$work/account.log"; then cat "$work/account.log" >&2; exit 1; fi
    gcloud iam service-accounts create openagents-labs-files --project "$project" \
        --display-name='Lab file links' --quiet > "$work/create.log"
fi
gcloud iam service-accounts add-iam-policy-binding "$signer" --project "$project" \
    --member="serviceAccount:$runtime" --role="projects/$project/roles/openagentsLabsFileSigner" \
    --condition=None --quiet > "$work/signing.json"
attempt=0
while ! gcloud storage buckets add-iam-policy-binding "gs://$bucket" --project "$project" \
    --member="serviceAccount:$signer" --role=roles/storage.objectViewer \
    --condition="title=LabsFileLinks,expression=resource.name.startsWith('projects/_/buckets/$bucket/objects/labs/'),description=Read only Lab files for signed downloads" \
    --quiet > "$work/reading.json" 2> "$work/reading.log"; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 6 ] || ! grep -q 'does not exist' "$work/reading.log"; then
        cat "$work/reading.log" >&2
        exit 1
    fi
    sleep 5
done
echo "Configured $signer for production Lab file links."
