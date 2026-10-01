#!/bin/sh
# Install OpenAgents Terminal: the `openagents` command, and the `microcoder`
# engine it runs Coder with.
#
#   curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh
#   curl -fsSL .../openagents/install.sh | sh -s 1.0.0-rc.1
#   curl -fsSL .../openagents/install.sh | OPENAGENTS_CHANNEL=rc sh
#
# POSIX sh, not bash: minimal images such as Alpine have no bash.
#
# Environment:
#   OPENAGENTS_CHANNEL    The channel to follow: `stable` or `rc`. Unset, it
#                         follows `stable`, and `rc` while no stable release
#                         exists yet.
#   OPENAGENTS_BIN_DIR    Where both programs go. Default $HOME/.openagents/bin.
#   OPENAGENTS_BASE_URL   Where releases are read from. Default the release
#                         bucket's public URL.
#   OPENAGENTS_NO_LAUNCH  Set to 1 to not start OpenAgents Terminal after
#                         installing, when a terminal is attached.
#
# It reads, under the base URL (scripts/release/terminal.sh writes them):
#   openagents.<channel>                  the version, one line
#   openagents-<version>-<platform>       `openagents`
#   microcoder-<version>-<platform>       `microcoder`
#   SHA256SUMS-openagents-<version>       the digests
# and installs nothing whose SHA-256 does not match.

set -eu

BASE_URL="${OPENAGENTS_BASE_URL:-https://storage.googleapis.com/openagentsgemini-cli-releases/openagents}"
BASE_URL="${BASE_URL%/}"
BIN_DIR="${OPENAGENTS_BIN_DIR:-$HOME/.openagents/bin}"
PRODUCT="openagents"
ENGINE="microcoder"
TARGET="${1:-}"

say() { echo "$@" >&2; }
die() {
    say "$@"
    exit 1
}

is_version() {
    printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-rc\.(0|[1-9][0-9]*))?$'
}

# Fetches $1 to the file $2, or to standard output when $2 is empty.
download() {
    if command -v curl >/dev/null 2>&1; then
        if [ -n "$2" ]; then curl -fsSL "$1" -o "$2"; else curl -fsSL "$1"; fi
    elif command -v wget >/dev/null 2>&1; then
        if [ -n "$2" ]; then wget -qO "$2" "$1"; else wget -qO - "$1"; fi
    else
        die "Neither curl nor wget is available."
    fi
}

sums_entry() {
    awk -v name="$2" '$2 == name || $2 == "*" name { print $1; exit }' "$1"
}

case "$(uname -s)" in
    Darwin) os=macos ;;
    Linux) os=linux ;;
    MINGW* | MSYS* | CYGWIN*) die "On Windows, install from PowerShell: irm $BASE_URL/install.ps1 | iex" ;;
    *) die "Unsupported operating system: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) die "Unsupported architecture: $(uname -m)" ;;
esac

# A shell under Rosetta reports x86_64 on Apple silicon; install the native
# build.
if [ "$os" = macos ] && [ "$arch" = x86_64 ] &&
    [ "$(sysctl -n hw.optional.arm64 2>/dev/null || :)" = 1 ]; then
    say "Apple silicon detected through Rosetta; installing the native arm64 build."
    arch=aarch64
fi

# Which libc a Linux machine has, decided by whether the glibc loader is on
# disk rather than by the distribution's name.
linux_libc() {
    case "$1" in
        x86_64) loaders="/lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /lib/x86_64-linux-gnu/ld-linux-x86-64.so.2" ;;
        *) loaders="/lib/ld-linux-aarch64.so.1 /lib64/ld-linux-aarch64.so.1 /lib/aarch64-linux-gnu/ld-linux-aarch64.so.1" ;;
    esac
    for loader in $loaders; do
        if [ -e "$loader" ]; then
            case "$(ldd --version 2>&1 || true)" in
                *musl*) echo musl ;;
                *) echo gnu ;;
            esac
            return
        fi
    done
    echo musl
}

