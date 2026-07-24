#!/usr/bin/env bash

set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PACKAGE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
readonly REPOSITORY_ROOT="$(cd "${PACKAGE_DIR}/../.." && pwd)"
readonly SERVICE_VERSION="0.1.0"
readonly PROTOCOL="openagents.omega.effectd.v1"
readonly NODE_VERSION="24.13.1"
readonly NODE_ARCHIVE="node-v${NODE_VERSION}-darwin-arm64.tar.gz"
readonly NODE_URL="https://nodejs.org/dist/v${NODE_VERSION}/${NODE_ARCHIVE}"
readonly NODE_ARCHIVE_SHA256="8c039d59f2fec6195e4281ad5b0d02b9a940897b4df7b849c6fb48be6787bba6"
readonly ARTIFACT_NAME="omega-effectd-v${SERVICE_VERSION}-macos-arm64.tar.gz"

OUTPUT_DIR="${REPOSITORY_ROOT}/target/omega-effectd-component"

usage() {
  printf 'Usage: %s [--output-dir DIR]\n' "${0##*/}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

cd "${REPOSITORY_ROOT}"

if [[ -n "$(git status --porcelain)" ]]; then
  printf 'error: the worktree must be clean before a component build.\n' >&2
  git status --short >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}/downloads"
readonly WORK_DIR="$(mktemp -d /tmp/omega-effectd-component-build.XXXXXX)"
trap 'rm -rf "${WORK_DIR}"' EXIT

readonly COMPONENT_ROOT="${WORK_DIR}/omega-effectd"
readonly NODE_DOWNLOAD="${OUTPUT_DIR}/downloads/${NODE_ARCHIVE}"
readonly ARTIFACT_PATH="${OUTPUT_DIR}/${ARTIFACT_NAME}"
readonly MANIFEST_PATH="${OUTPUT_DIR}/omega-effectd-component-manifest.json"
readonly LICENSE_LIST_PATH="${WORK_DIR}/production-licenses.json"
readonly METAFILE_PATH="${WORK_DIR}/esbuild-meta.json"

mkdir -p \
  "${COMPONENT_ROOT}/bin" \
  "${COMPONENT_ROOT}/dist" \
  "${COMPONENT_ROOT}/licenses" \
  "${COMPONENT_ROOT}/runtime/bin"

if [[ ! -f "${NODE_DOWNLOAD}" ]]; then
  curl --fail --location --silent --show-error "${NODE_URL}" --output "${NODE_DOWNLOAD}"
fi

observed_node_archive_sha256="$(shasum -a 256 "${NODE_DOWNLOAD}" | awk '{print $1}')"
if [[ "${observed_node_archive_sha256}" != "${NODE_ARCHIVE_SHA256}" ]]; then
  printf 'error: Node archive digest mismatch.\n' >&2
  exit 1
fi

mkdir -p "${WORK_DIR}/node"
tar -xzf "${NODE_DOWNLOAD}" -C "${WORK_DIR}/node"
cp "${WORK_DIR}/node/node-v${NODE_VERSION}-darwin-arm64/bin/node" \
  "${COMPONENT_ROOT}/runtime/bin/node"
cp "${WORK_DIR}/node/node-v${NODE_VERSION}-darwin-arm64/LICENSE" \
  "${COMPONENT_ROOT}/licenses/Node-LICENSE.txt"
cp \
  packages/omega-effectd/node_modules/@openagentsinc/agent-harness-contract/LICENSE \
  "${COMPONENT_ROOT}/licenses/OpenAgents-Apache-2.0.txt"
chmod 755 "${COMPONENT_ROOT}/runtime/bin/node"

pnpm --dir packages/omega-effectd exec esbuild \
  src/bin/omega-effectd.ts \
  --bundle \
  --platform=node \
  --target=node24 \
  --format=esm \
  --legal-comments=external \
  --metafile="${METAFILE_PATH}" \
  --outfile="${COMPONENT_ROOT}/dist/omega-effectd.mjs"

