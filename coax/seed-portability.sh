#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: compares the C seeds with what a host C++ compiler's stage0 emits)
# Check that the frontend C seeds (bootstrap/*.c and .h) are what stage0
# emits on this host and under Linux GCC, the compiler the API image builds
# with (coax/web/api/Dockerfile). Clang and GCC evaluate the operands of an
# unsequenced C++ expression in different orders, so an emitter that numbers
# temporaries inside one is platform dependent; a seed refreshed on a Mac then
# fails the Linux bootstrap with "Checker seed is stale".
#
#   coax/seed-portability.sh           this host only
#   coax/seed-portability.sh --linux   also debian:bookworm-slim GCC, through
#                                      Docker when it runs, else Cloud Build
#
# Run it after refreshing a seed or changing coax/src/emit_c.cpp. It runs no
# gates; coax/build.sh still decides whether a seed is trusted.
set -euo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
repo="$(cd "$here/.." && pwd -P)"
seeds=(lexer sources parser checker)
compare() {  # compare ROOT: build stage0 from ROOT's seeds, emit, compare
  cd "$1/coax"
  local w rc=0 s
  w="$(mktemp -d)"
  {
    printf 'static constexpr char PROTO_CORE_SOURCE[] = R"COAX_CORE('; cat lib/proto/wire.coax
    printf ')COAX_CORE";\nstatic constexpr char PROTO_MEMORY_SOURCE[] = R"COAX_MEMORY('; cat lib/proto/memory.coax
    printf ')COAX_MEMORY";\nstatic constexpr char PROTO_IMPORTER_SOURCE[] = R"COAX_IMPORTER('; cat lib/proto/importer.coax
    printf ')COAX_IMPORTER";\nstatic constexpr char PROTO_FINGERPRINT_SOURCE[] = R"COAX_FP('; cat lib/proto/fingerprint.coax
    printf ')COAX_FP";\n'
  } > "$w/proto_runtime.inc"
  local objs=()
  for s in budget check-domains profiles mutation-schedule lexer parser checker sources; do
    "${CC:-cc}" -std=c11 -O1 -c "bootstrap/$s.c" -o "$w/seed-$s.o" & objs+=("$w/seed-$s.o")
  done
  wait
  "${CXX:-c++}" -std=c++20 -O1 -w -DCOAX_BOOTSTRAP_IMPORTER -I bootstrap -I "$w" -o "$w/stage0" src/main.cpp "${objs[@]}"
  for s in "${seeds[@]}"; do
    "$w/stage0" c "lib/compiler/$s.coax" --library --header "$w/$s.h" > "$w/$s.c"
    if cmp -s "bootstrap/$s.c" "$w/$s.c" && cmp -s "bootstrap/$s.h" "$w/$s.h"; then echo "$s: matches"
    else echo "$s: DIFFERS from what stage0 emits"; diff "bootstrap/$s.c" "$w/$s.c" | head -20 || true; rc=1; fi
  done
  rm -rf "$w"
  return $rc
}
case "${1:-}" in
  --inside) compare "$2"; exit ;;
  '') compare "$repo"; exit ;;
  --linux) ;;
  *) echo 'usage: seed-portability.sh [--linux]' >&2; exit 2 ;;
esac
echo "host ($("${CXX:-c++}" --version | head -1)):"
compare "$repo"
ctx="$(mktemp -d)"
trap 'rm -rf "$ctx"' EXIT
git -C "$repo" ls-files -z -- coax/src coax/bootstrap coax/lib ':(glob)coax/*' | (cd "$repo" && rsync -a --from0 --files-from=- ./ "$ctx/")
cp "$here/seed-portability.sh" "$ctx/coax/"
# The working tree's copies, so an uncommitted refresh is what gets checked.
cmd='apt-get update -qq && apt-get install -y -qq --no-install-recommends gcc g++ libc6-dev > /dev/null && coax/seed-portability.sh --inside'
echo 'linux (debian:bookworm-slim GCC):'
if docker info > /dev/null 2>&1; then
  docker run --rm -v "$ctx:/w" -w /w debian:bookworm-slim bash -c "$cmd /w"
else
  project=openagentsgemini region=us-central1
  printf 'steps:\n- name: debian:bookworm-slim\n  entrypoint: bash\n  args: [-c, "%s /workspace"]\noptions:\n  machineType: E2_HIGHCPU_32\n' "$cmd" > "$ctx/cloudbuild.yaml"
  out="$(CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}" gcloud builds submit "$ctx" --project "$project" --region "$region" \
    --config "$ctx/cloudbuild.yaml" --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
    --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET 2>&1)" && rc=0 || rc=$?
  grep -E '^(lexer|sources|parser|checker): |^[<>] ' <<< "$out" || true
  exit $rc
fi
