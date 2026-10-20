# Pylon network runbook: a Mac, CoderOS, and a cloud VM

This runbook installs Pylon 1.0 on three kinds of computer, connects them
through the OpenAgents relay, and sends jobs between them. It records the
run of 2026-10-19: the commands, the results, the bugs it found, and how to
stop each pylon.

Later the same night, a GCE VM with an NVIDIA L4 (`oa-pylon-gpu-1`)
replaced the CPU VM (`oa-pylon-cloud-1`) under the same pylon key, and the
CPU VM was deleted. The CPU sections stay as a record of that run.

For what each command does, see the operator guide,
[Run a Pylon, and use one](pylon.md). For what Pylon 1.0 is meant to be, see
the [Pylon 1.0 specification](pylon-1.0.md).

## Contents

- [The three pylons](#the-three-pylons)
- [Before you start](#before-you-start)
- [Install on macOS (Apple Silicon, Metal)](#install-on-macos-apple-silicon-metal)
- [Install on NixOS with an NVIDIA GPU (CUDA)](#install-on-nixos-with-an-nvidia-gpu-cuda)
- [Install on a Google Compute Engine VM with an NVIDIA L4 (CUDA)](#install-on-a-google-compute-engine-vm-with-an-nvidia-l4-cuda)
- [Install on a Google Compute Engine VM (CPU, retired)](#install-on-a-google-compute-engine-vm-cpu-retired)
- [How the pylons find each other](#how-the-pylons-find-each-other)
- [The cross-job test](#the-cross-job-test)
- [Results from 2026-10-19](#results-from-2026-10-19)
- [Results for the L4 pylon](#results-for-the-l4-pylon)
- [Community lane, MCP, and the desktop Earn switch](#community-lane-mcp-and-the-desktop-earn-switch)
- [Troubleshooting](#troubleshooting)
- [Stop each pylon](#stop-each-pylon)
- [What this run did not cover](#what-this-run-did-not-cover)

## The three pylons

The Mac and CoderOS run `openagents` built from `8d36ad76f9` (2026-10-19)
with a Psionic engine built from the same tree; `678a76aa46`, a checker fix,
came after and matters only to a computer that runs `check decide`. The L4
VM runs `2b279549b6`, which includes both, and `b281ee839c` (#12234), which
names the model by its tensor digest. The CPU VM row is the pylon the L4 VM
replaced.

| Pylon | Computer | Engine | Lanes | Pylon key | Buyer key |
| --- | --- | --- | --- | --- | --- |
| `christohersmbp2`, "Mac M5 Max (network lane)" | The owner's MacBook Pro, Apple M5 Max, 128 GB | Psionic on Metal | Decisions (port 18196), text (port 18197) | `npub1rqplkvjtja2g0w7fn36jsxq97dqhpy9wf76v7qspe9r9hkvvr95qm99mt5` | `npub1mkr8nqfqx4cvvhdjeyh82k4jpcerckf8j8ezj47s665wyv2esyvsct6u9n` |
| `coderos-4080-net`, "CoderOS RTX 4080 (network lane)" | CoderOS (NixOS), RTX 4080 16 GB, 126 GB | Psionic on CUDA 13 | Decisions (port 18196), text (port 18097) | `npub1yna8wf3t6rs0mstyey7a2azr84rld32kgqgj8f6582we6r6w78eqj858ux` | `npub1fmhlry9uxlry6z33urta6vlculvny3hmpn0u80hv3wyl4kxg749secela6` |
| `oa-pylon-gpu-1`, "GCE L4 (network lane)" | GCE `g2-standard-8` in `us-central1-a`, NVIDIA L4 24 GB, 32 GB, Ubuntu 22.04 | Psionic on CUDA 12.9 | Decisions (port 18096), text (port 18097) | `npub1g47xkl9yhhtj3tta4t6wrzut5f27k4mtaf3qw4v8gj2j09ejk09qfulsw6` | `npub1pr3yk84y7a76ptdedvfck60wqxnxywlcu8atsfyqhyk6je49avpsp29pyu` |
| `oa-pylon-cloud-1` (deleted) | GCE `n2-standard-16` in `us-central1-a`, Debian 12, 64 GB, no GPU | Psionic on the CPU | Decisions only (port 18096) | Same as the L4 VM | Same as the L4 VM |

Each pylon admits only the three buyer keys (`--allow`), so each machine can
buy from the other two. All three serve the pinned Clef-Flash Q4_K_M file
(`sha256:36261166…`) and publish to `wss://relay.openagents.com` in the
`everglade` pool. None is linked to an owner and none is paid.

Each pylon keeps its keys and state in its own home, set with
`OPENAGENTS_PYLON_HOME`, so it does not touch the computer's default pylon
in `~/.openagents/compute` (the one the desktop app's Earn switch uses).
Models stay in the shared `~/.openagents/models`.

## Before you start

Each computer needs:

- The `openagents` command line, built from `main`. The run used commit
  `8d36ad76f9` or later; earlier builds have the bugs listed under
  [Troubleshooting](#troubleshooting).
- A `psionic-openai-server` for its platform. No engine is published yet
  (`openagents pylon version` says "not published in this release" for every
  platform), so you build it from `crates/psionic`.
- About 7 GB of free disk for the decision model, plus about 1 GB for the
  text model.
- Outbound HTTPS and WebSocket access. A pylon opens no inbound port.

Builds follow the repository's rules: work in a worktree, use the shared
Cargo target for the workspace, and load the build settings first:

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1 CARGO_INCREMENTAL=0
. scripts/dev/cargo-build-env.sh
```

The Psionic workspace is separate (`crates/psionic/Cargo.toml`), so it gets
its own target folder, such as `~/work/openagents-target-psionic`.

## Install on macOS (Apple Silicon, Metal)

1. Build the command line and the engine from a worktree of `main`:

   ```sh
   cargo build --release -p openagents-cli --bin openagents
   CARGO_TARGET_DIR=~/work/openagents-target-psionic \
     cargo build --release --manifest-path crates/psionic/Cargo.toml \
     -p psionic-serve --bin psionic-openai-server
   ```

   On macOS the engine builds with Metal; the result is about 23 MB.

1. Put both where the pylon can find them, named by commit:

   ```sh
   H=~/.openagents/pylon-net
   mkdir -p $H/bin $H/engine $H/home
   C=$(git rev-parse --short=10 HEAD)
   cp ~/work/openagents-target-1/release/openagents $H/bin/openagents-$C
   cp ~/work/openagents-target-psionic/release/psionic-openai-server $H/bin/psionic-openai-server-$C
   ln -sfn $H/bin/psionic-openai-server-$C $H/engine/psionic-openai-server
   ln -sfn $H/bin/openagents-$C $H/bin/openagents
   ```

   `OPENAGENTS_PSIONIC_BIN` names the folder that holds
   `psionic-openai-server`; the pylon looks there before `~/.openagents/bin`
   and `PATH`.

1. Download the text model. `up` turns on the text lane only when the file
   is present:

   ```sh
   cd ~/.openagents/models
   curl -fL -o Qwen3.5-0.8B-Q8_0.gguf.part \
     https://registry.ollama.ai/v2/library/qwen3.5/blobs/sha256:afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5
   shasum -a 256 Qwen3.5-0.8B-Q8_0.gguf.part   # afb707b6…897db5
   mv Qwen3.5-0.8B-Q8_0.gguf.part Qwen3.5-0.8B-Q8_0.gguf
   ```

1. Read the computer and make the keys:

   ```sh
   export OPENAGENTS_PYLON_HOME=$H/home OPENAGENTS_PSIONIC_BIN=$H/engine
   $H/bin/openagents pylon doctor --decide http://127.0.0.1:18196
   $H/bin/openagents pylon id
   ```

   `doctor` reported "Apple Silicon with 128 GB: Metal, about 0.3 s a
   decision" and blocked only on the missing decision model, which `up`
   downloads.

1. Start it. The run used ports 18196 and 18197 so that the default ports
   (18096 and 18097) stay free for the Earn switch. `launchctl submit` runs
   it as a launchd job that lasts until you remove it or the Mac restarts,
   and it writes no property list:

   ```sh
   launchctl submit -l com.openagents.pylon-net -o $H/up.log -e $H/up.log -- \
     /usr/bin/env OPENAGENTS_PYLON_HOME=$H/home OPENAGENTS_PSIONIC_BIN=$H/engine \
     HOME=$HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin \
     $H/bin/openagents pylon up --foreground --yes --port 18196 --text-port 18197 \
     --label "Mac M5 Max (network lane)" --allow BUYER1,BUYER2,BUYER3
   ```

   The first start downloads Clef-Flash (6.5 GB, digest checked), starts
   both engines, runs the 64-item self-check, and then serves. Later starts
   keep the settings in `$H/home/share.json`, so `up --foreground --yes` alone
   brings it back with the same ports and allowlist.

1. Check it: `$H/bin/openagents pylon status` says `serving`, names
   `clef-flash@sha256:36261166… on metal`, and lists both engines.

## Install on NixOS with an NVIDIA GPU (CUDA)

CoderOS has no system CUDA toolkit. The CUDA 13 pieces come from nixpkgs,
as `scripts/pylon-psionic.sh setup` fetches them; CoderOS already has them
in `~/work/pylon-p1/cuda`. Plain `cargo` builds the root workspace on
CoderOS without a nix shell.

1. Build both from a worktree of `main` (here `~/work/oa-pylon-net`):

   ```sh
   cargo build --release -p openagents-cli --bin openagents
   R=~/work/pylon-p1/cuda
   export NVCC=$R/nvcc/bin/nvcc PSI_CUDA_ARCH=sm_89
   export NVCC_PREPEND_FLAGS="-I$R/crt/include -I$R/cudart/include -I$R/cccl/include -I$R/cublas/include"
   export LIBRARY_PATH=$R/cudart/lib:$R/cublaslib-lib/lib
   export LD_LIBRARY_PATH=$R/cudart/lib:$R/cublaslib-lib/lib:/run/opengl-driver/lib
   unset RUSTC CARGO_BUILD_BUILD_DIR
   CARGO_TARGET_DIR=~/work/openagents-target-psionic \
     cargo build --release --manifest-path crates/psionic/Cargo.toml \
     -p psionic-serve --bin psionic-openai-server
   ```

   On a machine with `nvcc`, skip the exports. The CUDA engine is about
   147 MB. CoderOS's disk was at 17 to 50 GB free during the run, so the
   Psionic target was removed after copying the binary.

1. Install the binaries as on the Mac, under `~/work/pylon-net/bin` and
   `~/.openagents/pylon-net/engine`.

1. Use the pinned decision model file. CoderOS had an older copy with the
   same tensors but different header bytes (`sha256:fd3e9060…`); it passes
   the self-check, but checkers and the community lane compare the file
   digest, so download the pinned file ([#12234](https://github.com/OpenAgentsInc/openagents/issues/12234)):

   ```sh
   rm ~/.openagents/models/Clef-Flash-Q4_K_M.gguf   # only if it is not 36261166…
   OPENAGENTS_PYLON_HOME=~/.openagents/pylon-net/home openagents pylon setup
   ```

   The text model can be a link to an existing copy, such as
   `ln -s ~/work/pylon-p1/models/qwen3.5-0.8b-q8_0.gguf ~/.openagents/models/Qwen3.5-0.8B-Q8_0.gguf`.

1. Start it as a transient user unit. It lasts until it is stopped or the
   computer restarts, and no unit file is written. `KillMode=mixed` sends
   SIGTERM to the pylon only, so it stops its engines and publishes an
   offline beacon itself:

   ```sh
   R=~/work/pylon-p1/cuda
   systemd-run --user --unit=oa-pylon-net --collect -p KillMode=mixed \
     --setenv=OPENAGENTS_PYLON_HOME=$HOME/.openagents/pylon-net/home \
     --setenv=OPENAGENTS_PSIONIC_BIN=$HOME/.openagents/pylon-net/engine \
     --setenv=LD_LIBRARY_PATH=$R/cudart/lib:$R/cublaslib-lib/lib:/run/opengl-driver/lib \
     -p StandardOutput=append:$HOME/.openagents/pylon-net/up.log \
     -p StandardError=append:$HOME/.openagents/pylon-net/up.log \
     $HOME/work/pylon-net/bin/openagents pylon up --foreground --yes --port 18196 \
     --pylon coderos-4080-net --label "CoderOS RTX 4080 (network lane)" \
     --allow BUYER1,BUYER2,BUYER3
   ```

   The engine needs `libcuda` from `/run/opengl-driver/lib`, which is why
   `LD_LIBRARY_PATH` is set; the pylon passes it to its engines.

1. Check it with `pylon status`: `on cuda`, decisions on 18196 and text on
   18097, self-check passed.

To survive a reboot, the Pylon 1.0 path is the host service: run `up`
without `--foreground` in the home the host watches. The run did not
install a unit on the owner's computer.

## Install on a Google Compute Engine VM with an NVIDIA L4 (CUDA)

`oa-pylon-gpu-1` serves both lanes on one L4: Clef-Flash and the text model
together use about 8.4 GB of its 24 GB.

1. Create the VM on demand (not spot, which Google can reclaim at any
   time). GPU VMs can't live-migrate, so they need
   `--maintenance-policy TERMINATE`; Google restarts the VM after a host
   maintenance event, and the unit below starts the pylon again. Google
   no longer publishes a Debian Deep Learning VM image, so this uses the
   Ubuntu 22.04 one, which comes with NVIDIA driver 580 and CUDA 12.9
   (`nvcc` in `/usr/local/cuda`):

   ```sh
   export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
   gcloud compute instances create oa-pylon-gpu-1 --project openagentsgemini \
     --zone us-central1-a --machine-type g2-standard-8 \
     --accelerator type=nvidia-l4,count=1 \
     --maintenance-policy TERMINATE --provisioning-model STANDARD \
     --image-family common-cu129-ubuntu-2204-nvidia-580 \
     --image-project deeplearning-platform-release \
     --boot-disk-size 100GB --boot-disk-type pd-balanced \
     --metadata install-nvidia-driver=True \
     --labels purpose=oa-pylon-network
   ```

   The VM carries no `run-id` label, so no session's reaper stops it. If
   `us-central1-a` has no L4 capacity, try `us-central1-b` or
   `us-central1-c`. Check the GPU with `nvidia-smi`: `NVIDIA L4, 580.x,
   23034 MiB, 8.9`.

1. Install the build tools, a current `protoc`, and Rust. Ubuntu 22.04's
   `protoc` (3.12) refuses `spark-primitives`' proto3 optional fields, so
   install an upstream release and name it with `PROTOC`:

   ```sh
   sudo apt-get install -y --no-install-recommends build-essential pkg-config \
     clang libclang-dev cmake protobuf-compiler libprotobuf-dev libssl-dev \
     ca-certificates curl git jq unzip libasound2-dev libudev-dev libsqlite3-dev
   curl -fLO https://github.com/protocolbuffers/protobuf/releases/download/v25.1/protoc-25.1-linux-x86_64.zip
   sudo unzip -o protoc-25.1-linux-x86_64.zip -d /usr/local bin/protoc 'include/*'
   curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.1
   . ~/.cargo/env
   ```

1. Build both programs. Psionic's CUDA kernels include CUB from
   `<cccl/cub/...>`, which is CUDA 13's layout; CUDA 12.9 keeps CUB in
   `include/cub`. A folder with one link (`cccl` pointing at
   `/usr/local/cuda/include`) on `nvcc`'s include path bridges the two. The
   L4 is `sm_89`, like the RTX 4080:

   ```sh
   git clone --filter=blob:none https://github.com/OpenAgentsInc/openagents.git ~/src/openagents
   cd ~/src/openagents
   export CARGO_INCREMENTAL=0 PROTOC=/usr/local/bin/protoc
   CARGO_TARGET_DIR=~/target cargo build --release -p openagents-cli --bin openagents
   mkdir -p ~/cuda-shim && ln -sfn /usr/local/cuda/include ~/cuda-shim/cccl
   NVCC_PREPEND_FLAGS=-I$HOME/cuda-shim NVCC=/usr/local/cuda/bin/nvcc PSI_CUDA_ARCH=sm_89 \
     cargo build --release --manifest-path crates/psionic/Cargo.toml \
     -p psionic-serve --bin psionic-openai-server --target-dir ~/target-psionic
   C=$(git rev-parse --short=10 HEAD)
   mkdir -p ~/.openagents/bin
   install -m 0755 ~/target/release/openagents ~/.openagents/bin/openagents-$C
   install -m 0755 ~/target-psionic/release/psionic-openai-server ~/.openagents/bin/psionic-openai-server-$C
   ln -sfn openagents-$C ~/.openagents/bin/openagents
   ln -sfn psionic-openai-server-$C ~/.openagents/bin/psionic-openai-server
   ```

   On 8 vCPUs the command line took about 16 minutes and the CUDA engine
   about 22. The engine links `libcudart.so.12` from
   `/usr/local/cuda-12.9`.

1. Move the CPU VM's keys over, so the pylon keeps its key and the other
   pylons' allowlists, which name its buyer key, still admit it. The pipe
   goes from VM to VM and writes nothing on the computer that runs it:

   ```sh
   gcloud compute ssh oa-pylon-cloud-1 --zone us-central1-a --project openagentsgemini \
     --command 'cd ~/.openagents/pylon-net/home && tar czf - provider.key buyer.key checker.key aggregator.key checker-seed checker-round' |
   gcloud compute ssh oa-pylon-gpu-1 --zone us-central1-a --project openagentsgemini \
     --command 'mkdir -p ~/.openagents/pylon-net/home && chmod 700 ~/.openagents/pylon-net/home && tar xzf - -C ~/.openagents/pylon-net/home'
   ```

   For a new pylon, skip this step; `pylon id` makes fresh keys.

1. Download the text model as on the Mac (into `~/.openagents/models`,
   digest `afb707b6…`), then the decision model, and read the computer:

   ```sh
   export OPENAGENTS_PYLON_HOME=~/.openagents/pylon-net/home
   openagents pylon setup     # 6.5 GB, under 3 minutes inside Google's network
   openagents pylon doctor    # device: NVIDIA L4 with 22 GB: CUDA
   ```

1. Stop the CPU pylon before you start this one. Both would read every job
   sent to the shared pylon key: `sudo systemctl disable --now oa-pylon` on
   the CPU VM publishes its offline beacon.

1. Install a system unit. The engines find `libcudart` through
   `LD_LIBRARY_PATH`:

   ```ini
   # /etc/systemd/system/oa-pylon.service
   [Unit]
   Description=OpenAgents Pylon (oa-pylon-gpu-1, NVIDIA L4, network lane)
   After=network-online.target
   Wants=network-online.target

   [Service]
   User=christopherdavid
   Environment=HOME=/home/christopherdavid
   Environment=OPENAGENTS_PYLON_HOME=/home/christopherdavid/.openagents/pylon-net/home
   Environment=LD_LIBRARY_PATH=/usr/local/cuda/lib64
   ExecStart=/home/christopherdavid/.openagents/bin/openagents pylon up --foreground --yes --dedicated --pylon oa-pylon-gpu-1 --label "GCE L4 (network lane)" --allow BUYER1,BUYER2,BUYER3
   Restart=on-failure
   RestartSec=10
   KillMode=mixed
   TimeoutStopSec=120

   [Install]
   WantedBy=multi-user.target
   ```

   ```sh
   sudo systemctl daemon-reload
   sudo systemctl enable --now oa-pylon.service
   ```

   Within about a minute, `pylon status` says `serving`, `on cuda`, with
   decisions on 18096 and text on 18097. After `sudo shutdown -r now` the
   pylon was serving again within a minute of boot.

1. Check it:

   ```sh
   openagents pylon doctor     # ready, and the decision server is answering (cuda)
   openagents pylon verify     # engine, beacon, clock, and loopback: ok
   ```

   `verify` lists the owner link and trusted checks as `-`: this pylon is
   not linked, and only checkers this computer trusts count.

### What the L4 VM costs

On-demand prices in `us-central1` on 2026-10-19:

| Item | Price |
| --- | --- |
| `g2-standard-8` (8 vCPUs, 32 GB, and one L4; the GPU is in the machine price) | $0.8536 an hour, about $623 a month |
| 100 GB `pd-balanced` boot disk | $0.10 per GB a month, about $10 a month |
| Total while running | about $0.87 an hour, about $633 a month |
| Stopped | the disk only, about $10 a month |

The CPU VM it replaced (`n2-standard-16`) cost about $0.78 an hour. Spot
pricing for `g2-standard-8` is about $0.36 to $0.51 an hour, but Google can
stop a spot VM at any time, so this pylon runs on demand. Check current
prices on the [Compute Engine GPU pricing
page](https://cloud.google.com/compute/gpus-pricing) before you rely on these.

## Install on a Google Compute Engine VM (CPU, retired)

The L4 VM replaced this VM on 2026-10-19, and `oa-pylon-cloud-1` was
deleted. These are the steps it was built with.

1. Create a dedicated VM (not a pool host, which deletes itself when idle).
   Label it with the session's run labels while you set it up:

   ```sh
   export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
   gcloud compute instances create oa-pylon-cloud-1 --project openagentsgemini \
     --zone us-central1-a --machine-type n2-standard-16 \
     --image-family debian-12 --image-project debian-cloud \
     --boot-disk-size 120GB --boot-disk-type pd-balanced \
     --labels "$(python3 scripts/dev/agent-run.py labels),purpose=oa-pylon-network"
   ```

   A pylon needs no firewall rule: it dials out.

1. Install the build tools and Rust, then build both programs. Psionic's
   `PSIONIC_CUDA=off` links the CUDA backend's stubs for a CPU build:

   ```sh
   sudo apt-get install -y --no-install-recommends build-essential pkg-config \
     clang libclang-dev cmake protobuf-compiler libprotobuf-dev libssl-dev \
     ca-certificates curl git jq libasound2-dev libudev-dev libsqlite3-dev
   curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.1
   . ~/.cargo/env
   git clone --filter=blob:none https://github.com/OpenAgentsInc/openagents.git ~/src/openagents
   cd ~/src/openagents
   CARGO_TARGET_DIR=~/target cargo build --release -p openagents-cli --bin openagents
   PSIONIC_CUDA=off cargo build --release --manifest-path crates/psionic/Cargo.toml \
     -p psionic-serve --bin psionic-openai-server --target-dir ~/target-psionic
   install -m 0755 ~/target/release/openagents ~/.openagents/bin/openagents
   install -m 0755 ~/target-psionic/release/psionic-openai-server ~/.openagents/bin/
   ```

   On 16 vCPUs the first build of both took about 25 minutes; a rebuild of
   the command line after a pylon change took about 6.

1. Download the model: `OPENAGENTS_PYLON_HOME=~/.openagents/pylon-net/home
   openagents pylon setup` (about one minute inside Google's network).

1. Install a system unit. This VM runs nothing else, so `--dedicated` skips
   the lease table. A CPU pylon is refused unless `--allow-slow` says to
   serve anyway:

   ```ini
   # /etc/systemd/system/oa-pylon.service
   [Unit]
   Description=OpenAgents Pylon (oa-pylon-cloud-1, CPU, network lane)
   After=network-online.target
   Wants=network-online.target

   [Service]
   User=christopherdavid
   Environment=HOME=/home/christopherdavid
   Environment=OPENAGENTS_PYLON_HOME=/home/christopherdavid/.openagents/pylon-net/home
   ExecStart=/home/christopherdavid/.openagents/bin/openagents pylon up --foreground --yes --allow-slow --dedicated --allow BUYER1,BUYER2,BUYER3
   Restart=on-failure
   RestartSec=10
   KillMode=mixed
   TimeoutStopSec=120

   [Install]
   WantedBy=multi-user.target
   ```

   ```sh
   sudo systemctl daemon-reload
   sudo systemctl enable --now oa-pylon.service
   ```

   `Restart=on-failure` keeps `pylon down` meaningful: `down` ends the
   foreground supervisor with exit code 0, and the unit then stays down.

1. `pylon status` says `serving ... on cpu` once the self-check passes. On
   the CPU it takes about six minutes (0.16 decisions a second). The text
   lane stays off on a CPU-only computer.

1. Before the session that created it ends, remove the `run-id` label so
   the session's reaper does not stop a pylon meant to keep running:

   ```sh
   gcloud compute instances remove-labels oa-pylon-cloud-1 --zone us-central1-a \
     --project openagentsgemini --labels run-id,run-host
   ```

## How the pylons find each other

Nothing is configured between the machines. Each pylon:

1. Publishes a NIP-PYLON beacon (kind `30200`, address
   `30200:<pylon key>:<slug>`) to the relay when it starts, when its free
   slots or status change, and every 60 seconds; a beacon is valid for 240
   seconds.
1. Publishes a NIP-65 relay list (kind `10002`) for its key, naming the
   relay it reads jobs from.
1. Reads jobs addressed to its key: text jobs (`25900`) and decision jobs
   (`25910`), each NIP-44 encrypted to it.

A buyer reads the beacons (`openagents pylon status --field`), picks a
fresh, online pylon with a free slot (or the one `--pylon NPUB` names),
encrypts the job to it, and waits for the encrypted result. After the
restart at about 19:57 local time, every machine saw all three:

```text
$ openagents pylon status --field
online   oa-pylon-cloud-1     Gpu/Medium busy 0/2 jobs 1 up 0s  30200:457c6b7c…:oa-pylon-cloud-1
online   Mac M5 Max (network lane) Gpu/Medium busy 0/2 jobs 2 up 0s  30200:1803fb32…:christohersmbp2
online   CoderOS RTX 4080 (network lane) Gpu/Medium busy 0/2 jobs 3 up 0s  30200:24fa7726…:coderos-4080-net
```

Builds before `6677c531c1` named every pylon "Gpu/Medium". On the final
build (`8d36ad76f9`), each names its own class:

```text
online   oa-pylon-cloud-1     Cpu/Xl busy 0/2 jobs 4 up 0s  30200:457c6b7c…:oa-pylon-cloud-1
online   Mac M5 Max (network lane) UnifiedMemory/Xl busy 0/2 jobs 10 up 60s  30200:1803fb32…:christohersmbp2
online   CoderOS RTX 4080 (network lane) Gpu/Medium busy 0/2 jobs 14 up 60s  30200:24fa7726…:coderos-4080-net
```

The tier comes from the memory a pylon names (64 GB and 128 GB both read as
`Xl`), not from how fast it decides.

## The cross-job test

Each machine ran
[`matrix.sh`](#the-matrix-script) against the other two, as its own buyer
key, with its own decision engine as the checker's reference:

| Check | Command |
| --- | --- |
| Text job, three times (Mac and CoderOS only) | `openagents --json pylon ask "Name three planets in one line." --pylon NPUB` |
| Decision job, three times | `openagents --json pylon decide --state '{"message": "I was charged twice for my subscription this month."}' --questions '{"team": {"type": "choice", ...}, "urgent": {"type": "noul", ...}}' --pylon NPUB` |
| A checker round: eight fresh private decision items, recomputed on this machine's engine, and a signed NIP-32 verdict | `openagents pylon check decide --pylon NPUB --reference http://127.0.0.1:PORT --own-buyer` |

Run the checker round at least a minute after the other jobs, or from a
separate buyer: a pylon admits 10 jobs a minute per buyer by default
(`--rate`), and the six jobs plus eight check items exceed that.

Then, from the Mac, a pool aggregate over the hour and its recomputation:

```sh
openagents pylon pool --minutes 60 --publish
openagents pylon pool verify --aggregator npub1wsmd70wtn8qfj0kgw2z5mmrahmsksvvfe62n5u4dmhyr3j90vrss7xxc65
```

## Results from 2026-10-19

### Self-checks

Each pylon checked its own answers against the shipped Metal reference (64
items) before it served:

| Pylon | Device | Median distance | Worst item | Rounds that pass | Decisions a second |
| --- | --- | --- | --- | --- | --- |
| Mac | Metal | 0 | 4.4e-16 | 100 % | 3.2 |
| CoderOS | CUDA | 0.0011 | 0.0052 | 100 % | 20.7 |
| GCE VM | CPU | 0.00032 | 0.0020 | 100 % | 0.16 |

The limits are 0.05 for a round's mean and 0.5 for any item.

### Jobs between machines

Latencies are from the buyer's side: discovery is connecting and reading
beacons; answer is from sending the job to its result. Ranges cover three
jobs; every text job's receipt (`3201`) was published and accepted.

| Buyer → pylon | Text answer | Decision answer | Checker round (8 items) |
| --- | --- | --- | --- |
| Mac → CoderOS | 301–350 ms (discovery 422–453 ms) | 474–544 ms | pass, mean 0.0018, worst 0.0045, 465–558 ms an item |
| Mac → cloud | none (no text lane) | 8.0 s first, then 325–368 ms | pass, mean 0.0007, worst 0.0014, 5.9–7.8 s an item |
| CoderOS → Mac | 1,114–1,313 ms | 461–792 ms | pass, mean 0.0031, worst 0.0079, 490–741 ms an item |
| CoderOS → cloud | none | 357–385 ms | pass, mean 0.0020, worst 0.0032, 6.1–7.8 s an item |
| Cloud → Mac | 964–1,005 ms (discovery 107–136 ms) | 377–405 ms | pass, mean 0.0005, worst 0.0012, 354–586 ms an item |
| Cloud → CoderOS | 274–304 ms | 180–303 ms | pass, mean 0.0012, worst 0.0039, 211–309 ms an item |

All three pylons gave the same decision: `team` = `billing`, `urgent`
p = 0.806. Repeats of an identical decision on the CPU pylon took about
350 ms after the first (8 s); the checker's fresh items, which never repeat,
show the CPU's real speed of 6 to 8 seconds a decision. The Mac's text
answers were slower than CoderOS's because the Mac was running heavy builds
(load average 30 to 45) during the run.

A second run of the script from the Mac sent its checker rounds right after
the other six jobs, ran into the pylon's rate limit, and published a
`check-fail` against CoderOS that was not CoderOS's fault. That bug is fixed,
but the label stands, so the Mac's own `ask` skips CoderOS until 24 hours
after it (about 20:00 on 2026-10-20); CoderOS's other buyers are not
affected.

Earlier in the run, before the decision command existed, the Mac checked
the cloud pylon and the cloud pylon checked the Mac with `check decide`;
both passed (labels `4fdfa34b…` and `0d787647…`).

### The pool

```text
pool everglade: 3 online, 6 slots (6 free), 25 accepted jobs, 449 tokens
published a33937487053… by npub1wsmd70wtn8qfj0kgw2z5mmrahmsksvvfe62n5u4dmhyr3j90vrss7xxc65
verified: 3 online, 25 accepted jobs, 449 tokens in the window
```

`openagents pylon league` grouped the three under their class with their
job counts; a pylon shows a `*` for passing checks only from checkers the
reader trusts (`--checker NPUB`, `OPENAGENTS_PYLON_CHECKERS`, or this
computer's own checker key).

## Results for the L4 pylon

After the swap, the field showed the CPU pylon's last beacon offline and the
L4 pylon online under the same key:

```text
online   GCE L4 (network lane) Gpu/Medium busy 0/2 jobs 0 up 0s  30200:457c6b7c…:oa-pylon-gpu-1
offline  oa-pylon-cloud-1     Cpu/Xl busy 2/2 jobs 4 up 6046s  30200:457c6b7c…:oa-pylon-cloud-1
```

### Self-check

| Pylon | Device | Median distance | Worst item | Rounds that pass | Decisions a second |
| --- | --- | --- | --- | --- | --- |
| L4 VM | CUDA | 0.0011 | 0.0052 | 100 % | 7.95 |
| CPU VM (before) | CPU | 0.00032 | 0.0020 | 100 % | 0.16 |

The figure is from the self-check `up` runs at start (64 fresh items), so
the L4 is about 50 times as fast as the 16-vCPU CPU VM. A second
`openagents pylon selfcheck` on a running pylon printed 1,728 decisions a
second: it repeats the same 64 items, which the engine answers from cache,
so don't read it as a speed.

### Jobs with the L4 pylon

Latencies are from the buyer's side, as above.

| Buyer → pylon | Text answer | Decision answer | Checker round (8 items) |
| --- | --- | --- | --- |
| Mac → L4 | 3,575–3,593 ms (discovery 494–583 ms) | 386–431 ms (discovery 291–371 ms) | refused: the Mac's `8d36ad76f9` build names the model by file digest, the L4 pylon by tensor digest |
| CoderOS → L4 | 3,550–3,754 ms (discovery 454–1,023 ms) | 387–427 ms (discovery 272–299 ms) | pass, mean 1.6e-7, worst 4.2e-7, 453–547 ms an item (label `47c55b54…`) |
| L4 → Mac | skipped (see below) | 157–418 ms (discovery 51–54 ms) | not run |
| L4 → CoderOS | skipped (see below) | 188–214 ms (discovery 57–67 ms) | not run |

- Every decision gave `team` = `billing`, `urgent` p = 0.806, as on the
  other pylons.
- Each text answer ran to the 512-token limit, because the 0.8B model
  keeps going on this prompt: about 145 tokens a second on the L4. The
  first run didn't record its answers' token counts, so its text times
  don't compare directly with these.
- Checker items are fresh decisions, so they show the real speed: 0.45 to
  0.55 s an item on the L4, against 5.9 to 7.8 s on the CPU VM. The
  CoderOS round used a checker built from `cff0a8da08`, because a checker
  must name the model the same way the pylon does.
- `ask` from the L4 VM skipped the Mac and CoderOS: the checker key that
  moved over from the CPU VM had published `check-fail` verdicts against
  both on a build before `678a76aa46`, and `ask` skips a pylon with a
  trusted `check-fail` for 24 hours. Decisions don't use that filter.
  The skip ends on its own by 2026-10-20 evening.

## Community lane, MCP, and the desktop Earn switch

### Community lane

A gateway sends a contributor pylon community-lane decisions only when the
pylon serves the pinned Clef-Flash file, admits the gateway's dispatch key,
and holds a recent `check-pass` from a checker the gateway trusts
(`decisions.checkers`); see [Community pylons](pylon.md#answer-decisions-clef).
These three pylons admit only each other's buyer keys, and their checks come
from their own checker keys, so the gateway would not use them. To offer
one to the community lane:

1. Serve the pinned file (`sha256:36261166…`).
1. Admit the gateway's dispatch key, or run with `--allow-any`, because a
   checker sends each round from a new buyer key.
1. Have a trusted checker run `openagents pylon check decide --watch` near a
   reference Clef ([`deploy/pylon-checker/`](../../deploy/pylon-checker/README.md)).

The checker rounds above are the same rounds such a checker runs; with
`--own-buyer` they use the stored buyer key so allowlisted pylons admit
them.

### Drive a pylon from an agent (MCP)

On the Mac, `openagents pylon mcp` answered `initialize` and listed its ten
read-only tools. `pylon_status` returned this pylon's state, `pylon_decide`
sent a decision to CoderOS and returned the answer marked untrusted, and
`pylon_down` refused because the server was not started with `--control`.
To add it to Claude Code:

```sh
claude mcp add pylon -- env OPENAGENTS_PYLON_HOME=$HOME/.openagents/pylon-net/home \
  $HOME/.openagents/pylon-net/bin/openagents pylon mcp
```

### The desktop Earn switch

The run did not use the switch: it would run `openagents pylon up --yes` in
the default home (`~/.openagents/compute`), which is the owner's own pylon,
not one of these three. The page is described in
[Earn from the desktop app](pylon.md#earn-from-the-desktop-app). To try it
next to these pylons:

- Run a separate development build of the app with a temporary data
  directory; don't restart the owner's app.
- The Earn pylon uses the default ports 18096 and 18097. The Mac pylon here
  uses 18196 and 18197, so both fit. Before `--text-port` existed, two
  pylons on one computer collided on 18097.

## Troubleshooting

Each of these happened during the run. The fixed ones are on `main`.

| What you see | Cause | Fix |
| --- | --- | --- |
| `self-check failed: 30.4% of rounds pass, worst item 5.499` on an honest pylon | Choice answers were compared in the JSON map's key order, which `serde_json`'s `preserve_order` makes insertion order, while the shipped reference is in sorted order | Fixed in `9cea89e5de`. Rebuild. |
| The text engine logs `backend=cpu` on a Mac or an NVIDIA box | `up` passed no `--backend` to the text engine | Fixed in `cb4f0d2f57` |
| Every text job fails: `model server refused with 400 Bad Request` | The pylon asked for `qwen3.5-0.8b-q8_0`, but Psionic lists `Qwen3.5-0.8B-Q8_0.gguf` and matches case-sensitively | Fixed in `47678ba9ed` |
| `up` on a CPU-only computer: `this computer cannot serve yet: ... serve anyway with --allow-slow` | `up` had no `--allow-slow` | Fixed in `007a9e4ab9`: `up --allow-slow` |
| Every Mac pylon is named `pylon` | macOS has no `HOSTNAME` for child processes and no `/etc/hostname` | Fixed in `8b02651a10` (`hostname -s`) |
| `up --foreground` right after a stop prints "This pylon already runs in another terminal" and exits | `up` trusted any `live.json` newer than 20 seconds | Fixed in `d216364c5d`; with an older build, wait 20 seconds |
| A killed `up --foreground` leaves Psionic running and the beacon online | SIGTERM was not handled | Fixed in `fa866edc49` |
| A second pylon on one computer: the text engine never starts | The text port was fixed at 18097 | `up --text-port N` (`f150a21a1b`) |
| MCP `pylon_decide`: `unknown command decide` | The command did not exist | Fixed in `0f74d199ba`: `openagents pylon decide` |
| `decide --wait 120`: `relay connection lifetime must be within 120 seconds` | The decision client asked the transport for up to 300 s | Fixed in `ec2ab7e47c`; `--wait` is at most 118 |
| `verify`: `the engine does not answer at http://127.0.0.1:18096/health` while serving | `verify` and `doctor` ignored `up --port` | Fixed in `8d36ad76f9` |
| Every beacon says `Gpu/Medium` | The class came from a 16 GB default | Fixed in `6677c531c1` |
| A check from a machine with the pinned file refuses: `the reference serves clef-flash@sha256:36261166…, but the pylon serves clef-flash@sha256:fd3e9060…` | Same tensors, different file bytes | Download the pinned file ([#12234](https://github.com/OpenAgentsInc/openagents/issues/12234)) |
| `ask` returns `not_admitted` right after you change the allowlist | The old provider is still draining with the old list (up to 90 s), and a settings change restarts the engines | Wait for `status` to say `serving` again |
| A checker round prints no verdict, or (before the fix) `ask` later says `no fresh online pylon with a free slot that passes its checks` for an honest pylon | The buyer hit the pylon's rate limit (10 jobs a minute). Builds before `678a76aa46` counted the refused items as misses and published a `check-fail`, and `ask` skips a pylon with a trusted `check-fail` from the last 24 hours | Fixed: a rate-limited round now publishes nothing. Wait a minute between other jobs and a round, or raise `--rate` on the pylon. A wrong `check-fail` already published keeps that buyer's `ask` away for 24 hours; use another computer's buyer meanwhile |
| `pylon earnings`: `the OpenAgents API refused the request (HTTP 404)` | The earnings route was not answering for these parties | Not investigated in this run |
| `systemctl --user stop` logs `engine exited (signal: 15); restarting` | systemd's default `KillMode` signals the engines too | Start the unit with `-p KillMode=mixed` |
| `doctor` and `setup` print `next: openagents pylon start --allow-any` | The next-step text predates `up` | Use `up` |
| On Ubuntu 22.04, the `openagents` build fails in `spark-primitives`: `This file contains proto3 optional fields, but --experimental_allow_proto3_optional was not set` | The distribution's `protoc` is 3.12 | Install an upstream `protoc` (25.1) and set `PROTOC` |
| With CUDA 12, the Psionic build fails: `cccl/cub/block/block_radix_sort.cuh: No such file or directory` | The kernels use CUDA 13's `include/cccl` layout | Put a folder with a `cccl` link to `/usr/local/cuda/include` on `NVCC_PREPEND_FLAGS`, or use CUDA 13 |
| A check refuses: `the reference serves clef-flash@sha256:36261166…, but the pylon serves clef-flash@gguf-tensors:sha256:2e417b22…` | Builds since `b281ee839c` (#12234) name the model by its tensor digest; older checkers name it by file digest | Run the checker from a build at or after `b281ee839c`; the pylons on older builds need the same update before newer checkers can check them |
| After moving a pylon's keys to a new computer, `ask` from it skips pylons that serve fine | The moved checker key brought its earlier `check-fail` verdicts with it, and the buyer trusts its own checker key | Wait out the 24 hours, or start the new home without `checker.key` |
| `pylon selfcheck` on a running pylon reports over 1,000 decisions a second | It repeats the start-up self-check's items, which the engine answers from cache | Read the speed from the self-check `up` runs at start (`selfcheck.json` in the pylon's home) |

## Stop each pylon

Each stop publishes an offline beacon and stops the engines.

| Pylon | Stop | Start again |
| --- | --- | --- |
| Mac | `launchctl remove com.openagents.pylon-net` | The `launchctl submit` command in [Install on macOS](#install-on-macos-apple-silicon-metal) |
| CoderOS | `ssh christopherdavid@100.74.238.61 systemctl --user stop oa-pylon-net` | The `systemd-run` command in [Install on NixOS](#install-on-nixos-with-an-nvidia-gpu-cuda) |
| L4 VM, the pylon only | `sudo systemctl disable --now oa-pylon` on the VM | `sudo systemctl enable --now oa-pylon` |
| L4 VM, and stop paying for it (the disk stays, about $10 a month) | `gcloud compute instances stop oa-pylon-gpu-1 --zone us-central1-a --project openagentsgemini` | `gcloud compute instances start oa-pylon-gpu-1 --zone us-central1-a --project openagentsgemini`; the unit starts the pylon at boot |
| L4 VM, for good | `sudo systemctl disable --now oa-pylon` on the VM first, so it publishes an offline beacon, then `gcloud compute instances delete oa-pylon-gpu-1 --zone us-central1-a --project openagentsgemini` | Recreate it from this runbook; keep a copy of the keys first if the pylon should keep its key |

Use `CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config` for the `gcloud`
commands. A stopped on-demand GPU VM starts again only if the zone has an
L4 free at that moment; if it doesn't, wait or recreate the VM in another
`us-central1` zone.

Don't use `openagents pylon down` alone on the Mac: `launchctl submit` keeps
the job alive, so launchd starts `up` again and the pylon turns back on. On
the VM and CoderOS, `down` works and the pylon stays off.

The L4 VM costs about $0.87 an hour; see [What the L4 VM
costs](#what-the-l4-vm-costs).

## What this run did not cover

- **Paid jobs.** The Spark wallet takes only mainnet payments, and test
  sales need a funded testnet Lightning node, so no paid job ran. Rehearse
  the paid path with `cargo run -p pylon --features fixture --example
  paid_field`, which runs on an in-process relay and test Lightning.
- **Owner links and payouts.** None of the pylons is linked, so earnings
  would go to each pylon key's party.
- **Surviving a reboot.** The Mac job and the CoderOS unit last until a
  restart; only the VM's system unit comes back on its own (tested on the
  L4 VM with a reboot).
- **One build across the network.** The Mac and CoderOS still run
  `8d36ad76f9`, so checkers on newer builds refuse them and the Mac's
  checker refuses the L4 pylon until all three run a build at or after
  `b281ee839c`.
- **A published engine.** Every platform still needs a self-built
  `psionic-openai-server`.

### The matrix script

```sh
#!/usr/bin/env bash
# matrix.sh OPENAGENTS REF_URL NAME=NPUB:text|notext ...
set -u
O=$1; REF=$2; shift 2
Q='{"team":{"type":"choice","instructions":"Which team should take this message?","criteria":{"billing":"payments and refunds","bugs":"something is broken","sales":"buying more"}},"urgent":{"type":"noul","instructions":"Is this urgent?"}}'
S='{"message":"I was charged twice for my subscription this month."}'
for spec in "$@"; do
  name=${spec%%=*}; rest=${spec#*=}; npub=${rest%%:*}; lanes=${rest#*:}
  if [ "$lanes" = text ]; then
    for i in 1 2 3; do $O --json pylon ask "Name three planets in one line." --pylon "$npub"; done
  fi
  for i in 1 2 3; do $O --json pylon decide --state "$S" --questions "$Q" --pylon "$npub" --wait 110; done
  sleep 61   # stay under the pylon's 10 jobs a minute per buyer
  $O --json pylon check decide --pylon "$npub" --reference "$REF" --own-buyer
done
```
