#!/usr/bin/env bash
# Sync NIP specifications from the two pinned upstream sources into nips/.
#
# Usage: ./scripts/sync-nips.sh
#
# The script replaces the contents of nips/official/ and nips/block/
# with the current upstream files, and writes
# nips/manifest.json with the exact upstream commit for each source.
# A repo-owned lane README.md is preserved when the upstream does not
# publish one; an upstream README always wins. The manifest file count
# records upstream files only.
# Review the diff before you commit. A specification change becomes
# normative only after review and a fixture update (see nips/README.md).
set -euo pipefail
cd "$(dirname "$0")/.."

MANIFEST="nips/manifest.json"
ENTRIES=""

sync_source() {
  local name="$1" repo="$2" subdir="$3"
  local tmp
  tmp="$(mktemp -d)"
  echo "sync: ${name} <- ${repo} (${subdir})"
  git clone --quiet --depth 1 --filter=blob:none --sparse "${repo}" "${tmp}"
  if [ "${subdir}" != "." ]; then
    git -C "${tmp}" sparse-checkout set --cone "${subdir}" >/dev/null
  fi
  local commit branch
  commit="$(git -C "${tmp}" rev-parse HEAD)"
  branch="$(git -C "${tmp}" rev-parse --abbrev-ref HEAD)"
  local lane_readme=""
  if [ -f "nips/${name}/README.md" ]; then
    lane_readme="$(mktemp)"
    cp "nips/${name}/README.md" "${lane_readme}"
  fi
  rm -rf "nips/${name}"
  mkdir -p "nips/${name}"
  find "${tmp}/${subdir}" -maxdepth 1 -type f -name '*.md' \
    -exec cp {} "nips/${name}/" \;
  local count
  count="$(find "nips/${name}" -type f -name '*.md' | wc -l | tr -d ' ')"
  if [ -n "${lane_readme}" ]; then
    if [ ! -f "nips/${name}/README.md" ]; then
      cp "${lane_readme}" "nips/${name}/README.md"
      echo "sync: ${name} kept the repo-owned README.md"
    fi
    rm -f "${lane_readme}"
  fi
  echo "sync: ${name} = ${count} files at ${commit}"
  local entry
  entry="$(printf '    {\n      "name": "%s",\n      "repo": "%s",\n      "subdir": "%s",\n      "branch": "%s",\n      "commit": "%s",\n      "tree_url": "%s/tree/%s%s",\n      "synced_at": "%s",\n      "files": %s\n    }' \
    "${name}" "${repo}" "${subdir}" "${branch}" "${commit}" \
    "${repo%.git}" "${commit}" \
    "$([ "${subdir}" = "." ] && echo "" || echo "/${subdir}")" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "${count}")"
  if [ -n "${ENTRIES}" ]; then
    ENTRIES="${ENTRIES},
${entry}"
  else
    ENTRIES="${entry}"
  fi
  rm -rf "${tmp}"
}

sync_source "official" "https://github.com/nostr-protocol/nips" "."
sync_source "block" "https://github.com/block/buzz" "docs/nips"

printf '{\n  "sources": [\n%s\n  ]\n}\n' "${ENTRIES}" > "${MANIFEST}"
echo "sync: wrote ${MANIFEST}"
echo "sync: done. Review the diff, then commit."
