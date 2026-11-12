#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: gcloud storage around the symbols bucket)
# Each release's symbols, kept so a crash's addresses resolve to functions and lines after the
# release is gone (#13298, docs/deployment/crash-reports.md).
#
# Bucket gs://openagentsgemini-oa-symbols (us-central1, Standard, private, objects deleted after 180 days):
#   wasm/NAME.HASH.wasm.syms.gz   a page module's function names, one "INDEX NAME" line a function
#                                 (wasm.sh writes NAME.wasm.syms; deploy.sh names it by the module's hash)
#   room/BUILDID.debug.gz         a room binary's debug file (objcopy --only-keep-debug), named by its
#                                 build ID (vm-deploy.sh symbols)
#   room/releases/NAME            that release's build ID
#
#   coax/web/symbols.sh bucket                    make the bucket and its 180-day rule (idempotent)
#   coax/web/symbols.sh pages SITE_DIR            keep every *.wasm.syms under a staged site (deploy.sh)
#   coax/web/symbols.sh page MODULE.HASH.wasm [INDEX...]
#                                                 a module's names: all of them, or those of the stack's
#                                                 wasm-function[INDEX] frames
#   coax/web/symbols.sh room-put BUILDID DEBUG.gz RELEASE    keep a room's debug file (vm-deploy.sh)
#   coax/web/symbols.sh room-get BUILDID OUT.gz   fetch it (vm-deploy.sh resolve)
set -euo pipefail
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 bucket="gs://openagentsgemini-oa-symbols"
gs() { gcloud storage "$@" --project "$project" --quiet; }
case "${1:-}" in
  bucket)
    gs buckets describe "$bucket" >/dev/null 2>&1 ||
      gs buckets create "$bucket" --location "$region" --default-storage-class STANDARD --uniform-bucket-level-access --public-access-prevention
    rule="$(mktemp)"; trap 'rm -f "$rule"' EXIT
    printf '{"rule":[{"action":{"type":"Delete"},"condition":{"age":180}}]}\n' > "$rule"
    gs buckets update "$bucket" --lifecycle-file "$rule" >/dev/null
    gs buckets update "$bucket" --update-labels app=oa-symbols,owner=openagents >/dev/null
    echo "symbols: $bucket, objects deleted after 180 days" ;;
  pages)
    site="${2:?usage: symbols.sh pages SITE_DIR}"
    work="$(mktemp -d)"; trap 'rm -rf "${work:?}"' EXIT
    have="$(gs ls "$bucket/wasm/" 2>/dev/null | sed 's|.*/||' || true)"
    n=0
    while IFS= read -r f; do
      name="$(basename "$f").gz"
      grep -qxF -- "$name" <<< "$have" && continue
      gzip -9 -n -c "$f" > "$work/$name"; n=$((n + 1))
    done < <(find "$site" -name '*.wasm.syms' -type f | sort)
    if (( n > 0 )); then gs cp "$work"/*.gz "$bucket/wasm/" >/dev/null 2>&1; fi
    echo "symbols: $n new module symbol files kept in $bucket/wasm/" ;;
  page)
    module="${2:?usage: symbols.sh page MODULE.HASH.wasm [INDEX...]}"; shift 2
    [[ "$module" =~ ^[A-Za-z0-9._-]+\.wasm$ ]] || { echo "symbols: a module's file name, NAME.HASH.wasm" >&2; exit 2; }
    all="$(gs cat "$bucket/wasm/$module.syms.gz" | gunzip)" || { echo "symbols: no symbols kept for $module" >&2; exit 1; }
    if (( $# == 0 )); then printf '%s\n' "$all"; else
      for i in "$@"; do [[ "$i" =~ ^[0-9]+$ ]] || continue; grep -m1 "^$i " <<< "$all" || echo "$i ?"; done
    fi ;;
  room-put)
    id="${2:?usage: symbols.sh room-put BUILDID DEBUG.gz RELEASE}"; file="${3:?}"; release="${4:?}"
    [[ "$id" =~ ^[0-9a-f]{8,64}$ && "$release" =~ ^[A-Za-z0-9.+_-]+$ ]] || { echo "symbols: a build ID in hex and a release's name" >&2; exit 2; }
    gs cp "$file" "$bucket/room/$id.debug.gz" >/dev/null 2>&1
    printf '%s\n' "$id" | gs cp - "$bucket/room/releases/$release" >/dev/null 2>&1
    echo "symbols: room release $release (build $id) kept in $bucket/room/" ;;
  room-get)
    id="${2:?usage: symbols.sh room-get BUILDID OUT.gz}"; out="${3:?}"
    [[ "$id" =~ ^[0-9a-f]{8,64}$ ]] || { echo "symbols: a build ID in hex" >&2; exit 2; }
    gs cp "$bucket/room/$id.debug.gz" "$out" >/dev/null 2>&1 || { echo "symbols: no debug file kept for build $id" >&2; exit 1; } ;;
  *) sed -n '2,19p' "$0" >&2; exit 2 ;;
esac
