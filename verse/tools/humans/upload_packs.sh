#!/usr/bin/env bash
# Puts private human packs where openagents.com's broker serves them (coax/web/api/verse_packs.c,
# docs/verse/metahuman-support.md, "Sending packs to browsers"). Owner-run, on the machine that
# builds them; nothing here goes in git and nothing secret is printed.
#
#   verse/tools/humans/upload_packs.sh                  every pack in ~/.openagents/verse/private-build/web/
#   verse/tools/humans/upload_packs.sh human human-robe  those packs only
#   VERSE_PACKS_DIR=DIR ...                             packs from DIR instead
#
# For each NAME.pack: its SHA-256, then gzip -9, then AES-256-CTR under a fresh random key and iv,
# stored as gs://openagentsgemini-verse-private-assets/packs/<sha256> (Cache-Control no-store; the
# bucket enforces public access prevention). The manifest, Secret Manager verse-private-packs, gets
# the pack's line (name, digest, sizes, object, key, iv) in a new version, with the session master
# made once and kept. The API reads the new version within a minute; no redeploy.
# A pack already stored under its digest is not uploaded again (its key stays the manifest's).
set -euo pipefail
dir="${VERSE_PACKS_DIR:-$HOME/.openagents/verse/private-build/web}"
bucket="${VERSE_PACKS_BUCKET:-openagentsgemini-verse-private-assets}"
project=openagentsgemini secret=verse-private-packs
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
umask 077
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT

names=("$@")
if [[ ${#names[@]} -eq 0 ]]; then for f in "$dir"/*.pack; do n="$(basename "$f" .pack)"; names+=("$n"); done; fi
# The manifest as it stands (or a new one with its master).
if gcloud secrets describe "$secret" --project "$project" >/dev/null 2>&1; then
  gcloud secrets versions access latest --secret "$secret" --project "$project" > "$work/manifest"
else
  gcloud secrets create "$secret" --project "$project" --replication-policy automatic \
    --labels purpose=verse-private-packs >/dev/null
  # oa-api and oa-api-staging run as the project's compute account: they read the manifest.
  gcloud secrets add-iam-policy-binding "$secret" --project "$project" --role roles/secretmanager.secretAccessor \
    --member "serviceAccount:157437760789-compute@developer.gserviceaccount.com" >/dev/null
  : > "$work/manifest"
fi
grep -q '^master ' "$work/manifest" || { printf 'master %s\n' "$(openssl rand -hex 32)" | cat - "$work/manifest" > "$work/m2"; mv "$work/m2" "$work/manifest"; }

for name in "${names[@]}"; do
  [[ "$name" =~ ^[a-z0-9-]{1,64}$ ]] || { echo "upload_packs: $name: a pack's name is a-z, 0-9 and -" >&2; exit 1; }
  src="$dir/$name.pack"
  [[ -f "$src" ]] || { echo "upload_packs: no $src" >&2; exit 1; }
  [[ "$(head -c 4 "$src")" == VPK1 ]] || { echo "upload_packs: $src is not a VPK1 pack" >&2; exit 1; }
  sha="$(shasum -a 256 "$src" | cut -c1-64)"
  plain="$(wc -c < "$src" | tr -d ' ')"
  old="$(awk -v n="$name" -v s="$sha" '$1 == "pack" && $2 == n && $3 == s' "$work/manifest")"
  if [[ -n "$old" ]] && gcloud storage objects describe "gs://$bucket/packs/$sha" --format 'value(size)' >/dev/null 2>&1; then
    echo "upload_packs: $name is stored already (packs/${sha:0:12}...)"; continue
  fi
  key="$(openssl rand -hex 32)" iv="$(openssl rand -hex 8)0000000000000000"   # the low 8 bytes count blocks (WebCrypto's 64-bit counter)
  gzip -9 -n -c "$src" > "$work/gz"
  openssl enc -aes-256-ctr -K "$key" -iv "$iv" -in "$work/gz" -out "$work/object"
  stored="$(wc -c < "$work/object" | tr -d ' ')"
  gcloud storage cp "$work/object" "gs://$bucket/packs/$sha" --cache-control no-store \
    --content-type application/octet-stream --custom-metadata no-training=true,format=vpk1-gzip-aes256ctr --quiet 2>/dev/null
  awk -v n="$name" '!($1 == "pack" && $2 == n)' "$work/manifest" > "$work/m2"
  printf 'pack %s %s %s %s packs/%s %s %s\n' "$name" "$sha" "$plain" "$stored" "$sha" "$key" "$iv" >> "$work/m2"
  mv "$work/m2" "$work/manifest"
  echo "upload_packs: $name: $plain bytes, $stored stored (gzip, encrypted) as packs/${sha:0:12}..."
  rm -f "$work/gz" "$work/object"
done
gcloud secrets versions add "$secret" --project "$project" --data-file "$work/manifest" >/dev/null
echo "upload_packs: manifest updated ($(grep -c '^pack ' "$work/manifest") packs); the API reads it within a minute"
