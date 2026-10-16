#!/usr/bin/env bash
# Install the pinned tools for the disposable environment qualification.
set -euo pipefail
[[ $(uname -sm) == 'Linux x86_64' ]]
[[ ${OA_ENVIRONMENT_BUILD_GUEST:-} == 1 ]]
if [[ -x /usr/local/bin/rustc && -x /usr/local/bin/cargo && -x /usr/local/bin/coder &&
      -x /usr/local/bin/openagents && -x /usr/local/bin/microcoder &&
      -s /etc/ssl/certs/ca-certificates.crt ]] &&
    [[ $(/usr/local/bin/rustc --version) == 'rustc 1.97.1 '* ]] &&
    [[ $(/usr/local/bin/cargo --version) == 'cargo 1.97.1 '* ]] &&
    [[ $(/usr/local/bin/coder --version) == 'coder 1.0.0 (3230ded51181aa438656825da8245cea6c8e717c clean)' ]] &&
    command -v git >/dev/null && command -v cc >/dev/null && command -v pkg-config >/dev/null &&
    printf '%s\n' \
        '6bedc8cc2ea7626838b51ec3c12979383237528bc7ba0862e184679c38f0861d  /usr/local/bin/coder' \
        '675fdfac5ef5f8b6d343576fbd967c559cc9ebd9b8ef46f50d3432f8c90b7c64  /usr/local/bin/openagents' \
        'b05ecb338c980295f74b7ac3cb793cdaab1d461b42bb15c39ca55022854a9e43  /usr/local/bin/microcoder' \
        | sha256sum --check --status; then
    echo 'Pinned environment tools are already installed.'
    exit 0
fi
tool_root=${OA_ENVIRONMENT_TOOLS_WORK:?Set a private build directory.}
[[ $tool_root = /* && ! -L $tool_root ]]
mkdir -p "$tool_root"
chmod 0700 "$tool_root"
as_root() {
    if [[ $EUID == 0 ]]; then "$@"; else sudo -n "$@"; fi
}
as_root apt-get update
as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential pkg-config git curl ca-certificates xz-utils

rust_archive=rust-1.97.1-x86_64-unknown-linux-gnu.tar.xz
rust_sha=88f28fa9af20594179f85d6df67078dfd6fa93e2f6da5e1e9b0ac4997988ca4f
curl --fail --silent --show-error --location --connect-timeout 15 --max-time 600 \
    "https://static.rust-lang.org/dist/$rust_archive" -o "$tool_root/$rust_archive"
printf '%s  %s\n' "$rust_sha" "$tool_root/$rust_archive" | sha256sum --check --status
tar -xJf "$tool_root/$rust_archive" -C "$tool_root"
as_root "$tool_root/${rust_archive%.tar.xz}/install.sh" --prefix=/usr/local \
    --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu --disable-ldconfig

coder_archive=coder-1.0.0-linux-x86_64-musl.tar.gz
coder_sha=07d30ec2a1b8fafa4a802e4de0615808bcfd428903473e916a9f8a227abc79f4
curl --fail --silent --show-error --location --connect-timeout 15 --max-time 600 \
    "https://storage.googleapis.com/openagentsgemini-cli-releases/coder/$coder_archive" \
    -o "$tool_root/$coder_archive"
printf '%s  %s\n' "$coder_sha" "$tool_root/$coder_archive" | sha256sum --check --status
mkdir -p "$tool_root/coder"
tar -xzf "$tool_root/$coder_archive" -C "$tool_root/coder"
for command in coder openagents microcoder; do
    as_root install -m0755 "$tool_root/coder/$command" "/usr/local/bin/$command"
done
export PATH=/usr/local/bin:$PATH
[[ $(rustc --version) == 'rustc 1.97.1 '* ]]
[[ $(coder --version) == 'coder 1.0.0 (3230ded51181aa438656825da8245cea6c8e717c clean)' ]]
printf '%s  %s\n' 6bedc8cc2ea7626838b51ec3c12979383237528bc7ba0862e184679c38f0861d \
    /usr/local/bin/coder | sha256sum --check --status
echo 'Pinned environment tools installed.'
