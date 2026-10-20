#!/usr/bin/env bash
# Tests scripts/dev/regenerate.sh in a scratch repository with fake
# generators: which run for a change, what each line says, and the exit.
# Run: scripts/dev/regenerate.test.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(cd "$(mktemp -d)" && pwd -P)
trap 'rm -rf "$tmp"' EXIT
repo=$tmp/repo
mkdir -p "$repo/scripts/dev" "$repo/.openagents" "$repo/crates/a/src" "$repo/crates/b/src" "$repo/gen"
cp "$here/regenerate.sh" "$repo/scripts/dev/"
cat >"$repo/.openagents/generated.json" <<'EOF'
{"generated": [
  {"name": "from-sources", "files": ["gen/one.txt"], "sources": ["crates/a/"], "markers": ["USAGE"],
   "regenerate": "cat crates/a/src/lib.rs > gen/one.txt"},
  {"name": "by-package", "files": ["gen/two.txt"], "sources": [], "package": "b",
   "regenerate": "echo two > gen/two.txt"},
  {"name": "needs-ndk", "files": ["gen/three.txt"], "sources": ["crates/a/"],
   "requires": {"env": ["REGEN_TEST_NO_SUCH_VAR"], "commands": ["no-such-command-here"]},
   "regenerate": "echo three > gen/three.txt"},
  {"name": "broken", "files": ["gen/four.txt"], "sources": ["crates/b/"],
   "regenerate": "echo the generator broke >&2; exit 3"}
]}
EOF
printf '[package]\nname = "a"\n' >"$repo/crates/a/Cargo.toml"
printf '[package]\nname = "b"\n' >"$repo/crates/b/Cargo.toml"
echo 'const USAGE: &str = "old";' >"$repo/crates/a/src/lib.rs"
echo 'pub fn b() {}' >"$repo/crates/b/src/lib.rs"
echo 'const USAGE: &str = "old";' >"$repo/gen/one.txt"
echo two >"$repo/gen/two.txt"
git -C "$repo" init -q
git -C "$repo" -c user.name=t -c user.email=t@t add -A
git -C "$repo" -c user.name=t -c user.email=t@t commit -q -m base

fail=0
expect() { grep -qxF -- "$2" <<<"$1" || { echo "FAIL: expected line: $2"; echo "$1" | sed 's/^/  got: /'; fail=1; }; }
refuse() { if grep -qF -- "$2" <<<"$1"; then echo "FAIL: unexpected: $2"; fail=1; fi; }

# A change to crates/a's USAGE: due from-sources (marker) and needs-ndk
# (skipped), not by-package or broken.
echo 'const USAGE: &str = "new";' >"$repo/crates/a/src/lib.rs"
git -C "$repo" -c user.name=t -c user.email=t@t commit -qam 'change a'
out=$(cd "$repo" && CARGO_TARGET_DIR=$tmp/target scripts/dev/regenerate.sh --changed HEAD~1 2>/dev/null)
expect "$out" "regenerated from-sources: gen/one.txt"
expect "$out" 'skipped needs-ndk: needs $REGEN_TEST_NO_SUCH_VAR and no-such-command-here'
refuse "$out" "by-package"
refuse "$out" "broken"
grep -q new "$repo/gen/one.txt" || { echo "FAIL: gen/one.txt was not rewritten"; fail=1; }
git -C "$repo" checkout -q -- gen

# A source file without the marker is not due.
echo '// no marker' >"$repo/crates/a/src/other.rs"
git -C "$repo" add -A && git -C "$repo" -c user.name=t -c user.email=t@t commit -qm 'unmarked'
out=$(cd "$repo" && scripts/dev/regenerate.sh --changed HEAD~1 2>/dev/null)
refuse "$out" "from-sources"

# A change in package b: due by-package (its check lives there) and broken
# (its sources); a failure exits 1 and names the generator.
echo 'pub fn b2() {}' >>"$repo/crates/b/src/lib.rs"
git -C "$repo" -c user.name=t -c user.email=t@t commit -qam 'change b'
set +e
out=$(cd "$repo" && scripts/dev/regenerate.sh --changed HEAD~1 2>/dev/null)
status=$?
set -e
expect "$out" "current by-package"
expect "$out" "failed broken: exit 3: the generator broke"
[ "$status" = 1 ] || { echo "FAIL: a failed generator exits 1, got $status"; fail=1; }

# --packages names a package the checks test without a changed file there.
out=$(cd "$repo" && scripts/dev/regenerate.sh --changed HEAD --packages b 2>/dev/null)
expect "$out" "current by-package"
refuse "$out" "from-sources"

# --only, and an unknown name is refused.
out=$(cd "$repo" && scripts/dev/regenerate.sh --only by-package 2>/dev/null)
[ "$out" = "current by-package" ] || { echo "FAIL: --only ran: $out"; fail=1; }
if (cd "$repo" && scripts/dev/regenerate.sh --only nope >/dev/null 2>&1); then
  echo "FAIL: --only with an unknown name passed"; fail=1
fi

# --list prints each check beside its command.
out=$(cd "$repo" && scripts/dev/regenerate.sh --list)
expect "$out" "  regenerate: echo two > gen/two.txt"

# The repository's own registry lists every generator with a check.
out=$(cd "$here/../.." && scripts/dev/regenerate.sh --list)
for name in cli-tree sdk-reference app-openapi route-map-sources android-core-binding; do
  expect "$out" "$name"
done
if grep -qE '^  check: *$' <<<"$out"; then echo "FAIL: a generator has no check"; fail=1; fi

[ "$fail" = 0 ] && echo "regenerate.test.sh: ok"
exit "$fail"