pnpm --filter @openagentsinc/omega-effectd licenses list --prod --json \
  > "${LICENSE_LIST_PATH}"

python3 - \
  "${METAFILE_PATH}" \
  "${LICENSE_LIST_PATH}" \
  "${COMPONENT_ROOT}/licenses/THIRD_PARTY_NOTICES.json" \
  "${COMPONENT_ROOT}/licenses/THIRD_PARTY_LICENSES.txt" <<'PY'
import json
import pathlib
import sys

metafile_path, license_list_path, notices_path, texts_path = map(pathlib.Path, sys.argv[1:])
metafile = json.loads(metafile_path.read_text(encoding="utf-8"))
license_groups = json.loads(license_list_path.read_text(encoding="utf-8"))

bundled_packages: set[str] = set()
for input_path in metafile["inputs"]:
    if "/node_modules/" not in input_path:
        continue
    relative = input_path.rsplit("/node_modules/", 1)[1]
    parts = relative.split("/")
    bundled_packages.add("/".join(parts[:2]) if parts[0].startswith("@") else parts[0])

records = {}
for license_name, entries in license_groups.items():
    if license_name.lower() in {"unknown", "unlicensed"}:
        raise SystemExit(f"unsupported production license class: {license_name}")
    for entry in entries:
        if entry["name"] in bundled_packages:
            records[entry["name"]] = (license_name, entry)

missing = sorted(bundled_packages - records.keys())
if missing:
    raise SystemExit(f"missing bundled package license records: {', '.join(missing)}")

inventory = []
license_texts = []
for package_name in sorted(bundled_packages):
    license_name, entry = records[package_name]
    versions = sorted(entry.get("versions", []))
    package_path = pathlib.Path(entry["paths"][0])
    candidates = sorted(
        path
        for path in package_path.iterdir()
        if path.is_file()
        and path.name.lower().startswith(("license", "licence", "copying", "notice"))
    )
    if not candidates:
        raise SystemExit(f"missing bundled package license text: {package_name}")
    license_text = candidates[0].read_text(encoding="utf-8", errors="replace")
    inventory.append(
        {
            "name": package_name,
            "versions": versions,
            "license": license_name,
            "homepage": entry.get("homepage"),
        }
    )
    license_texts.append(
        f"===== {package_name}@{','.join(versions)} ({license_name}) =====\n\n{license_text.rstrip()}\n"
    )

notices_path.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")
texts_path.write_text("\n".join(license_texts), encoding="utf-8")
PY

python3 - "${COMPONENT_ROOT}/bin/omega-effectd" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
path.write_text(
    "#!/bin/sh\n"
    "set -eu\n"
    "component_root=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\"\n"
    "exec \"$component_root/runtime/bin/node\" \"$component_root/dist/omega-effectd.mjs\"\n",
    encoding="utf-8",
)
path.chmod(0o755)
PY

source_commit="$(git rev-parse HEAD)"
source_tree="$(git rev-parse HEAD^{tree})"
node_binary_sha256="$(shasum -a 256 "${COMPONENT_ROOT}/runtime/bin/node" | awk '{print $1}')"
service_bundle_sha256="$(shasum -a 256 "${COMPONENT_ROOT}/dist/omega-effectd.mjs" | awk '{print $1}')"
wrapper_sha256="$(shasum -a 256 "${COMPONENT_ROOT}/bin/omega-effectd" | awk '{print $1}')"
notices_sha256="$(shasum -a 256 "${COMPONENT_ROOT}/licenses/THIRD_PARTY_NOTICES.json" | awk '{print $1}')"
licenses_sha256="$(shasum -a 256 "${COMPONENT_ROOT}/licenses/THIRD_PARTY_LICENSES.txt" | awk '{print $1}')"

python3 - \
  "${COMPONENT_ROOT}/component-manifest.json" \
  "${source_commit}" \
  "${source_tree}" \
  "${node_binary_sha256}" \
  "${service_bundle_sha256}" \
  "${wrapper_sha256}" \
  "${notices_sha256}" \
  "${licenses_sha256}" <<PY
