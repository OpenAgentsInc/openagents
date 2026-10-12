#!/usr/bin/env bash
# Check that each synced lane under nips/ is byte-for-byte the upstream
# commit nips/manifest.json names.
#
# Usage: ./scripts/check-nips-pin.sh
#
# For each source in the manifest, the script reads the upstream tree at the
# pinned commit from the GitHub API, compares every top-level .md blob's
# git object id with the vendored file's, and fails on a changed byte, a
# missing file, or a stray file. A repo-owned README.md the upstream does not
# publish is allowed. Set GH_TOKEN to raise the API rate limit.
set -euo pipefail
cd "$(dirname "$0")/.."

MANIFEST="nips/manifest.json"
status=0
auth=()
if [ -n "${GH_TOKEN:-}" ]; then
  auth=(-H "Authorization: Bearer ${GH_TOKEN}")
fi

while IFS=$'\t' read -r name repo subdir commit; do
  slug="${repo#https://github.com/}"
  slug="${slug%.git}"
  ref="${commit}"
  if [ "${subdir}" != "." ]; then
    ref="${commit}:${subdir}"
  fi
  upstream="$(curl -fsS "${auth[@]}" "https://api.github.com/repos/${slug}/git/trees/${ref}" \
    | python3 -c 'import json,sys
for e in json.load(sys.stdin)["tree"]:
    if e["type"] == "blob" and e["path"].endswith(".md"):
        print(e["path"], e["sha"])')"
  expected=0
  while read -r path sha; do
    expected=$((expected + 1))
    local_file="nips/${name}/${path}"
    if [ ! -f "${local_file}" ]; then
      echo "drift: ${local_file} is missing (upstream ${sha})"
      status=1
      continue
    fi
    have="$(git hash-object "${local_file}")"
    if [ "${have}" != "${sha}" ]; then
      echo "drift: ${local_file} differs from upstream (${have} != ${sha})"
      status=1
    fi
  done <<< "${upstream}"
  while read -r local_file; do
    base="$(basename "${local_file}")"
    if ! grep -q "^${base} " <<< "${upstream}"; then
      if [ "${base}" = "README.md" ]; then
        continue
      fi
      echo "drift: ${local_file} is not in the upstream tree"
      status=1
    fi
  done < <(find "nips/${name}" -type f)
  echo "pin: ${name} checked ${expected} upstream files at ${commit}"
done < <(python3 -c 'import json
for s in json.load(open("nips/manifest.json"))["sources"]:
    print("\t".join([s["name"], s["repo"], s["subdir"], s["commit"]]))')

if [ "${status}" -ne 0 ]; then
  echo "pin: drift found; run ./scripts/sync-nips.sh or restore the files"
fi
exit "${status}"
