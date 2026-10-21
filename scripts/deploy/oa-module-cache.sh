#!/bin/sh
# Reuses a browser module's built files when none of its inputs changed
# (#12427). Inside a deploy image build, in place of the build command:
#
#   oa-module-cache NAME OUTDIR[,OUTDIR...] -- CMD...
#
# The key has two parts. The first, the recipe, digests NAME, CMD, the
# compilers and tools, the root and GPUI web workspaces' Cargo.lock files,
# Cargo's configuration, and every build script the context carries
# (scripts/ and apps/openagents/scripts/). The second digests the module's source files:
# the files Cargo's dep-info listed for the last build of this recipe, each
# widened to the whole package directory it belongs to, so a new file,
# dependency, or include in a package changes the key. When both match a
# stored build, its OUTDIRs are restored and CMD doesn't run. Otherwise CMD
# runs, and its OUTDIRs and its fresh input list are stored for the next
# build.
#
# Each build or reuse also stores the module's input list as
# web-modules/NAME/inputs-latest. OA_MODULE_RECORD=1 runs CMD and stores
# only that list, for a build that is never reused (the server's).
#
# OA_STAGING_BUILD=1 builds with scripts/deploy/staging-build-env.sh's
# staging-only settings, under a recipe of their own.
#
# Builds are stored in the shared compile cache's bucket under
# web-modules/NAME/, through the Cloud Build machine's metadata server
# (the RUN needs --network=host, as for scripts/deploy/oa-kache.sh). Any
# cache error runs CMD; it never fails the build. OA_MODULE_CACHE=0 always
# runs CMD and stores nothing; OA_MODULE_CACHE_DIR stores builds in a local
# directory instead of the bucket (for tests). CMD must touch every source file first, as
# the Dockerfile's `find . -exec touch` does, so Cargo writes fresh dep-info.
set -eu

BUCKET=${OA_BUILD_CACHE_BUCKET:-openagentsgemini-rust-build-cache}
ROOT=${OA_MODULE_ROOT:-/build}
METADATA=http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token

