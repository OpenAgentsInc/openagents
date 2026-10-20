#!/usr/bin/env bash
# Check that the compatibility wrapper previews and never weakens age limits.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/disk-cleanup-wrapper.XXXXXX")
trap 'rm -rf "$test_root"' EXIT
mkdir -p "$test_root/home/.openagents"
cat > "$test_root/tool" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$CLEANUP_TEST_ARGS"
EOF
chmod +x "$test_root/tool"
export JANITOR_HOME=$test_root/home OPENAGENTS_DISK_CLEANUP=$test_root/tool CLEANUP_TEST_ARGS=$test_root/args
bash "$here/disk-janitor.sh" --now --low --dry-run
test "$(head -n 1 "$test_root/args")" = scan
grep -qx -- '--age-hours' "$test_root/args"
test "$(tail -n 1 "$test_root/args")" = 24
bash "$here/disk-janitor.sh" --apply --plan reviewed.json --receipt receipt.json
test "$(head -n 1 "$test_root/args")" = apply
grep -qx reviewed.json "$test_root/args"
rm "$test_root/args"
touch "$test_root/home/.openagents/disk-janitor.off"
bash "$here/disk-janitor.sh" --now
test ! -e "$test_root/args"
echo 'disk cleanup wrapper tests passed'
