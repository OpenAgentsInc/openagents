# Pylon network runbook: a Mac, CoderOS, and a cloud VM

This runbook installs Pylon 1.0 on three kinds of computer, connects them
through the OpenAgents relay, and sends jobs between them. It records the
run of 2026-10-19: the commands, the results, the bugs it found, and how to
stop each pylon.

For what each command does, see the operator guide,
[Run a Pylon, and use one](pylon.md). For what Pylon 1.0 is meant to be, see
the [Pylon 1.0 specification](pylon-1.0.md).

## Contents

- [The three pylons](#the-three-pylons)
- [Before you start](#before-you-start)
- [Install on macOS (Apple Silicon, Metal)](#install-on-macos-apple-silicon-metal)
- [Install on NixOS with an NVIDIA GPU (CUDA)](#install-on-nixos-with-an-nvidia-gpu-cuda)
- [Install on a Google Compute Engine VM (CPU)](#install-on-a-google-compute-engine-vm-cpu)
- [How the pylons find each other](#how-the-pylons-find-each-other)
- [The cross-job test](#the-cross-job-test)
- [Results from 2026-10-19](#results-from-2026-10-19)
- [Community lane, MCP, and the desktop Earn switch](#community-lane-mcp-and-the-desktop-earn-switch)
- [Troubleshooting](#troubleshooting)
- [Stop each pylon](#stop-each-pylon)
- [What this run did not cover](#what-this-run-did-not-cover)

## The three pylons

All three run `openagents` built from `8d36ad76f9` (2026-10-19) with a
Psionic engine built from the same tree; `678a76aa46`, a checker fix, came
after and matters only to a computer that runs `check decide`.

| Pylon | Computer | Engine | Lanes | Pylon key | Buyer key |
| --- | --- | --- | --- | --- | --- |
| `christohersmbp2`, "Mac M5 Max (network lane)" | The owner's MacBook Pro, Apple M5 Max, 128 GB | Psionic on Metal | Decisions (port 18196), text (port 18197) | `npub1rqplkvjtja2g0w7fn36jsxq97dqhpy9wf76v7qspe9r9hkvvr95qm99mt5` | `npub1mkr8nqfqx4cvvhdjeyh82k4jpcerckf8j8ezj47s665wyv2esyvsct6u9n` |
| `coderos-4080-net`, "CoderOS RTX 4080 (network lane)" | CoderOS (NixOS), RTX 4080 16 GB, 126 GB | Psionic on CUDA 13 | Decisions (port 18196), text (port 18097) | `npub1yna8wf3t6rs0mstyey7a2azr84rld32kgqgj8f6582we6r6w78eqj858ux` | `npub1fmhlry9uxlry6z33urta6vlculvny3hmpn0u80hv3wyl4kxg749secela6` |
| `oa-pylon-cloud-1` | GCE `n2-standard-16` in `us-central1-a`, Debian 12, 64 GB, no GPU | Psionic on the CPU | Decisions only (port 18096) | `npub1g47xkl9yhhtj3tta4t6wrzut5f27k4mtaf3qw4v8gj2j09ejk09qfulsw6` | `npub1pr3yk84y7a76ptdedvfck60wqxnxywlcu8atsfyqhyk6je49avpsp29pyu` |

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

## Install on a Google Compute Engine VM (CPU)

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

## Stop each pylon

Each stop publishes an offline beacon and stops the engines.

| Pylon | Stop | Start again |
| --- | --- | --- |
| Mac | `launchctl remove com.openagents.pylon-net` | The `launchctl submit` command in [Install on macOS](#install-on-macos-apple-silicon-metal) |
| CoderOS | `ssh christopherdavid@100.74.238.61 systemctl --user stop oa-pylon-net` | The `systemd-run` command in [Install on NixOS](#install-on-nixos-with-an-nvidia-gpu-cuda) |
| GCE VM | `sudo systemctl disable --now oa-pylon` on the VM, or stop the VM: `gcloud compute instances stop oa-pylon-cloud-1 --zone us-central1-a --project openagentsgemini` | `sudo systemctl enable --now oa-pylon` |
| GCE VM, for good | `gcloud compute instances delete oa-pylon-cloud-1 --zone us-central1-a --project openagentsgemini` | Recreate it from this runbook |

Don't use `openagents pylon down` alone on the Mac: `launchctl submit` keeps
the job alive, so launchd starts `up` again and the pylon turns back on. On
the VM and CoderOS, `down` works and the pylon stays off.

The VM costs about $0.78 an hour as an `n2-standard-16`. To keep it cheaper,
stop it and change the machine type to `n2-standard-8`, which still holds
the 6.5 GB model with room to spare.

## What this run did not cover

- **Paid jobs.** The Spark wallet takes only mainnet payments, and test
  sales need a funded testnet Lightning node, so no paid job ran. Rehearse
  the paid path with `cargo run -p pylon --features fixture --example
  paid_field`, which runs on an in-process relay and test Lightning.
- **Owner links and payouts.** None of the pylons is linked, so earnings
  would go to each pylon key's party.
- **Surviving a reboot.** The Mac job and the CoderOS unit last until a
  restart; only the VM's system unit comes back on its own.
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
