#!/usr/bin/env bash
# Builds and runs the old Rust relay (crates/nostr-relay at 0b401561a0^, what
# relay.openagents.com runs) inside a bubblewrap sandbox, for the R1
# differential (coax/tests/nostr/relay_rust_differential.py). Owner-approved
# 2026-10-29: no network beyond fetching its pinned crates, never production
# data, never relay.openagents.com.
#
#   coax/tests/nostr/rust_relay_sandbox.sh extract   # git archive into SCRATCH (no network)
#   coax/tests/nostr/rust_relay_sandbox.sh fetch     # cargo fetch: the only networked step
#   coax/tests/nostr/rust_relay_sandbox.sh build     # cargo build --offline, network off
#   coax/tests/nostr/rust_relay_sandbox.sh run CMD.. # a throwaway Postgres and the relay,
#                                                    # network off (loopback only), then CMD
#   coax/tests/nostr/rust_relay_sandbox.sh all       # extract, fetch, build
#
# SCRATCH (default ${TMPDIR:-/tmp}/coax-rust-relay) must be outside the
# repository. The extracted code is untrusted: every cargo and relay step runs
# in bwrap with only SCRATCH writable, the repository not mounted for the
# build and read-only for `run`, and ~/work, ~/.ssh and the rest of $HOME not
# mounted. Build scripts of crates run inside that sandbox too.
#
# Inputs: RUST_TOOLCHAIN (default ~/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu,
# the crate's rust-toolchain.toml), POSTGRES (a nix store path; default
# `nix build nixpkgs#postgresql`), BWRAP (default /run/current-system/sw/bin/bwrap).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
rev="0b401561a0^"
scratch="${SCRATCH:-${TMPDIR:-/tmp}/coax-rust-relay}"
toolchain="${RUST_TOOLCHAIN:-$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu}"
bwrap="${BWRAP:-/run/current-system/sw/bin/bwrap}"
cacert="$(readlink -f /etc/ssl/certs/ca-certificates.crt)"
mkdir -p "$scratch"
scratch="$(cd "$scratch" && pwd -P)"
case "$scratch/" in "$root"/*) echo "SCRATCH must be outside the repository" >&2; exit 1;; esac
src="$scratch/src"

# The sandbox: /nix and the system profile read-only, SCRATCH writable, a
# private /tmp, a clean environment. NET=1 keeps the network (fetch only).
sandbox() {
    local net=(--unshare-net)
    if [[ "${NET:-0}" == 1 ]]; then net=(--share-net --ro-bind /etc/resolv.conf /etc/resolv.conf); fi
    local extra=()
    [[ -d /lib64 ]] && extra+=(--ro-bind /lib64 /lib64)
    "$bwrap" --unshare-all "${net[@]}" --die-with-parent --new-session \
        --ro-bind /nix /nix --ro-bind /run/current-system /run/current-system \
        --ro-bind /bin /bin --ro-bind /usr/bin /usr/bin "${extra[@]}" \
        --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group --ro-bind /etc/hosts /etc/hosts \
        --ro-bind "$toolchain" "$toolchain" \
        --proc /proc --dev /dev --tmpfs /tmp \
        --bind "$scratch" "$scratch" "${SANDBOX_BINDS[@]}" \
        --chdir "$src" --clearenv \
        --setenv PATH "$toolchain/bin:/run/current-system/sw/bin:${POSTGRES:-/nonexistent}/bin" \
        --setenv HOME "$scratch/home" --setenv CARGO_HOME "$scratch/cargo-home" \
        --setenv CARGO_TARGET_DIR "$scratch/target" --setenv TMPDIR /tmp \
        --setenv SSL_CERT_FILE "$cacert" --setenv CARGO_HTTP_CAINFO "$cacert" \
        --setenv CARGO_NET_GIT_FETCH_WITH_CLI false --setenv RUSTUP_TOOLCHAIN none \
        "${SANDBOX_ENV[@]}" "$@"
}
SANDBOX_BINDS=()
SANDBOX_ENV=()

extract() {
    rm -rf "$src"; mkdir -p "$src" "$scratch/home" "$scratch/cargo-home"
    (cd "$root" && git archive "$rev" crates/nostr-relay crates/nostr crates/agent-contract migrations nips tests/fixtures Cargo.lock) \
        | tar -x -C "$src"
    git -C "$root" show "$rev:Cargo.lock" > "$scratch/Cargo.lock.orig"
    # The old workspace had ~1900 packages. A minimal one: the relay, its two
    # path crates, and an empty stand-in for the hakari workspace-hack (it
    # only unified features across the old workspace).
    mkdir -p "$src/crates/workspace-hack/src"
    : > "$src/crates/workspace-hack/src/lib.rs"
    cat > "$src/crates/workspace-hack/Cargo.toml" <<'EOF'
[package]
name = "openagents-workspace-hack"
version = "0.1.0"
edition = "2021"
publish = false
EOF
    cat > "$src/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/nostr-relay", "crates/nostr", "crates/agent-contract"]
resolver = "2"

[workspace.package]
version = "1.0.0"
publish = false
edition = "2024"
rust-version = "1.97.1"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }

[patch.crates-io]
openagents-workspace-hack = { path = "crates/workspace-hack" }
EOF
    echo "extracted $rev into $src"
}

# Every package the pruned lock keeps must have the version and checksum it
# had in the old workspace's Cargo.lock: the crates are the pinned ones.
check_lock() {
    python3 -I - "$scratch/Cargo.lock.orig" "$src/Cargo.lock" <<'EOF'
import sys, tomllib
def pk(p): return {(x['name'], x['version']): x.get('checksum') for x in tomllib.load(open(p, 'rb'))['package']}
old, new = pk(sys.argv[1]), pk(sys.argv[2])
bad = [k for k, v in new.items() if k[0] != 'openagents-workspace-hack' and (k not in old or old[k] != v)]
print('lock: %d packages, all pinned by the old Cargo.lock' % len(new) if not bad else 'lock: NOT PINNED %r' % bad)
sys.exit(1 if bad else 0)
EOF
}

fetch() {
    # The only networked step. cargo fetch downloads sources; it runs no build
    # script. --locked is not possible: the minimal workspace prunes the old
    # lock (and drops the stand-in's dependencies), so cargo resolves once with
    # the old lock as its pin and check_lock proves nothing was re-resolved.
    NET=1 sandbox cargo fetch
    check_lock
    NET=1 sandbox cargo fetch --locked
}

build() {
    sandbox cargo build --offline --locked --release --bin nostr-relay -p nostr-relay -j "${JOBS:-8}"
    ls -la "$scratch/target/release/nostr-relay"
}

# A throwaway Postgres in SCRATCH/pg (Unix socket in the private /tmp, TCP on
# 127.0.0.1 only), then CMD inside the same network-less sandbox. The
# repository is read-only inside; RUN_BINDS adds more read-only paths.
run() {
    : "${POSTGRES:=$(nix --extra-experimental-features 'nix-command flakes' build --no-link --print-out-paths nixpkgs#postgresql | grep -v -- '-man$' | head -1)}"
    export POSTGRES
    SANDBOX_BINDS=(--ro-bind "$root" "$root")
    for p in ${RUN_BINDS:-}; do SANDBOX_BINDS+=(--ro-bind "$p" "$p"); done
    SANDBOX_ENV=(--setenv RUST_RELAY "$scratch/target/release/nostr-relay" --setenv PGDATA "$scratch/pg"
                 --setenv PGHOST /tmp/pgsock --setenv PGPORT 54329 --setenv PGUSER relay --setenv PGDATABASE relay)
    for v in ${RUN_ENV:-}; do SANDBOX_ENV+=(--setenv "${v%%=*}" "${v#*=}"); done
    sandbox bash -c '
        set -euo pipefail
        rm -rf "$PGDATA" "$PGHOST"; mkdir -p "$PGHOST"
        initdb -A trust -U relay -D "$PGDATA" >/dev/null
        pg_ctl -D "$PGDATA" -l "$PGDATA.log" -o "-k $PGHOST -h 127.0.0.1 -p $PGPORT -c fsync=off" -w start >/dev/null
        trap "pg_ctl -D \"\$PGDATA\" -m immediate stop >/dev/null" EXIT
        createdb relay
        export DATABASE_URL="postgres://relay@127.0.0.1:$PGPORT/relay"
        "$@"' run "$@"
}

case "${1:-all}" in
    extract) extract ;;
    fetch) fetch ;;
    build) build ;;
    run) shift; run "$@" ;;
    all) extract; fetch; build ;;
    *) echo "usage: $0 extract|fetch|build|run CMD...|all" >&2; exit 2 ;;
esac
