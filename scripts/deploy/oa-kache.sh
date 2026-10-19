#!/bin/sh
# The shared remote compile cache (#12176) inside a deploy image build
# (#12177). A Dockerfile copies this script in and calls:
#
#   oa-kache install       # the pinned kache, and its config naming the bucket
#   oa-kache run CMD...    # CMD with kache as Cargo's rustc wrapper
#
# `run` starts kache's daemon, runs CMD, uploads what CMD compiled, and
# stops the daemon. kache's own files live on a cache mount at
# /var/cache/kache, so the layer holds only what CMD wrote. The RUN needs
# `--network=host` (and the build `--allow network.host`, which
# scripts/deploy/cloud-buildx.sh passes) so kache reaches the Cloud Build
# machine's metadata server for the build account's credentials. kache keys
# each crate by the content of its inputs, so a hit is an earlier compile of
# the same inputs, on this or another Linux builder
# (docs/coder/runtime/remote-build-cache.md). A cache error compiles the
# crate locally; it never fails the build. OA_BUILD_CACHE=0 in CMD's
# environment turns the cache off.
#
# The kache version and checksum match scripts/dev/remote-build-cache.sh;
# upgrade them together.
set -eu

KACHE_VERSION=0.28.1
KACHE_SHA256=68635a9f92ce5015a2c48c98b968d1b3707b6dbee0ca725e51d1671c9346f38f
BUCKET=${OA_BUILD_CACHE_BUCKET:-openagentsgemini-rust-build-cache}
CONFIG=/etc/kache/config.toml

case ${1:-} in
    install)
        tmp=$(mktemp -d)
        name=kache-x86_64-unknown-linux-musl.tar.gz
        curl -fsSL -o "$tmp/$name" \
            "https://github.com/kunobi-ninja/kache/releases/download/v$KACHE_VERSION/$name"
        echo "$KACHE_SHA256  $tmp/$name" | sha256sum -c - > /dev/null
        tar -xzf "$tmp/$name" -C /usr/local/bin kache
        rm -rf "$tmp"
        mkdir -p "$(dirname "$CONFIG")"
        printf '[cache.remote]\ntype = "gcs"\nbucket = "%s"\nprefix = "artifacts"\n' "$BUCKET" > "$CONFIG"
        ;;
    run)
        shift
        if [ "${OA_BUILD_CACHE:-1}" = 0 ]; then exec "$@"; fi
        export KACHE_CONFIG=$CONFIG KACHE_CACHE_DIR=${KACHE_CACHE_DIR:-/var/cache/kache}
        export RUSTC_WRAPPER=/usr/local/bin/kache
        kache daemon start > /dev/null 2>&1 || true
        # The first compile must find the daemon, or it misses the remote.
        i=0
        while [ $i -lt 50 ] && ! kache daemon status --json 2> /dev/null | grep -q '"daemon_running": true'; do
            sleep 0.1
            i=$((i + 1))
        done
        status=0
        "$@" || status=$?
        kache stats 2> /dev/null | grep -E '^(Hit rate|Time saved):' || true
        # Upload what CMD compiled before the step ends: stopping the daemon
        # gives its queue only a short drain.
        started=$(date +%s)
        if [ $status -eq 0 ]; then kache sync --push 2>&1 | grep -E '^ *(Plan|Uploaded|Nothing)' || true; fi
        kache daemon stop > /dev/null 2>&1 || true
        echo "oa-kache: uploads finished in $(( $(date +%s) - started )) s"
        exit $status
        ;;
    *)
        echo "usage: oa-kache install | run CMD..." >&2
        exit 2
        ;;
esac
