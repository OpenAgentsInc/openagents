#!/usr/bin/env bash
# Time the edit-to-build loop for chosen packages, so a build setting can be
# compared before and after on the same machine.
#
#   scripts/dev/build-bench.sh [options] PACKAGE...
#
# For each package it runs, in order:
#   1. warm-up   the command once, untimed in the table, so every dependency
#                is built with this command's features (timed as "prep")
#   2. noop      the same command again, nothing changed
#   3. edit      after adding one private function to the package's root
#                source file, which forces the package itself to recompile
# for both `cargo build -p P` and `cargo test -p P --no-run`. The edit is
# reverted when the script exits.
# With --edit-package, every row edits that package and measures the named
# consumer. Each consumer must depend on the edited package.
#
# Options:
#   --manifest-path M  a nested workspace (apps/openagents/Cargo.toml)
#   --target-dir D     the Cargo target directory (default: CARGO_TARGET_DIR,
#                      else a "build-bench" folder in `openagents scratch`)
#   --variant NAME     a label for this run; known names also set flags:
#                        baseline   the repository's settings
#                        incr       CARGO_INCREMENTAL=1 (every workspace crate)
#                        noincr     CARGO_INCREMENTAL=0 (no crate; the rule
#                                   before 2026-10-19)
#                        full       full debug information (debug = 2)
#                        lt         debug = "line-tables-only" for every package
#                        d0         debug = 0 for every package
#                        mold       link with mold (Linux)
#                        lld        link with lld
#                      Several names join with "+", e.g. lt+mold.
#   --jobs N           CARGO_BUILD_JOBS (default: half the cores)
#   --profile NAME     override Cargo's default build/test profile
#   --no-wrapper       bypass the compile cache wrapper (kache or sccache),
#                      to time the compiler itself
#   --only build|test  run one of the two commands
#   --edit-package P   edit P instead of each measured package, to time a
#                      split crate together with its dependents
#   --timings          pass --timings to the warm-up build (an HTML report
#                      under the target's cargo-timings folder)
#   --cold             first build every named package together, timed as
#                      the "(all)" row, with --timings; on an empty target
#                      this is the cold build
#   --out FILE         append rows as tab-separated values; the final column
#                      names the edited package
#
# A variant changes compiler flags, so give each variant its own target
# directory unless a cold build is what you want to time. The script loads
# scripts/dev/cargo-build-env.sh (AGENTS.md, Velocity), then applies the
# variant, so `incr` and `noincr` deliberately override the profile's list of
# incremental crates.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
manifest=
target=${CARGO_TARGET_DIR:-}
variant=baseline
jobs=
profile=
wrapper=1
only=
edit_package=
timings=0
cold=0
out=
packages=()

while [ $# -gt 0 ]; do
  case $1 in
    --manifest-path) manifest=$2; shift 2 ;;
    --target-dir) target=$2; shift 2 ;;
    --variant) variant=$2; shift 2 ;;
    --jobs) jobs=$2; shift 2 ;;
    --profile) profile=$2; shift 2 ;;
    --no-wrapper) wrapper=0; shift ;;
    --only) only=$2; shift 2 ;;
    --edit-package)
      [ $# -ge 2 ] && [ -n "$2" ] && [[ $2 != -* ]] || {
        echo 'Name a package after --edit-package.' >&2; exit 2;
      }
      edit_package=$2; shift 2 ;;
    --timings) timings=1; shift ;;
    --cold) cold=1; shift ;;
    --out) out=$2; shift 2 ;;
    -h|--help) sed -n '2,/^set -/{ /^set -/d; p; }' "$0"; exit 0 ;;
    -*) echo "Unknown option: $1" >&2; exit 2 ;;
    *) packages+=("$1"); shift ;;
  esac
done
[ ${#packages[@]} -gt 0 ] || { echo 'Name at least one package.' >&2; exit 2; }

if [ -z "$target" ]; then
  scratch=$(openagents scratch 2>/dev/null || true)
  [ -n "$scratch" ] || { echo 'Set --target-dir or CARGO_TARGET_DIR.' >&2; exit 2; }
  target=$scratch/build-bench
fi
mkdir -p "$target"
export CARGO_TARGET_DIR=$target
scratch=$(openagents scratch)
run_dir=$(mktemp -d "$scratch/build-bench.XXXXXX")

cores=$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)
export CARGO_BUILD_JOBS=${jobs:-$(( cores / 2 > 0 ? cores / 2 : 1 ))}

cargo_args=()
[ -n "$manifest" ] && cargo_args+=(--manifest-path "$manifest")
profile_args=()
[ -n "$profile" ] && profile_args+=(--profile "$profile")

# The build settings every agent build uses.
(
  cd "$root"
  [ -n "$manifest" ] && cd "$(dirname "$manifest")"
  oa_build_manifest=
  . "$root/scripts/dev/cargo-build-env.sh"
  env | grep -E '^(RUSTC|CARGO_BUILD_BUILD_DIR|CARGO_INCREMENTAL)=' > "$run_dir/env"
)
unset CARGO_INCREMENTAL
set -a; . "$run_dir/env"; set +a

if [ "$wrapper" = 0 ]; then
  export RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= OA_SCCACHE=0
fi

os=$(uname -s)
linker_flags=
IFS=+ read -r -a parts <<< "$variant"
for part in "${parts[@]}"; do
  case $part in
    baseline) ;;
    incr) export CARGO_INCREMENTAL=1 ;;
    noincr) export CARGO_INCREMENTAL=0 ;;
    full) export CARGO_PROFILE_DEV_DEBUG=full CARGO_PROFILE_TEST_DEBUG=full ;;
    lt) export CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only ;;
    d0) export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 ;;
    mold) linker_flags="$linker_flags -C link-arg=-fuse-ld=mold" ;;
    lld)
      if [ "$os" = Darwin ]; then
        linker_flags="$linker_flags -C link-arg=-fuse-ld=$(command -v ld64.lld)"
      else
        linker_flags="$linker_flags -C link-arg=-fuse-ld=lld"
      fi ;;
    *) echo "Unknown variant part: $part (the label is kept, no flags set)" >&2 ;;
  esac
