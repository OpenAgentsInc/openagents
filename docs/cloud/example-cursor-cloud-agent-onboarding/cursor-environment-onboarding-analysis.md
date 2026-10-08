# Cursor environment onboarding: observed workflow and port analysis

Status: evidence analysis, October 8, 2026. The proposed implementation is in
[environment onboarding](environment-onboarding.md). The complete action ledger
is in [the tool sequence](cursor-environment-onboarding-tool-sequence.md).
The retained [Cursor evidence](README.md)
contains the original responses, scripts, logs, HTML, and recovery audit.

This analysis describes the one recorded setup of OpenAgents. It does not
establish that every Cursor environment follows the same implementation.
Cursor's internal system command bodies and snapshot storage implementation
are not exposed in the evidence.

## The process to port

Cursor combines four things into one workspace: a setup conversation that can
inspect and repair the machine, a reviewable install/start recipe, asynchronous
environment snapshot and build operations, and a fresh verifier followed by
explicit Save. Future agents use the saved result. The conversation explains
progress, while the environment backend owns the build and saved state.

OpenAgents already has the compute, source-transfer, task, and observation
primitives needed for most of this experience. It needs a repository environment
owner that joins them into a durable lifecycle. The first port should use
dedicated Boat machines through the operator lane. The current shared GCE pool
and retail v1 contract do not admit arbitrary machine installation, saved
customer images, or this parent/child setup workflow.

## Evidence and ordering

The run is `run-7b90ddb0-9d8b-43cb-870a-22fdd0e4fd31`, under agent
`bc-83e18906-00e2-436c-978b-13a4932f58b0`. It runs from 10:11:06 AM to
10:38:19 AM Central time, October 8, 2026. All times below use
America/Chicago, UTC−05:00.

The earlier complete-record summary used approximate time windows. The times
here come from SSE event IDs, command records, build logs, and the saved
environment API response. Call numbers refer to the
[121-call ledger](cursor-environment-onboarding-tool-sequence.md).
Concurrent calls are shown in their recorded submission order.

There are 120 parent provider calls marked completed. Their results are
recoverable across provider events, 99 interaction-level completions, and the
SDK conversation. One additional `pr_management` event remains running with
no response. The SDK also retains seven child verifier calls. The PR state is
independently confirmed by GitHub, but its Cursor tool response remains absent.
These limits matter when turning an agent summary into evidence of success.

| Time | Stage | What actually changes |
| --- | --- | --- |
| 10:11:06 | Setup run created | A conversation/run exists; this is not a saved environment. |
| 10:13:07–10:15:17 | Discover tools, source, and machine | Five-stage checklist, repository reads, environment metadata, package and capacity inventory. |
| 10:16:08–10:18:56 | Write, run, and repair install recipe | Initial script includes both Cargo workspaces; phone locked fetch fails; script is narrowed to the root workspace. |
| 10:19:18–10:21:33 | Repeat installation | Two further invocations produce successful rerun/version evidence. |
| 10:21:45–10:26:09 | Exercise repository and preserve logs | ATIF/lease tests, targeted Nostr test, formatting, artifact copies. No application is launched. |
| 10:26:26–10:26:46 | Publish repository guidance separately | AGENTS.md edit, commit/push, draft PR #10979. This precedes image verification. |
| 10:26:55–10:30:58 | Capture exploration machine | Snapshot request, four status reads, then snapshot ready. |
| 10:31:09–10:35:29 | Build environment candidate | Draft build with explicit recipe plus exploration snapshot; source clone, runtime assets, recipe rerun, output snapshot. |
| 10:36:32–10:37:54 | Verify in a fresh machine | Child agent boots the exact build, checks versions, offline fetch, and one behavioral test. |
| 10:37:54–10:38:19 | Propose and summarize | Recipe proposal references the verified build; agent finishes and asks for Save. |
| 10:38:37 | Saved environment observed | Later saved-environment API record has this update time; the saved UI message is present in the 10:47 HTML. |

## 1. Start from a setup objective, not a generic coding task

The user invokes `/create-environment /env-setup` and asks Cursor to follow the
environment workflow, run relevant applications, and demonstrate the environment
end to end. The repository is `OpenAgentsInc/openagents`.

The agent creates five checklist items: understand the codebase, generate the
setup script, take a snapshot, verify the build in a subagent, and verify
success/show the result card. It discovers tool schemas before invoking the
environment tools. This is a guided agent workflow, rather than a fixed
package-install script selected without inspection.

