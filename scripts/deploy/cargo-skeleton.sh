#!/bin/sh
# Writes the dependency-only copy of a Cargo build context (#12177):
#
#   scripts/deploy/cargo-skeleton.sh SOURCE DEST [WORKSPACE]
#
# WORKSPACE is the directory, relative to SOURCE, of the workspace the
# Dockerfile builds (default: SOURCE itself). DEST gets the manifests of the
# packages that workspace loads (its members and, transitively, every local
# path dependency, `[patch]` path, and path a package inherits with
# `workspace = true` from another workspace), the manifests of the workspaces those
# packages inherit fields from, that workspace's Cargo.lock, the Cargo
# configuration files above it, and an empty stand-in for each target root a
# manifest can name: src/lib.rs, src/main.rs, src/bin/*, examples/*,
# build.rs, each named test, bench, or example, and any `path = "....rs"` or
# `build = "....rs"` the manifest sets.
#
# A deploy Dockerfile builds its exact `cargo build` commands against DEST
# first, so the compiled dependencies form an image layer that changes only
# when one of those manifests, the lock file, or the set of targets changes.
# A source-only change, or a manifest change in a crate the build doesn't
# load, reuses that layer and compiles only the workspace's own crates.
#
# Every file in DEST gets one fixed modification time, so the layer's
# checksum depends on content alone. This is the role cargo-chef's
# `prepare` plays; it is a script because cargo-chef leaves out path
# dependencies outside the workspace (apps/openagents/crates, vendor/).
set -eu

src=$(cd "${1:?SOURCE}" && pwd -P)
dest=${2:?DEST}
ws=${3:-.}
mkdir -p "$dest"
dest=$(cd "$dest" && pwd -P)
cd "$src"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
: > "$work/packages"
: > "$work/queue"

rel() { # an existing directory, as a path relative to SOURCE, or nothing
    d=$(cd "$1" 2> /dev/null && pwd -P) || return 0
    case $d in
        "$src") echo . ;;
        "$src"/*) echo "${d#"$src"/}" ;;
    esac
}

# The quoted strings of a `KEY = [ ... ]` array in the [workspace] table.
workspace_list() { # MANIFEST KEY
    awk -v key="$2" '
        /^\[/ { inws = ($0 ~ /^\[workspace\][[:space:]]*$/) }
        inws && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { grab = 1 }
        grab {
            line = $0
            sub(/#.*/, "", line)
            while (match(line, /"[^"]*"/)) {
                print substr(line, RSTART + 1, RLENGTH - 2)
                line = substr(line, RSTART + RLENGTH)
            }
            if ($0 ~ /\]/) grab = 0
        }' "$1"
}

enqueue() { # DIR relative to SOURCE
    [ -n "$1" ] && [ -f "$1/Cargo.toml" ] || return 0
    grep -qxF "$1" "$work/packages" && return 0
    echo "$1" >> "$work/packages"
    echo "$1" >> "$work/queue"
}

# The workspace's own package, then its members, less its exclusions.
if grep -q '^\[package\]' "$ws/Cargo.toml"; then enqueue "$(rel "$ws")"; fi
workspace_list "$ws/Cargo.toml" exclude | while IFS= read -r e; do
    rel "$ws/$e"
done > "$work/exclude"
for m in $(workspace_list "$ws/Cargo.toml" members); do
    for d in "$ws"/$m; do
        [ -f "$d/Cargo.toml" ] || continue
        r=$(rel "$d")
        grep -qxF "$r" "$work/exclude" || enqueue "$r"
    done
done

# The nearest workspace above a package directory, or nothing.
workspace_of() { # DIR relative to SOURCE
    up=$1
    while [ "$up" != . ]; do
        up=$(dirname "$up")
        if [ -f "$up/Cargo.toml" ] && grep -q '^\[workspace\]' "$up/Cargo.toml"; then
            echo "$up"
            return
        fi
    done
}