platform="$os-$arch"
if [ "$os" = linux ] && [ "$(linux_libc "$arch")" = musl ]; then
    platform="$platform-musl"
    say "No glibc loader found; installing the statically linked (musl) build."
fi

pointer() {
    download "$BASE_URL/$PRODUCT.$1" "" 2>/dev/null | tr -d '[:space:]'
}

if [ -n "$TARGET" ]; then
    is_version "$TARGET" || die "Not a version: $TARGET (a version looks like 1.0.0 or 1.0.0-rc.1; set OPENAGENTS_CHANNEL to follow a channel)"
    version="$TARGET"
else
    channel="${OPENAGENTS_CHANNEL:-}"
    if [ -z "$channel" ]; then
        channel=stable
        version="$(pointer stable || :)"
        if [ -z "$version" ]; then
            channel=rc
            version="$(pointer rc || :)"
            [ -z "$version" ] || say "No stable release yet; following the rc channel."
        fi
    else
        printf '%s' "$channel" | grep -Eq '^[A-Za-z0-9_-]+$' || die "Not a channel name: $channel"
        version="$(pointer "$channel" || :)"
    fi
    [ -n "$version" ] ||
        die "Could not read the '$channel' channel from $BASE_URL/$PRODUCT.$channel. Check your connection, or pass a version: sh -s X.Y.Z"
    is_version "$version" || die "The '$channel' channel names something that is not a version: $version"
fi

mkdir -p "$BIN_DIR"
work="$(mktemp -d "$BIN_DIR/.install.XXXXXX")"
trap 'rm -rf "$work"' EXIT INT TERM

if command -v shasum >/dev/null 2>&1; then
    digest() { shasum -a 256 "$1" | awk '{ print $1 }'; }
elif command -v sha256sum >/dev/null 2>&1; then
    digest() { sha256sum "$1" | awk '{ print $1 }'; }
else
    die "Neither shasum nor sha256sum is available; refusing to install unverified bytes."
fi

say "Installing OpenAgents Terminal $version for $platform..."
sums="$work/SHA256SUMS"
download "$BASE_URL/SHA256SUMS-$PRODUCT-$version" "$sums" 2>/dev/null ||
    die "Version $version is not published (no $BASE_URL/SHA256SUMS-$PRODUCT-$version), or $BASE_URL is unreachable."

for name in "$PRODUCT" "$ENGINE"; do
    artifact="$name-$version-$platform"
    expected="$(sums_entry "$sums" "$artifact")"
    [ -n "$expected" ] || die "Version $version has no $platform build ($artifact is not in its checksum file)."
    download "$BASE_URL/$artifact" "$work/$name" 2>/dev/null ||
        die "Could not download $BASE_URL/$artifact, which version $version lists. Check your connection and run this again."
    actual="$(digest "$work/$name")"
    if [ "$actual" != "$expected" ]; then
        die "Checksum mismatch for $artifact: expected $expected, got $actual. Nothing was installed."
    fi
    chmod +x "$work/$name"
    say "  Verified $artifact (sha256 $actual)."
done

# Both are verified before either replaces what is there. `openagents` runs
# Coder through the `microcoder` beside it, so they move together.
mv -f "$work/$ENGINE" "$BIN_DIR/microcoder"
mv -f "$work/$PRODUCT" "$BIN_DIR/openagents"
say "  Installed $BIN_DIR/openagents and $BIN_DIR/microcoder"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        say ""
        say "$BIN_DIR is not on your PATH. Add it to your shell profile:"
        say "  export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac

say ""
say "Run it with: openagents"

# Start it when a terminal is attached. Under `curl | sh` the script arrives
# on standard input, so the program gets the terminal's own device instead.
if [ "${OPENAGENTS_NO_LAUNCH:-}" != 1 ] && [ -t 2 ]; then
    terminal="$(tty <&2 2>/dev/null || true)"
    if [ -n "$terminal" ] && [ -r "$terminal" ]; then
        rm -rf "$work"
        trap - EXIT INT TERM
        say "Starting OpenAgents Terminal..."
        exec "$BIN_DIR/openagents" terminal <"$terminal" >"$terminal"
    fi
fi