The first environment-info response says no environment is linked and egress
is unrestricted. That response separates the current machine from an existing
saved environment. Later metadata reports an override environment recorded
through the build trigger. The evidence does not show every internal action
that allocates that draft identity.

**Port:** create a setup-purpose canonical task attached to a project environment
draft. Retain the selected repository, source revision, execution profile,
machine policy, and bounds before provisioning. Reuse Coder's agent loop and
task store; the environment owner supplies lifecycle tools and state.

## 2. Discover the repository's real build boundary

Calls 4–55 inspect contributing files, toolchain manifests, Dockerfile,
verification guidance, README, Cargo manifests, cloud setup, leases, Nix build
inputs, native build dependencies, gateway/oak/Nostr entry points, lock files,
Git dependencies, patches, and configuration. Reads use offsets/limits where
necessary; the agent does not rely on one README.

The machine inventory establishes Ubuntu 24.04.4 LTS, four cores, about 15 GiB
of reported memory, sudo access, and substantial free disk. Rust 1.97.1 is
already active through the repository toolchain. Python, Git, clang,
pkg-config, and CMake are present. The OpenSSL development metadata and other
required native tools are missing. The agent checks the login-shell PATH as
well as the current shell, because an interactive command succeeding does not
prove a later install/start process will find it.

The agent reads the existing OpenAgents host installer and derives package
choices from it. It also recognizes that `crates/openagents-mobile` is a
separate workspace. Gateway/application exploration does not result in an
application launch.

**Port:** retain an inventory artifact with exact versions, architecture, OS,
resource shape, workspace boundaries, required services, and source paths that
support the plan. A runtime base image and a repository environment recipe
are separate inputs. Do not silently promote repository instructions or agent
preferences into authority to install software on a shared host.

## 3. Generate a recipe, run it, and repair a real failure

Call 56 writes `/home/ubuntu/work/cloud-agent-install.sh`. Call 57 launches it
in a tmux session and redirects the output to a log. The initial recipe installs
native packages, installs/selects the pinned Rust toolchain, and attempts
locked dependency fetches for both Cargo workspaces.

The first run exposes a failure: the phone workspace's checked-in lock file
would need an update. The agent reads the logs and lock files and executes
separate root/mobile fetch and metadata commands at 10:18:33 and 10:18:44.
The root offline fetch works; the locked mobile operation exits 101. Call 67
rewrites the recipe to fetch only the root workspace and records the phone
limitation rather than changing its lock file.

The requested log `grep_search` has a truncated provider result. Its retained
interaction result includes workspace matches, while the next shell invocation
runs `rg` directly against the install log. This is a useful example of why a
tool's requested arguments and its actual response must both be retained.

The final [51-line install script](cloud-agent-install.sh)
does the following, in order:

1. Enable strict shell error handling and noninteractive apt.
2. Update apt metadata.
3. Install compiler/build tools, protobuf, OpenSSL/SQLite/libclang headers,
   certificates, Git, curl, ripgrep, jq, archive tools, procps, bubblewrap,
   and Python.
4. Install rustup only when absent, and expose its command shims.
5. Install Rust 1.97.1 with rustfmt/clippy, then select it as default.
6. Create the work/cache directory.
7. Fetch the root workspace with `--locked`.

The script does not provide PostgreSQL, fetch Psionic, resolve the phone
workspace, or define a start script. These are observed limits of this source
revision, not permanent facts about the current repository.

**Port:** preserve each recipe revision, failure, repair, and rerun separately.
Bind the final recipe bytes to the build. Execute commands through a typed
observation contract that retains stdout, stderr, and the actual process exit.
The recorded shell sometimes prints a marker after a pipeline; an enclosing
shell's success or the last pipeline program's status is not a reliable
replacement for the original command's result.

## 4. Establish idempotence and a bounded useful check

Calls 70 and 73 run the revised installer again. The parent report says
idempotence passed twice, and the saved rerun evidence records exit 0 and tool
versions. This supports rerunning the recipe on an already configured machine.
It does not by itself prove a recipe works on a clean base.

At 10:21:45 the agent launches `cargo test --offline -p atif -p coder-lease`
using a target directory outside the checkout and `CARGO_BUILD_JOBS=2`.
It waits and inspects the log. At 10:23:51 it launches the targeted Nostr
signed-agreement test. At 10:26:09 it runs focused formatting and copies logs
to Cursor's artifact directory.

The final logs record:

- ATIF: 56 library tests, 3 integration tests, and 1 doctest passed.
- coder-lease: 66 library tests, 2 holder-recovery tests, and 5 shim tests passed.
- The selected Nostr test passed, with 449 filtered out.
- Formatting is clean.

