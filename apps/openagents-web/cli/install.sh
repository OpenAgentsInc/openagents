#!/bin/sh
# Install Coder. Rerun to update.
# Reimplemented from the public Grok installer workflow and the existing
# OpenAgents release contract; no xAI authentication or backend code is used.
#
#   curl -fsSL https://openagents.com/cli/install.sh | bash
#   curl -fsSL https://openagents.com/cli/install.sh | bash -s -- 1.0.0
#   curl -fsSL https://openagents.com/cli/install.sh | bash -s -- rc
#
# Environment:
#   CODER_VERSION         Exact version; a positional version takes precedence.
#   CODER_CHANNEL         stable or rc. Without one, the installer follows
#                         stable, and rc until a stable release is published.
#   CODER_BIN_DIR         Default ~/.openagents/bin.
#   CODER_BASE_URL        Default the release bucket's /coder prefix.
#   CODER_NO_PATH_UPDATE  1 leaves shell profiles unchanged.
#
# Each version publishes one archive per platform, coder-VERSION-PLATFORM.tar.gz,
# holding the commands Coder installs (coder, the openagents command, and the
# microcoder engine Coder runs with), and SHA256SUMS-coder-VERSION. Versions up
# to 1.0.0-rc.5 published each command separately (NAME-VERSION-PLATFORM); a
# sums file without the archive selects that layout. The pointers coder.stable
# and coder.rc name immutable versions. All checksums and --version checks pass
# before any installed command changes.

set -eu

coder_base_url="${CODER_BASE_URL:-https://storage.googleapis.com/openagentsgemini-cli-releases/coder}"
coder_base_url="${coder_base_url%/}"
coder_bin_dir="${CODER_BIN_DIR:-${OPENAGENTS_HOME:-$HOME/.openagents}/bin}"
coder_target="${1:-${CODER_VERSION:-}}"
coder_channel="${CODER_CHANNEL:-}"
coder_commands='coder openagents microcoder'

say() { printf '%s\n' "$*" >&2; }
die() { say "coder installer: $*"; exit 1; }
usage() {
    cat <<'EOF'
Install Coder and the openagents command.
Usage: install.sh [VERSION | stable | rc]
Rerun without a version to install the latest stable release (or the latest
release candidate while there is no stable release); pass rc for candidates.
CODER_BIN_DIR changes the install directory; CODER_NO_PATH_UPDATE=1 skips PATH setup.
EOF
}

[ "$#" -le 1 ] || die "Expected at most one version or channel."
case "$coder_target" in
    --help | -h) usage; exit 0 ;;
    stable | rc) coder_channel="$coder_target"; coder_target='' ;;
esac

is_version() {
    printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-rc\.(0|[1-9][0-9]*))?$'
}

case "$coder_channel" in
    '' | stable | rc) ;;
    *) die "Unknown channel '$coder_channel'; choose stable or rc." ;;
esac
if [ -n "$coder_target" ]; then
    is_version "$coder_target" || die "Not a version: $coder_target (use X.Y.Z or X.Y.Z-rc.N)."
fi

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --connect-timeout 15 --max-time 600 --max-filesize 1073741824 "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --timeout=60 --tries=2 -O "$2" "$1"
    else
        die "Install curl or wget before running this installer."
    fi
}

case "$(uname -s)" in
    Darwin) coder_os=macos ;;
    Linux) coder_os=linux ;;
    MINGW* | MSYS* | CYGWIN*) die "Use PowerShell on Windows: irm https://openagents.com/cli/install.ps1 | iex" ;;
    *) die "Unsupported operating system: $(uname -s)." ;;
esac
case "$(uname -m)" in
    x86_64 | amd64 | AMD64) coder_arch=x86_64 ;;
    arm64 | aarch64 | ARM64) coder_arch=aarch64 ;;
    *) die "Unsupported architecture: $(uname -m)." ;;
esac
if [ "$coder_os" = macos ] && [ "$coder_arch" = x86_64 ]; then
    coder_sysctl="$(command -v sysctl || printf '%s' /usr/sbin/sysctl)"
    if [ "$("$coder_sysctl" -n hw.optional.arm64 2>/dev/null || :)" = 1 ]; then
        coder_arch=aarch64
        say "Apple silicon detected through Rosetta; installing the native build."
    fi
fi

