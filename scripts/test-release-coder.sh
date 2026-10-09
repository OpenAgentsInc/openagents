#!/bin/sh
# Test Coder release names, complete channel coverage, source isolation,
# and refusal before a published version can build. Uses scratch files only.
set -eu

checkout=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/coder-release-test.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
CODER_RELEASE_SCRIPT="$checkout/scripts/release/coder.sh"
CODER_RELEASE_LIBRARY=1
# shellcheck source=scripts/release/coder.sh
. "$CODER_RELEASE_SCRIPT"
set -eu

failures=0
pass() { echo "ok   $1"; }
fail() { echo "FAIL $1"; failures=$((failures + 1)); }

version=1.0.0-rc.3
for good in 1.0.0 1.0.0-rc.0 1.0.0-rc.3 12.3.45-rc.10; do
  if (version=$good; check_version) 2>/dev/null; then pass "version $good"; else fail "version $good"; fi
done
for bad in 1.0.0-rc3 1.0.0-rc.03 v1.0.0 1.0 rc.3 1.0.0-beta.1; do
  if (version=$bad; check_version) 2>/dev/null; then fail "refuse version $bad"; else pass "refuse version $bad"; fi
done
for bad in '../openagents' 'rc/name' 'rc name' ''; do
  if (check_channel "$bad") 2>/dev/null; then fail "refuse channel $bad"; else pass "refuse channel $bad"; fi
done
if (version=1.0.0-rc.3; check_stable_version stable) 2>/dev/null; then fail "refuse a candidate on stable"; else pass "refuse a candidate on stable"; fi
if (version=1.0.0; check_stable_version stable) && (version=1.0.0-rc.3; check_stable_version rc); then
  pass "stable takes a release, rc takes a candidate"
else
  fail "stable takes a release, rc takes a candidate"
fi
[ "$prefix" = coder ] && [ "$channel" = rc ] && [ "$(channel_name rc)" = coder.rc ] &&
  [ "$(sums_file_name)" = SHA256SUMS-coder-1.0.0-rc.3 ] && pass "separate Coder prefix and default RC channel" || fail "separate Coder prefix and default RC channel"
[ "$(binary_for coder)" = coder-new ] && [ "$(products_for linux-x86_64)" = 'coder openagents microcoder' ] &&
  [ "$(products_for windows-x86_64)" = 'coder openagents microcoder coder-boundary' ] && pass "bundle the CLI, its engine, and the Windows boundary" || fail "bundle the CLI, its engine, and the Windows boundary"

: >"$work/sums"
: >"$work/objects"
for platform in $all_platforms; do
  archive=$(archive_name "$platform")
  printf '%s\n' "$archive" >>"$work/objects"
  printf '%064d  %s\n' 0 "$(sums_name_for "$archive" "$platform")" >>"$work/sums"
done
[ "$(wc -l <"$work/objects" | tr -d ' ')" = 7 ] && [ -z "$(uncovered_platforms "$work/sums" "$work/objects")" ] &&
  pass "all seven platforms require one archive and its checksum" || fail "all seven platforms require one archive and its checksum"
[ "$(archive_name macos-aarch64)" = coder-1.0.0-rc.3-macos-aarch64.tar.gz ] &&
  [ "$(archive_name windows-x86_64)" = coder-1.0.0-rc.3-windows-x86_64.zip ] &&
  [ "$(installed_name microcoder windows-x86_64)" = microcoder.exe ] && [ "$(installed_name coder linux-x86_64)" = coder ] &&
  pass "archives are .tar.gz, Windows .zip, holding installed names" || fail "archives are .tar.gz, Windows .zip, holding installed names"

grep -vxF 'coder-1.0.0-rc.3-linux-aarch64-musl.tar.gz' "$work/objects" >"$work/gap-objects"
[ "$(uncovered_platforms "$work/sums" "$work/gap-objects")" = ' linux-aarch64-musl' ] &&
  pass "a missing archive blocks its platform" || fail "a missing archive blocks its platform"
grep -v 'coder-1.0.0-rc.3-windows-x86_64.zip' "$work/sums" >"$work/gap-sums"
[ "$(uncovered_platforms "$work/gap-sums" "$work/objects")" = ' windows-x86_64' ] &&
  pass "a missing archive checksum blocks the channel" || fail "a missing archive checksum blocks the channel"
grep -vxF 'coder-1.0.0-rc.3-windows-x86_64.zip' "$work/objects" >"$work/gap-objects"
for name in coder openagents microcoder coder-boundary; do
  printf '%s\n' "$name-1.0.0-rc.3-windows-x86_64.exe" >>"$work/gap-objects"
done
[ "$(uncovered_platforms "$work/sums" "$work/gap-objects")" = ' windows-x86_64' ] &&
  pass "separate executables do not cover an archive release" || fail "separate executables do not cover an archive release"