import json
import pathlib
import sys

(
    manifest_path,
    source_commit,
    source_tree,
    node_binary_sha256,
    service_bundle_sha256,
    wrapper_sha256,
    notices_sha256,
    licenses_sha256,
) = sys.argv[1:]

manifest = {
    "schema": "openagents.omega.effectd.component.v1",
    "package": "@openagentsinc/omega-effectd",
    "serviceVersion": "${SERVICE_VERSION}",
    "protocol": "${PROTOCOL}",
    "platform": "darwin-arm64",
    "source": {
        "repository": "https://github.com/OpenAgentsInc/openagents",
        "commit": source_commit,
        "tree": source_tree,
    },
    "node": {
        "version": "${NODE_VERSION}",
        "archive": "${NODE_ARCHIVE}",
        "archiveSha256": "${NODE_ARCHIVE_SHA256}",
        "binarySha256": node_binary_sha256,
    },
    "files": {
        "bin/omega-effectd": wrapper_sha256,
        "dist/omega-effectd.mjs": service_bundle_sha256,
        "licenses/THIRD_PARTY_NOTICES.json": notices_sha256,
        "licenses/THIRD_PARTY_LICENSES.txt": licenses_sha256,
    },
}
pathlib.Path(manifest_path).write_text(
    json.dumps(manifest, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY

cp "${COMPONENT_ROOT}/component-manifest.json" "${MANIFEST_PATH}"

python3 - "${COMPONENT_ROOT}" "${ARTIFACT_PATH}" <<'PY'
import gzip
import pathlib
import tarfile
import sys

component_root = pathlib.Path(sys.argv[1])
artifact_path = pathlib.Path(sys.argv[2])

with artifact_path.open("wb") as raw:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for path in sorted(component_root.rglob("*")):
                relative = pathlib.Path("omega-effectd") / path.relative_to(component_root)
                info = archive.gettarinfo(str(path), arcname=str(relative))
                info.uid = 0
                info.gid = 0
                info.uname = "root"
                info.gname = "root"
                info.mtime = 0
                if path.is_file():
                    with path.open("rb") as handle:
                        archive.addfile(info, handle)
                else:
                    archive.addfile(info)
PY

artifact_sha256="$(shasum -a 256 "${ARTIFACT_PATH}" | awk '{print $1}')"
printf '%s  %s\n' "${artifact_sha256}" "${ARTIFACT_NAME}" \
  > "${ARTIFACT_PATH}.sha256"

mkdir -p "${WORK_DIR}/smoke"
tar -xzf "${ARTIFACT_PATH}" -C "${WORK_DIR}/smoke"
smoke_output="$(
  printf '%s\n%s\n' \
    '{"schema":"openagents.omega.effectd.v1","kind":"request","id":"init-1","generation":0,"method":"initialize","params":{"generation":7}}' \
    '{"schema":"openagents.omega.effectd.v1","kind":"request","id":"health-1","generation":7,"method":"health"}' \
  | OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT="${WORK_DIR}/data" \
      "${WORK_DIR}/smoke/omega-effectd/bin/omega-effectd"
)"

python3 - "${smoke_output}" <<'PY'
import json
import sys

lines = [json.loads(line) for line in sys.argv[1].splitlines() if line.strip()]
if len(lines) != 2:
    raise SystemExit("component smoke did not return two protocol responses")
initialize, health = lines
if initialize.get("ok") is not True or initialize.get("result", {}).get("generation") != 7:
    raise SystemExit("component initialize smoke failed")
if health.get("ok") is not True or health.get("result", {}).get("status") != "running":
    raise SystemExit("component health smoke failed")
PY

printf 'omega-effectd component ready\n'
printf '  artifact: %s\n' "${ARTIFACT_PATH}"
printf '  sha256:   %s\n' "${artifact_sha256}"
printf '  manifest: %s\n' "${MANIFEST_PATH}"
