#!/bin/sh
# Prove the producer version grammar in ops/release-cli.sh.
#
# These cases must die before a build starts. A name the script accepts
# but the crate does not match is a later, correct failure.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
release="$script_dir/release-cli.sh"
failed=0

expect_die() {
  label=$1
  version=$2
  needle=$3
  shift 3
  output=$(mktemp)
  set +e
  sh "$release" --version "$version" "$@" >"$output" 2>&1
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    echo "FAIL $label: expected a refusal, script exited 0" >&2
    cat "$output" >&2
    rm -f "$output"
    failed=1
    return
  fi
  if ! grep -F -q -- "$needle" "$output"; then
    echo "FAIL $label: refusal did not mention '$needle'" >&2
    cat "$output" >&2
    rm -f "$output"
    failed=1
    return
  fi
  echo "ok   $label"
  rm -f "$output"
}

expect_die "bare rc8" rc8 "invalid version"
expect_die "bare rc.8" rc.8 "invalid version"
expect_die "rc8.12" rc8.12 "invalid version"
expect_die "0.2.0-rc8" 0.2.0-rc8 "invalid version"
expect_die "0.2.0-rc8.12" 0.2.0-rc8.12 "invalid version"
expect_die "0.2.0-rc.8.12" 0.2.0-rc.8.12 "invalid version"
expect_die "leading-zero rc" 0.2.0-rc.08 "invalid version"
expect_die "alpha suffix" 0.2.0-alpha.1 "invalid version"
expect_die "skip-tests with publish" 0.2.0-rc.8 "--skip-tests is refused with --publish" --publish --skip-tests

# Canonical RC name that is not this tree's crate version. 0.2.0-rc.8 is
# already in the bucket; the name guard must still pass, and the
# source-version check is the correct next stop.
expect_die "canonical rc.N vs crate" 0.2.0-rc.8 "Cargo.toml version"

if [ "$failed" -ne 0 ]; then
  exit 1
fi

echo "ops/release-cli-version-test.sh: all refusals matched"

# Sibling object names live in this script. A release that forgets them
# ships a CLI `coder --dev` cannot start.
grep -F -q 'openagents-coder-api' "$release" || {
  echo "FAIL release script no longer names openagents-coder-api" >&2
  exit 1
}
grep -F -q 'staged_name openagents-coder-api' "$release" || {
  echo "FAIL release script does not stage openagents-coder-api-<version>-<platform>" >&2
  exit 1
}
echo "ok   sibling artifact name"
