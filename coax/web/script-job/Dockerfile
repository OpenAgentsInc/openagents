# oa-script-job: the compile job for realm scripts other people wrote (GitHub #13467;
# docs/protocol/world-scripts-v1.md, "The compile job"), behind a socket (serve.c). deploy.sh --stage writes
# this build's context:
#   compiler/  the Coax compiler's sources, only to name and check its published Linux build
#   repo/      the files the job reads, at their paths in the repository, and nothing else
#   host/      serve.c and the job runner it shares with the API (script-job-run.c)
# The image holds no key, token or secret, and runs as nobody.

# The compiler: this tree's published static Linux build (coax/published.sh), its hashes checked.
FROM debian:bookworm-slim AS coax
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY compiler/ /src/
RUN /src/coax/published.sh fetch /src /coax

# The Wasm toolchain, linked by the repository's own coax/web/toolchain.sh (clang 23, wasm-ld, the WASI
# builtins), from nixpkgs at the revision the build hosts use; then everything it needs, copied out.
FROM nixos/nix:2.24.10 AS wasm
ARG NIXPKGS=f6bf1059f02576e0b913c7b49d5e4eb6dda9cc22
ENV NIX_CONFIG="experimental-features = nix-command flakes"
COPY repo/coax/web/toolchain.sh /toolchain.sh
RUN nix registry add nixpkgs "github:NixOS/nixpkgs/$NIXPKGS" \
 && bash -c 'export COAXIAL_TOOLCHAIN=/opt/wasm; source /toolchain.sh; "$COAXIAL_CLANG" --version | head -n 1' \
 && bash -c 'set -euo pipefail; mkdir -p /closure; roots=(); \
      for l in /opt/wasm/bin/* /opt/wasm/sysroot/include /opt/wasm/sysroot/lib/* /opt/wasm/resource/include /opt/wasm/resource/lib/*/*; do \
        t="$(readlink "$l")"; t="${t#/nix/store/}"; roots+=("/nix/store/${t%%/*}"); done; \
      for p in $(nix-store -qR "${roots[@]}"); do cp -a "$p" /closure/; done; ls /closure | wc -l' 

# The service's host: C, platform effects only.
FROM debian:bookworm-slim AS host
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
COPY host/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I/src -o /oa-script-job /src/serve.c /src/script-job-run.c

FROM debian:bookworm-slim
# bash, coreutils (timeout), util-linux (unshare) and findutils are the base image's; perl for shasum (tool.sh);
# gcc and libc6-dev build the job's two Coax tools once, below.
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev perl && rm -rf /var/lib/apt/lists/*
COPY --from=wasm /closure/ /nix/store/
COPY --from=wasm /opt/wasm/ /opt/wasm/
COPY repo/ /opt/openagents/
COPY --from=coax /coax/coax /opt/openagents/coax/bin/coax
COPY --from=host /oa-script-job /usr/local/bin/oa-script-job
ENV COAX=/opt/openagents/coax/bin/coax COAX_TOOL_CACHE=0 COAXIAL_TOOLCHAIN=/opt/wasm COAXIAL_CLANG=/opt/wasm/bin/clang \
    COAXIAL_SYSROOT=/opt/wasm/sysroot COAXIAL_RESOURCES=/opt/wasm/resource COAXIAL_WASM_OPT=/opt/wasm/bin/wasm-opt \
    PATH=/opt/wasm/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin TMPDIR=/tmp \
    OA_SCRIPT_JOB=/opt/openagents/coax/web/script-job.sh OA_SCRIPT_JOB_SANDBOX=closed
# The job's tools, built once here (root owns them; the service's user can't change them), then the sample
# script compiled through the whole job as a smoke. The build has a network, so this one run is unboxed.
WORKDIR /opt/openagents
RUN coax/web/script-job.sh --warm \
 && OA_SCRIPT_JOB_SANDBOX=off coax/web/script-job.sh metaverse-engine/script/examples/pacer.coax pacer /tmp/smoke \
 && test -s /tmp/smoke/pacer.wasm && test -s /tmp/smoke/pacer.json && rm -rf /tmp/smoke /root/.cache
USER nobody
CMD ["oa-script-job"]