done
if [ -n "$linker_flags" ]; then
  host=$(rustc -vV | sed -n 's/^host: //p')
  key=CARGO_TARGET_$(printf '%s' "$host" | tr 'a-z-' 'A-Z_')_RUSTFLAGS
  export "$key=${linker_flags# }"
fi

# Map a package to its root source file.
src_of() {
  cargo metadata --format-version 1 --no-deps "${cargo_args[@]}" 2>/dev/null |
    python3 -c '
import json, sys
name = sys.argv[1]
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == name:
        for want in ("lib", "rlib", "cdylib", "staticlib", "bin"):
            for t in p["targets"]:
                if want in t["kind"]:
                    print(t["src_path"]); sys.exit(0)
sys.exit(1)' "$1"
}

edited=()
backup_dir=$run_dir/backups
mkdir -p "$backup_dir"
restore() {
  local i=0 f
  for f in "${edited[@]:-}"; do
    [ -n "$f" ] && cp "$backup_dir/$i" "$f"
    i=$((i + 1))
  done
  rm -rf "$backup_dir"
}
trap restore EXIT

now() { python3 -c 'import time; print(time.time())'; }
timed() {
  local start end status=0
  start=$(now)
  "$@" > "$run_dir/last.log" 2>&1 || status=$?
  end=$(now)
  python3 -c "print(f'{$end - $start:.1f}')"
  if [ $status -ne 0 ]; then
    echo "  failed: $* (see $run_dir/last.log)" >&2
    tail -20 "$run_dir/last.log" >&2
    return $status
  fi
}

counter=$(date +%s)
edit() {
  local file=$1 f seen=0
  for f in "${edited[@]:-}"; do [ "$f" = "$file" ] && seen=1; done
  if [ "$seen" = 0 ]; then
    cp "$file" "$backup_dir/${#edited[@]}"
    edited+=("$file")
  fi
  counter=$((counter + 1))
  printf '\n#[allow(dead_code)]\nfn build_bench_edit_%s() -> u64 {\n    %s\n}\n' "$counter" "$counter" >> "$file"
}

host_name=$(hostname -s 2>/dev/null || hostname)
printf '%-28s %-14s %-6s %8s %8s %8s %s\n' package variant cmd prep noop edit edited_package
row() {
  printf '%-28s %-14s %-6s %8s %8s %8s %s\n' "$1" "$variant" "$2" "$3" "$4" "$5" "$6"
  if [ -n "$out" ]; then
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u +%FT%TZ)" "$host_name" "$1" "$variant" "$2" \
      "$CARGO_BUILD_JOBS" "$3" "$4" "$5" "$6" >> "$out"
  fi
}
edit_file=
if [ -n "$edit_package" ]; then
  edit_file=$(src_of "$edit_package") || { echo "No package named $edit_package" >&2; exit 2; }
fi
if [ "$cold" = 1 ]; then
  all=()
  for pkg in "${packages[@]}"; do all+=(-p "$pkg"); done
  cold_time=$(timed cargo build -q "${cargo_args[@]}" "${profile_args[@]}" "${all[@]}" --timings)
  row '(all)' cold "$cold_time" - - -
fi
for pkg in "${packages[@]}"; do
  file=$(src_of "$pkg") || { echo "No package named $pkg" >&2; exit 2; }
  [ -n "$edit_file" ] && file=$edit_file
  for cmd in build test; do
    [ -n "$only" ] && [ "$only" != "$cmd" ] && continue
    if [ "$cmd" = build ]; then
      argv=(cargo build -q "${cargo_args[@]}" "${profile_args[@]}" -p "$pkg")
    else
      argv=(cargo test -q "${cargo_args[@]}" "${profile_args[@]}" -p "$pkg" --no-run)
    fi
    prep_argv=("${argv[@]}")
    [ "$timings" = 1 ] && [ "$cmd" = build ] && prep_argv+=(--timings)
    prep=$(timed "${prep_argv[@]}")
    noop=$(timed "${argv[@]}")
    edit "$file"
    change=$(timed "${argv[@]}")
    row "$pkg" "$cmd" "$prep" "$noop" "$change" "${edit_package:-$pkg}"
  done
done