coder_platform="$coder_os-$coder_arch"
if [ "$coder_os" = linux ]; then
    case "$coder_arch" in
        x86_64) coder_loaders='/lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /lib/x86_64-linux-gnu/ld-linux-x86-64.so.2' ;;
        aarch64) coder_loaders='/lib/ld-linux-aarch64.so.1 /lib64/ld-linux-aarch64.so.1 /lib/aarch64-linux-gnu/ld-linux-aarch64.so.1' ;;
    esac
    coder_libc=musl
    for coder_loader in $coder_loaders; do
        if [ -e "$coder_loader" ]; then coder_libc=gnu; break; fi
    done
    case "$(ldd --version 2>&1 || :)" in *musl*) coder_libc=musl ;; esac
    if [ "$coder_libc" = musl ]; then coder_platform="$coder_platform-musl"; fi
fi

if command -v shasum >/dev/null 2>&1; then
    digest() { shasum -a 256 "$1" | awk '{ print $1 }'; }
elif command -v sha256sum >/dev/null 2>&1; then
    digest() { sha256sum "$1" | awk '{ print $1 }'; }
else
    die "Install shasum or sha256sum; unverified binaries are never installed."
fi

mkdir -p "$coder_bin_dir"
coder_work="$(mktemp -d "$coder_bin_dir/.coder-install.XXXXXX")"
coder_replacing=0
coder_installed=0
cleanup() {
    coder_exit=$?
    coder_keep_work=0
    if [ "$coder_replacing" = 1 ] && [ "$coder_installed" != 1 ]; then
        for coder_command in $coder_commands; do
            if [ -e "$coder_work/$coder_command.previous" ] || [ -L "$coder_work/$coder_command.previous" ]; then
                if ! mv -f "$coder_work/$coder_command.previous" "$coder_bin_dir/$coder_command"; then
                    coder_keep_work=1
                    say "Could not restore $coder_command; its previous copy is in $coder_work."
                fi
            elif [ -f "$coder_work/$coder_command.absent" ]; then
                rm -f "$coder_bin_dir/$coder_command" || coder_keep_work=1
            fi
        done
    fi
    if [ "$coder_keep_work" = 0 ]; then rm -rf "$coder_work"; fi
    trap - EXIT
    exit "$coder_exit"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

pointer() {
    download "$coder_base_url/coder.$1" "$coder_work/pointer" 2>/dev/null || return 1
    tr -d '\r\n' <"$coder_work/pointer"
}
if [ -n "$coder_target" ]; then
    coder_version="$coder_target"
elif [ -n "$coder_channel" ]; then
    coder_version="$(pointer "$coder_channel" || :)"
else
    # No channel named: stable, or the newest release candidate while no
    # stable release is published.
    coder_channel=stable
    coder_version="$(pointer stable || :)"
    if [ -z "$coder_version" ]; then
        coder_version="$(pointer rc || :)"
        if [ -n "$coder_version" ]; then
            coder_channel=rc
            say "No stable release is published yet; installing the release candidate."
        fi
    fi
fi
[ -n "$coder_version" ] || die "Could not read coder.$coder_channel from $coder_base_url. Check your connection or choose an exact version."
is_version "$coder_version" || die "The channel does not name a valid version: $coder_version."

say "Installing Coder $coder_version for $coder_platform..."
download "$coder_base_url/SHA256SUMS-coder-$coder_version" "$coder_work/SHA256SUMS" 2>/dev/null ||
    die "Version $coder_version is not published, or $coder_base_url is unreachable."
# The digest the sums file names for $1 once, or nothing.
sums_entry() {
    awk -v name="$1" '
        $2 == name || $2 == "*" name { count++; hash=$1 }
        END { if (count == 1) print hash }
    ' "$coder_work/SHA256SUMS"
}
# Downloads $1 to $2 and checks it against the sums file's digest $3.
fetch_verified() {
    printf '%s' "$3" | grep -Eq '^[A-Fa-f0-9]{64}$' ||
        die "Version $coder_version has no verified $coder_platform build."
    download "$coder_base_url/$1" "$2" 2>/dev/null ||
        die "Could not download $1. Rerun the installer to retry."
    [ "$(digest "$2")" = "$(printf '%s' "$3" | tr 'A-F' 'a-f')" ] ||
        die "Checksum mismatch for $1. The existing installation is unchanged."
    say "  Verified $1."
}
coder_archive="coder-$coder_version-$coder_platform.tar.gz"
if grep -Eq "[[:space:]]\*?coder-$coder_version-$coder_platform\.tar\.gz\$" "$coder_work/SHA256SUMS"; then
    command -v tar >/dev/null 2>&1 || die "Install tar before running this installer."
    fetch_verified "$coder_archive" "$coder_work/archive.tar.gz" "$(sums_entry "$coder_archive")"
    mkdir "$coder_work/unpacked"
    tar -xzf "$coder_work/archive.tar.gz" -C "$coder_work/unpacked" ||
        die "Could not unpack $coder_archive. The existing installation is unchanged."
    for coder_command in $coder_commands; do
        if [ -L "$coder_work/unpacked/$coder_command" ] || [ ! -f "$coder_work/unpacked/$coder_command" ]; then
            die "$coder_archive has no $coder_command. The existing installation is unchanged."
        fi
        mv "$coder_work/unpacked/$coder_command" "$coder_work/$coder_command"
        chmod 755 "$coder_work/$coder_command"
    done
else
    for coder_command in $coder_commands; do
        coder_artifact="$coder_command-$coder_version-$coder_platform"
        coder_expected="$(sums_entry "$coder_artifact")"
        printf '%s' "$coder_expected" | grep -Eq '^[A-Fa-f0-9]{64}$' ||
            die "Version $coder_version has no verified $coder_platform build."
        fetch_verified "$coder_artifact" "$coder_work/$coder_command" "$coder_expected"
        chmod 755 "$coder_work/$coder_command"
    done
fi
for coder_command in $coder_commands; do
    coder_version_output="$("$coder_work/$coder_command" --version </dev/null)" ||
        die "Downloaded $coder_command does not run on this system. The existing installation is unchanged."
    printf '%s\n' "$coder_version_output" | awk -v name="$coder_command" -v version="$coder_version" '
        NR == 1 && $1 == name && $2 == version { matches=1 }
        END { exit !matches }
    ' || die "Downloaded $coder_command does not report version $coder_version. The existing installation is unchanged."
    if [ "$coder_command" = coder ]; then say "  $coder_version_output"; fi
done

# Stage and replace on the same filesystem. Keep previous entries until all
# renames succeed, including symlinks from earlier source installations.
for coder_command in $coder_commands; do
    if [ -d "$coder_bin_dir/$coder_command" ]; then die "$coder_bin_dir/$coder_command is a directory."; fi
    if [ -e "$coder_bin_dir/$coder_command" ] || [ -L "$coder_bin_dir/$coder_command" ]; then
        cp -p -P "$coder_bin_dir/$coder_command" "$coder_work/$coder_command.previous"
    else
        : >"$coder_work/$coder_command.absent"
    fi
done
coder_replacing=1
for coder_command in $coder_commands; do
    mv -f "$coder_work/$coder_command" "$coder_bin_dir/$coder_command" || die "Could not install $coder_command; restoring the previous commands."
done
coder_installed=1
say "Installed Coder (coder and openagents) in $coder_bin_dir."

# Append an idempotent PATH entry. Appending preserves symlinked dotfiles,
# their permissions, and every setting outside this installer block.
if [ "${CODER_NO_PATH_UPDATE:-}" != 1 ]; then
    coder_shell="$(basename "${SHELL:-sh}")"
    case "$coder_shell" in
        zsh) coder_profile="${ZDOTDIR:-$HOME}/.zshrc" ;;
        bash)
            if [ "$coder_os" = macos ]; then coder_profile="$HOME/.bash_profile"; else coder_profile="$HOME/.bashrc"; fi
            ;;
        fish) coder_profile="${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish" ;;
        *) coder_profile="$HOME/.profile" ;;
    esac
    # Single quotes keep arbitrary install paths literal in shell syntax.
    coder_quoted="$(printf '%s' "$coder_bin_dir" | sed "s/'/'\\\\''/g")"
    if [ "$coder_shell" = fish ]; then
        coder_path_line="fish_add_path '$coder_quoted'"
    else
        coder_path_line="export PATH='$coder_quoted':\"\$PATH\""
    fi
    if ! grep -Fqx "$coder_path_line" "$coder_profile" 2>/dev/null; then
        if mkdir -p "$(dirname "$coder_profile")" &&
            printf '\n# Coder installer PATH\n%s\n' "$coder_path_line" >>"$coder_profile"; then
            say "Added $coder_bin_dir to PATH in $coder_profile. Open a new terminal to use it."
        else
            say "Could not update $coder_profile. Add $coder_bin_dir to PATH manually."
        fi
    fi
fi
case ":$PATH:" in
    *":$coder_bin_dir:"*) say "Run coder from your project directory." ;;
    *) say "Run $coder_bin_dir/coder now, or reopen your terminal and run coder." ;;
esac
say "Rerun this installer to update Coder."