name=${1:?usage: oa-module-cache NAME OUTDIR[,OUTDIR...] -- CMD...}
outdirs=${2:?usage: oa-module-cache NAME OUTDIR[,OUTDIR...] -- CMD...}
shift 2
[ "${1:-}" = -- ] || { echo "usage: oa-module-cache NAME OUTDIR[,OUTDIR...] -- CMD..." >&2; exit 2; }
shift
[ $# -gt 0 ] || { echo "oa-module-cache: no command" >&2; exit 2; }

command -v sha256sum > /dev/null 2>&1 || sha256sum() { shasum -a 256 "$@"; }

say() { printf 'oa-module-cache %s: %s\n' "$name" "$*" >&2; }

if [ "${OA_MODULE_CACHE:-1}" = 0 ]; then exec "$@"; fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
started=$(date +%s)

token() {
    [ -s "$work/token" ] && { cat "$work/token"; return 0; }
    curl -fsS -m 10 -H 'Metadata-Flavor: Google' "$METADATA" 2> /dev/null |
        sed -n 's/.*"access_token" *: *"\([^"]*\)".*/\1/p' > "$work/token" || return 1
    [ -s "$work/token" ] && cat "$work/token"
}

object_url() { # OBJECT
    printf 'https://storage.googleapis.com/storage/v1/b/%s/o/%s' "$BUCKET" \
        "$(printf '%s' "$1" | sed 's|/|%2F|g')"
}

fetch() { # OBJECT FILE
    if [ -n "${OA_MODULE_CACHE_DIR:-}" ]; then cp "$OA_MODULE_CACHE_DIR/$1" "$2" 2> /dev/null; return; fi
    t=$(token) || return 1
    curl -fsS -m 300 -H "Authorization: Bearer $t" -o "$2" "$(object_url "$1")?alt=media" 2> /dev/null
}

store() { # FILE OBJECT
    if [ -n "${OA_MODULE_CACHE_DIR:-}" ]; then
        mkdir -p "$OA_MODULE_CACHE_DIR/$(dirname "$2")" && cp "$1" "$OA_MODULE_CACHE_DIR/$2"
        return
    fi
    t=$(token) || return 1
    curl -fsS -m 600 -X POST -H "Authorization: Bearer $t" \
        -H 'Content-Type: application/octet-stream' --data-binary "@$1" \
        "https://storage.googleapis.com/upload/storage/v1/b/$BUCKET/o?uploadType=media&name=$(printf '%s' "$2" | sed 's|/|%2F|g')" \
        > /dev/null 2>&1
}

# The recipe: everything but the module's own sources.
{
    printf 'name %s\n' "$name"
    printf 'outdirs %s\n' "$outdirs"
    printf 'arg %s\n' "$@"
    printf 'staging %s\n' "${OA_STAGING_BUILD:-0}"
    for tool in rustc cargo wasm-bindgen wasm-opt brotli gzip clang; do
        if command -v "$tool" > /dev/null 2>&1; then
            printf 'tool %s %s\n' "$tool" "$("$tool" --version 2>&1 | head -n 3 | tr '\n' ' ')"
        fi
    done
    rustc -vV 2> /dev/null || true
    env | grep -E '^(CC_|AR_|CFLAGS|RUSTFLAGS|CARGO_|PROTOC|OPENAGENTS_|WEB_BUILD_)' | grep -v '^CARGO_HOME=' | sort
    # The lock files of the two workspaces modules build in, and Cargo's
    # configuration.
    for f in Cargo.lock apps/openagents/web/Cargo.lock .cargo/config.toml \
        apps/openagents/.cargo/config.toml apps/openagents/web/.cargo/config.toml; do
        if [ -f "$ROOT/$f" ]; then (cd "$ROOT" && sha256sum "$f"); fi
    done
    # The image build's own plumbing doesn't change a module's files.
    for d in scripts apps/openagents/scripts; do
        if [ -d "$ROOT/$d" ]; then
            (cd "$ROOT" && find "$d" -type f ! -path scripts/deploy/cloud-buildx.sh \
                ! -path scripts/deploy/cargo-skeleton.sh ! -path scripts/deploy/oa-kache.sh |
                LC_ALL=C sort | xargs sha256sum)
        fi
    done
} > "$work/recipe"
recipe=$(sha256sum < "$work/recipe" | cut -c1-40)
prefix=web-modules/$name/$recipe

# The nearest package directory above PATH (relative to ROOT), or PATH
# itself when none is below the workspace root.
widen() {
    p=$1
    d=$p
    [ -d "$ROOT/$d" ] || d=$(dirname "$d")
    while [ "$d" != . ] && [ "$d" != / ]; do
        if [ -f "$ROOT/$d/Cargo.toml" ] && grep -q '^\[package\]' "$ROOT/$d/Cargo.toml"; then
            echo "$d"
            return
        fi
        d=$(dirname "$d")
    done
    echo "$p"
}

# The digest of the files under each listed path (a file or a directory),
# with a line for each listed path that doesn't exist. With BEFORE (the
# sorted list of paths that existed before the build), files the build
# wrote into the source tree are left out (pylon-card's patched wgpu, for
# one): the next build looks its key up before they exist.
digest_inputs() { # LIST [BEFORE]
    while IFS= read -r p; do
        if [ -d "$ROOT/$p" ]; then
            (cd "$ROOT" && find "$p" -path '*/target' -prune -o -type f -print)
        elif [ -f "$ROOT/$p" ]; then
            echo "$p"
        else
            echo "missing $p" >> "$work/missing"
        fi
    done < "$1" | LC_ALL=C sort -u > "$work/files"
    if [ -n "${2:-}" ]; then
        LC_ALL=C comm -12 "$work/files" "$2" > "$work/files.before"
        mv "$work/files.before" "$work/files"
    fi
    {
        (cd "$ROOT" && tr '\n' '\0' < "$work/files" | xargs -0 sha256sum)
        cat "$work/missing" 2> /dev/null || true
    } | sha256sum | cut -c1-40
}

# A stored build for this recipe and these sources: restore it. With
# OA_MODULE_RECORD=1 (the server's build) nothing is restored or stored but
# the input list.
if [ "${OA_MODULE_RECORD:-}" != 1 ] && fetch "$prefix/inputs" "$work/inputs" && [ -s "$work/inputs" ]; then
    : > "$work/missing"
    inputs=$(digest_inputs "$work/inputs")
    if fetch "$prefix/$inputs.tar.gz" "$work/build.tar.gz"; then
        if tar -xzf "$work/build.tar.gz" -C / 2> /dev/null; then
            store "$work/inputs" "web-modules/$name/inputs-latest" || true
            say "reused build $recipe/$inputs ($(( $(date +%s) - started )) s)"
            exit 0
        fi
        say "the stored build $recipe/$inputs did not unpack; building"
    fi
fi

# Build, then store the build and the inputs Cargo reported.
(cd "$ROOT" && find . -path ./target -prune -o -path ./target-gpuiweb -prune -o -print |
    sed 's|^\./||' | LC_ALL=C sort) > "$work/before"
: > "$work/stamp"
sleep 1
if [ "${OA_STAGING_BUILD:-0}" = 1 ]; then
    # Staging-only settings (scripts/deploy/staging-build-env.sh, #12435):
    # no LTO, quick compression. Their recipe differs, so these builds never
    # stand in for a production one.
    bash -c '. "$1" && shift && exec "$@"' _ "$ROOT/scripts/deploy/staging-build-env.sh" "$@"
else
    "$@"
fi
built=$(date +%s)
find "$ROOT" ${CARGO_TARGET_DIR:+"$CARGO_TARGET_DIR"} -name '*.d' -type f -newer "$work/stamp" 2> /dev/null |
    awk -F/ '{ p = $(NF-1); if (p == "release" || p == "debug" || p == "verse-web" || p == "guest" || p == "wasm-tiny" || p == "cad-web" || p == "capture") print }' \
    > "$work/depinfo"
if [ ! -s "$work/depinfo" ]; then
    say "built in $(( built - started )) s; Cargo wrote no dep-info, so the build is not stored"
    exit 0
fi
root_real=$(cd "$ROOT" && pwd -P)
xargs cat < "$work/depinfo" | sed 's/\\$//' | tr ' ' '\n' | sed -n 's/:$//; /^\//p' | sort -u |
    while IFS= read -r p; do
        if [ -d "$p" ]; then r=$(cd "$p" && pwd -P)
        elif [ -f "$p" ]; then r=$(cd "$(dirname "$p")" && pwd -P)/$(basename "$p")
        else continue
        fi
        case $r in
            "$root_real"/target/* | "$root_real"/target-*/*) ;;
            "$root_real"/*) widen "${r#"$root_real"/}" ;;
        esac
    done | LC_ALL=C sort -u | LC_ALL=C comm -12 - "$work/before" > "$work/inputs"
if [ ! -s "$work/inputs" ]; then
    say "built in $(( built - started )) s; no source inputs found, so the build is not stored"
    exit 0
fi
# The newest input list for each name, which scripts/deploy/web.sh reads to
# tell a module change from a server change.
store "$work/inputs" "web-modules/$name/inputs-latest" || true
if [ "${OA_MODULE_RECORD:-}" = 1 ]; then
    say "built in $(( built - started )) s; recorded $(wc -l < "$work/inputs" | tr -d " ") input paths"
    exit 0
fi
: > "$work/missing"
inputs=$(digest_inputs "$work/inputs" "$work/before")
dirs=$(printf '%s' "$outdirs" | tr ',' ' ')
# shellcheck disable=SC2086
if tar -czf "$work/build.tar.gz" -C / $(for d in $dirs; do printf '%s ' "${d#/}"; done) &&
    store "$work/build.tar.gz" "$prefix/$inputs.tar.gz" &&
    store "$work/inputs" "$prefix/inputs"; then
    say "built in $(( built - started )) s and stored $recipe/$inputs ($(wc -l < "$work/inputs" | tr -d " ") input paths)"
else
    say "built in $(( built - started )) s; storing it failed, the next build compiles it again"
fi
