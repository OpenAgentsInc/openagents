#!/bin/sh
# Run the dependency release gate from any working directory.
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

# cargo-deny does not expire advisory exceptions. Refuse an overdue review.
if [ "$(date -u +%Y%m%d)" -ge 20261020 ]; then
    echo 'RUSTSEC-2024-0436 requires a new maintainer review; see docs/dependencies.md.' >&2
    exit 1
fi

# Scope the exception to the exact dependency version reviewed here.
paste_version=$(cargo +1.97.1 tree --locked --workspace --all-features -i paste --depth 0 --prefix none)
if [ "$paste_version" != 'paste v1.0.15 (proc-macro)' ]; then
    echo 'The reviewed paste dependency changed. Revisit its advisory exception.' >&2
    exit 1
fi
cargo +1.97.1 deny --locked check advisories licenses sources