sed 's/1\.0\.0-rc\.3-/1.0.0-rc.30-/' "$work/objects" >"$work/longer-objects"
[ "$(uncovered_platforms "$work/sums" "$work/longer-objects")" = " $(printf '%s' "$all_platforms" | sed 's/ $//')" ] &&
  pass "a longer RC version cannot cover this RC" || fail "a longer RC version cannot cover this RC"

# The archive step packs the staged executables under their installed names.
dist="$work/dist"
mkdir -p "$dist"
for platform in linux-x86_64 windows-x86_64; do
  for name in $(products_for "$platform"); do
    printf '#!/bin/sh\necho %s\n' "$name" >"$dist/$(artifact_name "$name" "$platform")"
  done
  make_archive "$platform"
done
[ "$(tar -tzf "$dist/coder-1.0.0-rc.3-linux-x86_64.tar.gz" | sort | tr '\n' ' ')" = 'coder microcoder openagents ' ] &&
  pass "a Unix archive holds coder, openagents, and microcoder" || fail "a Unix archive holds coder, openagents, and microcoder"
mkdir -p "$work/unpacked"
tar -xzf "$dist/coder-1.0.0-rc.3-linux-x86_64.tar.gz" -C "$work/unpacked"
[ -x "$work/unpacked/coder" ] && [ "$("$work/unpacked/microcoder")" = microcoder ] &&
  pass "archived executables keep their mode" || fail "archived executables keep their mode"
[ "$(unzip -Z1 "$dist/coder-1.0.0-rc.3-windows-x86_64.zip" | sort | tr '\n' ' ')" = 'coder-boundary.exe coder.exe microcoder.exe openagents.exe ' ] &&
  pass "the Windows archive holds the four executables" || fail "the Windows archive holds the four executables"
[ -z "$(find "$dist" -name '.bundle-*')" ] && pass "the archive step leaves no staging folder" || fail "the archive step leaves no staging folder"

gs() { return 1; }
if (refuse_uncovered_channel rc "$work/sums" "$work/gap-objects") >"$work/refusal.log" 2>&1; then
  fail "a channel cannot pass a missing Windows archive"
else
  grep -q 'windows-x86_64' "$work/refusal.log" && pass "a channel cannot pass a missing Windows archive" || fail "a channel cannot pass a missing Windows archive"
fi

# Compare a scratch commit and its archive, then reject a changed source.
repository="$work/repository"
source_fixture="$work/source"
mkdir -p "$repository" "$source_fixture"
git -C "$repository" init -q
printf 'committed source\n' >"$repository/source.txt"
printf 'target/\n' >"$repository/.gitignore"
git -C "$repository" add .
git -C "$repository" -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm fixture
fixture_commit=$(git -C "$repository" rev-parse HEAD)
git -C "$repository" archive HEAD | (cd "$source_fixture" && tar -xf -)
source_matches "$repository" "$fixture_commit" "$source_fixture" && pass "an isolated commit archive matches its source tree" || fail "an isolated commit archive matches its source tree"
printf 'changed source\n' >"$source_fixture/source.txt"
if source_matches "$repository" "$fixture_commit" "$source_fixture" >"$work/diff.log"; then fail "changed source is refused"; else pass "changed source is refused"; fi

# A fake bucket says the checksum file already exists. Cargo must never run.
mkdir -p "$work/tools" "$work/config"
cat >"$work/tools/gcloud" <<'SH'
#!/bin/sh
case "$*" in 'storage ls '*SHA256SUMS-coder-*) exit 0 ;; *) exit 1 ;; esac
SH
cat >"$work/tools/cargo" <<'SH'
#!/bin/sh
printf 'unexpected build\n' >"$CODER_RELEASE_FIXTURE_MARKER"
exit 1
SH
chmod +x "$work/tools/gcloud" "$work/tools/cargo"
if CODER_RELEASE_LIBRARY=0 CLOUDSDK_CONFIG="$work/config" OPENAGENTS_NOTARY_ENV="$work/no-notary-config" CARGO_TARGET_DIR="$work/target" \
  CODER_RELEASE_FIXTURE_MARKER="$work/cargo-ran" PATH="$work/tools:$PATH" \
  sh "$CODER_RELEASE_SCRIPT" --version 1.0.0-rc.3 --publish >"$work/immutable.log" 2>&1; then
  fail "a published version cannot build again"
else
  grep -q 'already published' "$work/immutable.log" && [ ! -e "$work/cargo-ran" ] && pass "a published version cannot build again" || fail "a published version cannot build again"
fi
if CODER_RELEASE_LIBRARY=0 CLOUDSDK_CONFIG="$work/config" OPENAGENTS_NOTARY_ENV="$work/no-notary-config" CARGO_TARGET_DIR="$work/target" \
  sh "$CODER_RELEASE_SCRIPT" --version 1.0.0-rc.3 --publish --skip-notarization >"$work/notary.log" 2>&1; then
  fail "publishing without notarization is refused"
else
  grep -q 'skip-notarization is refused with --publish' "$work/notary.log" && pass "publishing without notarization is refused" || fail "publishing without notarization is refused"
fi

exit "$failures"
