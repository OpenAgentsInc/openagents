# Source for staging-only fast builds of the browser modules (#12435):
#
#   . scripts/deploy/staging-build-env.sh
#   scripts/build-pylons-web.sh OUTDIR      # or any module build script
#
# Every release-style profile builds without LTO, with 16 codegen units and
# incrementally (Cargo's CARGO_PROFILE_<NAME>_* overrides), `wasm-opt` copies
# its input, brotli runs at quality 5, and gzip at level 6, through shims
# first on PATH. A one-line edit to pylons-web then rebuilds in about 0.7 s
# and everglade-web in about 5 s, against 3 s and 2.5 minutes with the
# production settings, which production promotion keeps. The modules come
# out larger (everglade-web 29 MB against 25 MB).

for oa_profile in RELEASE VERSE_WEB GUEST LIVE_WASM WASM_TINY CAD_WEB KILN_WEB; do
  export "CARGO_PROFILE_${oa_profile}_LTO=false"
  export "CARGO_PROFILE_${oa_profile}_CODEGEN_UNITS=16"
  export "CARGO_PROFILE_${oa_profile}_INCREMENTAL=true"
done
unset oa_profile
OA_STAGING_SHIMS="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/staging-shims" && pwd)"
case ":$PATH:" in
  *":$OA_STAGING_SHIMS:"*) ;;
  *) PATH="$OA_STAGING_SHIMS:$PATH" ;;
esac
export PATH OA_STAGING_SHIMS
export OA_STAGING_BUILD=1