# The paths of the `name.workspace = true` dependencies a package takes
# from its workspace's [workspace.dependencies], relative to that workspace.
inherited_paths() { # PACKAGE-MANIFEST WORKSPACE-MANIFEST
    sed -nE 's/^[[:space:]]*([A-Za-z0-9_-]+)([[:space:]]*\.[[:space:]]*workspace[[:space:]]*=[[:space:]]*true|[[:space:]]*=[[:space:]]*\{.*workspace[[:space:]]*=[[:space:]]*true).*/\1/p' "$1" |
        sort -u > "$work/names"
    awk 'NR == FNR { want[$0] = 1; next }
         /^\[/ { deps = ($0 ~ /^\[workspace\.dependencies\][[:space:]]*$/); next }
         deps {
             name = $0; sub(/^[[:space:]]*/, "", name); sub(/[[:space:]=.].*/, "", name)
             if (!(name in want)) next
             if (match($0, /path[[:space:]]*=[[:space:]]*"[^"]*"/)) {
                 p = substr($0, RSTART, RLENGTH); sub(/^[^"]*"/, "", p); sub(/"$/, "", p); print p
             }
         }' "$work/names" "$2"
}

# The workspace manifest's own paths: [workspace.dependencies] and [patch].
sed -nE 's/.*path[[:space:]]*=[[:space:]]*"([^"]*)".*/\1/p' "$ws/Cargo.toml" |
    { grep -v '\.rs$' || true; } | while IFS= read -r p; do
        enqueue "$(rel "$ws/$p")"
    done

# Every local path the loaded manifests name, transitively. Dependency
# paths name directories; a `.rs` path is a target.
while [ -s "$work/queue" ]; do
    dir=$(head -n 1 "$work/queue")
    tail -n +2 "$work/queue" > "$work/queue.next"
    mv "$work/queue.next" "$work/queue"
    sed -nE 's/.*path[[:space:]]*=[[:space:]]*"([^"]*)".*/\1/p' "$dir/Cargo.toml" |
        { grep -v '\.rs$' || true; } | while IFS= read -r p; do
            enqueue "$(rel "$dir/$p")"
        done
    root=$(workspace_of "$dir")
    if [ -n "$root" ] && [ "$root" != "$ws" ]; then
        inherited_paths "$dir/Cargo.toml" "$root/Cargo.toml" | while IFS= read -r p; do
            enqueue "$(rel "$root/$p")"
        done
    fi
done

stub() { # PATH: an empty library, or a main for a binary or build script
    mkdir -p "$dest/$(dirname "$1")"
    case "$1" in
        */lib.rs) : > "$dest/$1" ;;
        *) printf 'fn main() {}\n' > "$dest/$1" ;;
    esac
}

copy() { # FILE relative to SOURCE
    mkdir -p "$dest/$(dirname "$1")"
    cp "$1" "$dest/$1"
}

while IFS= read -r dir; do
    manifest=$dir/Cargo.toml
    copy "$manifest"
    for root in "$dir/src/lib.rs" "$dir/src/main.rs" "$dir/build.rs" \
        "$dir"/src/bin/*.rs "$dir"/src/bin/*/main.rs "$dir"/examples/*.rs \
        "$dir"/examples/*/main.rs; do
        if [ -f "$root" ]; then stub "$root"; fi
    done
    # Explicit target and build-script paths, including inline tables
    # (`lib = { path = "x.rs" }`).
    sed -nE 's/.*(path|build)[[:space:]]*=[[:space:]]*"([^"]*\.rs)".*/\2/p' "$manifest" |
        while IFS= read -r path; do
            if [ -f "$dir/$path" ]; then stub "$dir/$path"; fi
        done
    # A named `[[test]]`, `[[bench]]`, or `[[example]]` without a path
    # must still find its file at the default location.
    awk '/^\[\[(test|bench|example)\]\]/ { kind = $0; gsub(/[][]/, "", kind); next }
         /^\[/ { kind = "" }
         kind != "" && /^[[:space:]]*name[[:space:]]*=/ {
             n = $0; sub(/^[^"]*"/, "", n); sub(/".*/, "", n)
             d = (kind == "test") ? "tests" : (kind == "bench") ? "benches" : "examples"
             print d "/" n ".rs"; print d "/" n "/main.rs" }' "$manifest" |
        while IFS= read -r path; do
            if [ -f "$dir/$path" ]; then stub "$dir/$path"; fi
        done
    # The nearest enclosing workspace, for inherited fields: its manifest
    # only, not its members.
    root=$(workspace_of "$dir")
    if [ -n "$root" ]; then copy "$root/Cargo.toml"; fi
done < "$work/packages"

copy "$ws/Cargo.toml"
copy "$ws/Cargo.lock"
# Cargo reads .cargo/config.toml from the workspace directory upward.
up=$ws
while :; do
    for c in "$up/.cargo/config.toml" "$up/.cargo/config"; do
        if [ -f "$c" ]; then copy "$c"; fi
    done
    [ "$up" = . ] && break
    up=$(dirname "$up")
done

find "$dest" -exec touch -h -t 198001010000 {} +
