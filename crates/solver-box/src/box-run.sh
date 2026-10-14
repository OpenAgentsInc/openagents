#!/usr/bin/env bash
# Runs on the box (an oa-boat sandbox) as its `user`, started detached by
# oa-solver-box (crates/solver-box). Puts Docker on the box when the image
# lacks it, signs Docker in to Artifact Registry with the token the runner
# wrote to /run/oa-solver-host/registry-token (and deletes it), pulls the
# solver container, and runs it with /run/oa-solver mounted (the run's env
# file and the Vertex AI token, written by the runner). The container's
# lines go to stdout as they come; this script's own are
# {"type":"solver_box_step",...}. When the run never reaches the container,
# the last line is {"type":"solver_box_summary","ok":false,"started":false}
# and the exit is 2. The image is the first argument.
set -uo pipefail

IMAGE="$1"
REGISTRY="${IMAGE%%/*}"
HOST_DIR=/run/oa-solver-host

step() {
  printf '{"type":"solver_box_step","step":"%s","state":"%s","text":"%s"}\n' "$1" "$2" "$3"
}

give_up() {
  rm -f "$HOST_DIR/registry-token"
  printf '{"type":"solver_box_summary","ok":false,"started":false,"error":"%s"}\n' "$1"
  exit 2
}

if ! command -v docker >/dev/null 2>&1; then
  step docker start "Installing Docker on the computer"
  sudo -n env DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1
  sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq docker.io >/dev/null 2>&1 \
    || give_up "The solver could not start: Docker did not install."
  step docker done "Docker is ready"
fi

sudo -n docker login -u oauth2accesstoken --password-stdin "https://$REGISTRY" \
  <"$HOST_DIR/registry-token" >/dev/null 2>&1 \
  || give_up "The solver could not start: the image store refused the sign-in."
rm -f "$HOST_DIR/registry-token"

step pull start "Getting the solver"
sudo -n docker pull -q "$IMAGE" >/dev/null 2>&1 \
  || give_up "The solver could not start: the image did not download."
step pull done "Got the solver"

# --privileged: the hands' run boundary (bubblewrap) needs namespaces.
sudo -n docker run --rm --privileged --network host \
  -v /run/oa-solver:/run/oa-solver "$IMAGE"