An intermediate progress-inspection invocation returns an error, while the
actual retained test log ends `EXIT:0`. The process that checks progress and
the tested process have distinct outcomes. The agent uses tmux to keep work
alive and poll logs; those are implementation workarounds rather than the
product feature to copy.

No application-level interaction is exercised. The fresh verifier is later
instructed not to start a server. The original end-to-end request is therefore
only partially satisfied, even though the environment is useful for these
crate tests.

**Port:** freeze an acceptance profile that identifies whether the result is
toolchain-only, CLI, service, or graphical application qualification. Reuse
build leases and browser helpers. Provide durable background-command IDs,
cursors, exit statuses, and cancellation instead of tmux/log polling in the
setup agent's prompt. Preserve the difference between package tests, startup
readiness, and an application demonstration.

## 5. Separate environment setup from source publication

At 10:26:26 the agent adds 20 lines of Cloud-specific guidance to AGENTS.md.
At 10:26:32 it creates a branch, commits `71c16fbba8`, and pushes.
At 10:26:42 a PR-management event starts; GitHub records draft PR #10979 at
10:26:46. The PR describes the toolchain, target directory, lower compile
parallelism, and excluded workspaces. Its exact Cursor tool response is not
retained.

This source publication occurs before the environment snapshot, build, or
fresh verifier. The environment's saved state and the documentation PR's
review/merge state are independent. No merge is shown.

**Port:** a setup session may propose a recipe file and repository guidance
patch. Retain those as reviewable source artifacts. Save must activate an
environment version, not implicitly commit, push, open a PR, merge, or deploy.
Use the existing publication owner when the user separately authorizes those
effects. An optional documentation PR must not be a dependency of a useful
environment.

## 6. Capture the explored machine, then build a draft environment

Call 96 starts snapshot
`snapshot-20261008-c299d928-d12c-4169-ab8e-0b22d49b2f5a` at 10:26:55.
The response says upload continues in the background and instructs the agent
to check readiness. Four checks return creating, creating, creating, then
ready at 10:30:58. The exploration snapshot takes about four minutes.

Call 108 starts a draft environment build at 10:31:09 with two explicit inputs:
the install script body and the exploration snapshot ID. The response returns
environment ID `7ce2f49e-c32a-11f1-bb68-864e54d14197` and build ID
`bld-20261008-26522d23-d3aa-427c-b6f2-3fc6ffbabdb8`. It states that draft
builds do not affect new agents until activated; activating this build also
saves its supplied configuration.

This is two distinct operations: preserve the explored disk, then produce a
build that has install configuration and a reusable output identity.
The evidence does not explain the provider's block/filesystem format or which
paths the exploration snapshot contains.

**Port:** preserve this visible separation where useful, but do not require two
expensive snapshots when one recipe-based build suffices. An exploration
snapshot can accelerate later work and retain a paused session. Qualification
must still record whether the final image was built from a clean admitted base
or an explored disk. A successful rerun on a populated snapshot is not proof
of clean-base reproducibility.

## 7. The backend prepares source, runtime, user dependencies, and output image

The [build log](environment-build-install-logs.txt)
shows the following order:

| Time | Backend action |
| --- | --- |
| 10:31:23.809 | A preceding hidden install step exits 0; its command body is not exposed. |
| 10:31:24.152 | Environment setup span begins. |
| 10:31:24.214–10:33:28.432 | Clone workspace, switch/reset main, finish source setup. |
| 10:33:28.460–10:33:51.239 | Install/check exec daemon, Cloud assets, desktop/VNC, Chrome, locales, fonts, themes, artifact paths, Git configuration, gh link, and agent-store FUSE. Many existing assets are skipped as already installed. |
| 10:33:51.272–10:33:51.537 | Start core-dump handling, desktop initialization, and exec daemon. |
| 10:33:51 onward | Run the supplied user installation; its packages/toolchain/fetch output follows. |
| 10:33:57.855 | User install exits 0. |
| 10:33:57.971 | Prepare the output snapshot. |
| 10:33:59.634–10:35:29.032 | Create/finish the build snapshot. |
| 10:35:29.032 | Warming skipped because the build is draft. |

The build record moves from IN_PROGRESS to SUCCEEDED and assigns the build ID
as its user-facing snapshot identity. Build duration is about 4 minutes
17 seconds, measured from its recorded creation/completion times.
The agent discovers success on its 10:36:08 status read.

The clone selects main during the build. The retained log does not establish
an exact source commit for that checkout. The setup session's source branch
and build source therefore must not be assumed identical. The system assets
are runtime prerequisites, not proof that an OpenAgents application started.

