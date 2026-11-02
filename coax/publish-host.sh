#!/usr/bin/env bash
# Firm reason: 4. A release host's loop: keep this platform's published
# compiler build current with origin/main (coax/RELEASE.md, Published builds).
#
#   coax/publish-host.sh            # once: build and publish main's compiler if it isn't yet
#   coax/publish-host.sh --every 600   # forever, every 10 minutes (a systemd or launchd service)
#
# It works in its own clone (COAX_PUBLISH_CLONE, default ~/.cache/coax-publish/openagents),
# never in a checkout people work in. When main's source fingerprint already
# has a build for this platform it does nothing. Otherwise it bootstraps with
# this platform's release toolchain and uploads the result:
#   Linux: GCC for musl from nixpkgs (pkgsStatic), COAX_LDFLAGS=-static, so the
#          build runs on any Linux of its architecture.
#   macOS: the system's clang with MACOSX_DEPLOYMENT_TARGET=13.0.
# Uploads need write access to the bucket (CLOUDSDK_CONFIG on the release hosts).
# No GitHub Actions: release hosts are our own machines.
set -euo pipefail
every=0
[[ "${1:-}" == --every ]] && { every="${2:?--every SECONDS}"; }
clone="${COAX_PUBLISH_CLONE:-$HOME/.cache/coax-publish/openagents}"
remote="${COAX_PUBLISH_REMOTE:-https://github.com/OpenAgentsInc/openagents}"
log() { printf '%s coax publish-host: %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
once() {
  if [[ ! -d "$clone/.git" ]]; then mkdir -p "$(dirname "$clone")"; git clone -q "$remote" "$clone"; fi
  git -C "$clone" fetch -q origin main
  git -C "$clone" checkout -q --detach origin/main
  git -C "$clone" clean -qfdx -e coax/bin
  local name url
  name="$("$clone/coax/published.sh" name "$clone")"
  url="${COAX_PUBLISHED_URL:-https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds}/$name.tar.gz.sha256"
  if curl -fsS -o /dev/null "$url" 2> /dev/null; then log "${name:0:12} $(basename "$name") is published"; return 0; fi
  log "building ${name:0:12} for $(basename "$name") from $(git -C "$clone" rev-parse --short HEAD)"
  case "$(uname -s)" in
    Linux)
      # The musl toolchain's compilers by name, inside a nix shell that provides them.
      nix shell nixpkgs#pkgsStatic.stdenv.cc nixpkgs#python3 nixpkgs#gnumake --command env \
        CC="$(uname -m)-unknown-linux-musl-gcc" CXX="$(uname -m)-unknown-linux-musl-g++" COAX_LDFLAGS=-static \
        COAX_NO_PUBLISHED=1 COAX_MUTATION_FULL=1 "$clone/coax/build.sh"
      nix shell nixpkgs#pkgsStatic.stdenv.cc --command env \
        CC="$(uname -m)-unknown-linux-musl-gcc" CXX="$(uname -m)-unknown-linux-musl-g++" COAX_LDFLAGS=-static \
        "$clone/coax/published.sh" publish "$clone" ;;
    Darwin)
      env MACOSX_DEPLOYMENT_TARGET=13.0 COAX_NO_PUBLISHED=1 COAX_MUTATION_FULL=1 "$clone/coax/build.sh"
      env MACOSX_DEPLOYMENT_TARGET=13.0 "$clone/coax/published.sh" publish "$clone" ;;
    *) log "no release toolchain for $(uname -s)"; return 1 ;;
  esac
}
if [[ "$every" == 0 ]]; then once; exit; fi
while true; do once || log 'failed; trying again next round'; sleep "$every"; done
