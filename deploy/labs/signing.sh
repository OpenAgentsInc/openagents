#!/bin/sh
# Grant each named Labs runtime only the signing permission its file links need.
set -eu
project=openagentsgemini
role_id=openagentsLabsFileSigner
role=projects/$project/roles/$role_id
[ "$#" -gt 0 ] || { echo 'Name at least one Labs runtime service account.' >&2; exit 2; }
scratch=$(openagents scratch)
work=$(mktemp -d "$scratch/labs-signing.XXXXXX")
if gcloud iam roles describe "$role_id" --project "$project" --format=json > "$work/role.json" 2> "$work/role.log"; then
    python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); assert set(r["includedPermissions"])=={"iam.serviceAccounts.signBlob"} and not r.get("deleted"), "Review the existing Labs signing role before changing it"' "$work/role.json"
else
    if ! grep -qi 'NOT_FOUND\|does not exist' "$work/role.log"; then cat "$work/role.log" >&2; exit 1; fi
    gcloud iam roles create "$role_id" --project "$project" \
        --title='Lab file links' --description='Sign short-lived Lab file download links.' \
        --permissions=iam.serviceAccounts.signBlob --stage=GA --quiet > "$work/create.log"
fi
for account in "$@"; do
    case "$account" in
        *@openagentsgemini.iam.gserviceaccount.com|157437760789-compute@developer.gserviceaccount.com) ;;
        *) echo 'Choose a runtime service account in openagentsgemini.' >&2; exit 2 ;;
    esac
    gcloud iam service-accounts add-iam-policy-binding "$account" --project "$project" \
        --member="serviceAccount:$account" --role="$role" --condition=None --quiet > "$work/binding.json"
    echo "Configured Lab file signing for $account."
done