**Port:** retain base/runtime/source/recipe identities before build dispatch.
Keep the trusted runtime image separate from repository dependencies.
Build/install/start spans need independent results. Reuse the existing Coder
runtime; desktop and Chrome are profile choices, not universal prerequisites.
Persist the output image identity before publishing success.

## 8. A fresh verifier boots the exact build

At 10:36:32 the parent creates child
`bc-3a90601a-d753-5a94-8be4-6fb84bda6279` with
`machine.newCloudVm.environmentBuildId` set to the build ID. It uses a
separate agent and a separate machine. The prompt forbids repository mutation,
commit/push/PR, production endpoints, and external-service mutation.

The verifier performs seven tool calls. It searches local metadata, prints
versions, calls environment-info, performs root offline fetch, and runs the
corrupt lease-table behavioral test with its own target directory. The local
filesystem search does not find the build identity; the environment-info
response establishes it and reports gitSetup reuse, warmFork cold, and
resolution resolved.

The child sees Rust/cargo 1.97.1, rustfmt/clippy, Python 3.12.3, protoc 3.21.12,
OpenSSL 3.0.13, SQLite 3.45.1, bubblewrap 0.9.0, and Git 2.43.0.
Root `cargo fetch --locked --offline` exits 0. The selected lease test passes.
The start-user directory is absent and no server starts, as the parent
requested.

This is the strongest evidence of future-machine readiness in the run:
the result is tied to a fresh machine from the output build rather than the
setup machine that the agent repaired.

**Port:** keep explicit setup-task → build → image → verifier-task →
verifier-machine lineage. Run frozen deterministic checks independently of the
setup agent's narrative. The current protected source-candidate verifier has
different write authority; environment qualification needs a separate adapter
that can execute on its own writable image fork.

## 9. Show the tested proposal, then save

At 10:37:54 the parent completes the checklist, reads current environment
metadata, and calls propose-environment-json with the verified build ID and the
final install body. The returned start script is empty. The proposal tool says
the user can review/edit scripts before saving.

At 10:38:19 the run finishes and asks the user to click Save. The 10:47 export
contains “This environment is saved,” while another panel still says Draft.
The saved-environment API list independently contains the environment, updated
at 10:38:37.614. This confirms saved-environment existence after the run.
It does not expose the exact UI request that performed activation.

**Port:** Save must be a native, revision-fenced promotion of the exact verified
recipe/image. If a user edits the proposed script after verification, create a
new draft and require a new build/check before it can be saved as verified.
Display saved identity and active revision consistently across the chat card,
environment panel, and future task selector.

## What to retain, improve, or omit

| Cursor behavior | OpenAgents decision | Reason |
| --- | --- | --- |
| Guided setup conversation and five-stage progress | Retain the experience | Users can steer discovery and fixes while native state survives disconnects. |
| Tool/schema discovery | Reuse Coder's existing tool registration | A new agent engine or dynamic-schema service is unnecessary for this feature. |
| Inspect repository before choosing packages | Retain | Existing manifests and workspaces determine the recipe. |
| Idempotent installation and visible repair | Retain; freeze revisions | Failed attempts explain why the final recipe differs. |
| Mutable explored-machine snapshot | Optional acceleration | Useful for continuation, but clean-base qualification must remain explicit. |
| Asynchronous build/readiness polling | Retain native operations; improve recovery | Lost replies must reconcile the original build rather than create another. |
| Fresh agent on exact output build | Retain | It tests the future-machine path rather than the setup machine. |
| Empty start script despite an end-to-end objective | Improve | Qualification level and omitted application checks must be visible. |
| Source reset to main during build | Replace with exact source identity | Avoid a moving source changing the recipe's meaning during verification. |
| Local tmux sessions and sleeps | Replace with durable command owner | Preserve exit status, reconnect, cancellation, and original byte output. |
| Direct Cargo checks | Reuse targeted checks under build leases | Honor resource/disk policy and separate target slots. |
| PR publication inside setup | Optional independent operation | Environment Save does not need repository publication. |
| Runtime/desktop system assets | Reuse Coder runtime; choose desktop per profile | Do not recreate Cursor exec-daemon, FUSE store, VNC, or an entire desktop by default. |
| Retention-limited streams and multiple transcript schemas | Improve before launch | Archive full authorized results once; views may remain bounded. |
| Draft/build/saved status split | Retain; strengthen UI consistency | “Agent finished” and “environment active” are separate facts. |

The proposed native owner, mappings, implementation slices, and acceptance
criteria are in [the port specification](environment-onboarding.md).
