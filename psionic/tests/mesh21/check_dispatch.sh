#!/usr/bin/env bash
# Prove mesh-only checks never enter global Python/model oracle setup.
set -euo pipefail
cd "$(dirname "$0")/../../.."
tmp=$(mktemp -d "${TMPDIR:-/tmp}/mesh-dispatch.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
cat > "$tmp/bin/coax" <<'COAX'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MESH_DISPATCH_LOG"
COAX
cat > "$tmp/bin/python3" <<'PYTHON'
#!/usr/bin/env bash
echo 'unexpected global Python oracle' >&2
exit 99
PYTHON
chmod +x "$tmp/bin/coax" "$tmp/bin/python3"
export MESH_DISPATCH_LOG="$tmp/calls"
PATH="$tmp/bin:$PATH" COAX="$tmp/bin/coax" psionic/check.sh --jobs 2 --only mesh/binding
cat > "$tmp/expected" <<'EXPECTED'
test psionic/lib/mesh/binding.coax
test psionic/lib/mesh/binding.coax --release
mutate psionic/lib/mesh/binding.coax --library --root-only --jobs 2
EXPECTED
cmp "$tmp/calls" "$tmp/expected"
if PATH="$tmp/bin:$PATH" COAX="$tmp/bin/coax" psionic/check.sh --only mesh/not-a-unit >"$tmp/invalid.log" 2>&1; then
  echo 'unknown mesh selection unexpectedly passed' >&2; exit 1
fi
cmp "$tmp/calls" "$tmp/expected"
psionic/tests/check_mesh.sh --list mesh/selection mesh/binding > "$tmp/list"
printf 'mesh/selection\nmesh/binding\n' > "$tmp/selected"
cmp "$tmp/list" "$tmp/selected"
echo 'mesh dispatch: selected roots, imported-baseline mutation scope, no global oracles, unknown selection refusal passed'
