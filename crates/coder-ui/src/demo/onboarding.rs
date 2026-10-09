//! A synthetic repository environment setup, including its first later task.
//!
//! Every operation and result is a local fixture. The proposed environment
//! owner, provider resources, credentials, and checks are not connected.

use super::{
    agents::{DemoAgent, DemoMessage},
    tools::{PluginCall, ToolCall, ToolKind, ToolState},
};

pub const DEMO: DemoAgent = DemoAgent {
    name: "environment-setup",
    task: "OpenAgents root Rust · inspect → install → build → verify → save → reuse",
    tokens: "14.6k",
    elapsed_seconds: 647,
    conversation: &[
        DemoMessage::User(
            "Set up OpenAgents for cloud Rust work. Install what it needs, check a fresh machine, and save the environment for later tasks.",
        ),
        DemoMessage::Assistant(
            "This is a synthetic walkthrough. I'll inspect the repository, prepare an install recipe, and verify a separate fresh machine before offering Save.",
        ),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "inspect",
            input: "project=openagents · source=a28e2c8992 · purpose=repository setup",
            output: "Synthetic result\nNo environment configured\nPlatform: Boat · Linux x86-64\nExecutor: headless Coder / Codex\nTrusted base: demo-runtime-2026-10-08\nSetup machine: demo-setup-01\nRecipe: no draft yet\nStage: Inspecting repository",
            state: ToolState::Complete,
        }),
        DemoMessage::Tool(ToolCall {
            kind: ToolKind::Read,
            input: "AGENTS.md · rust-toolchain.toml · Cargo.toml · crates/openagents-mobile/Cargo.toml · scripts/cloud/coder-host-setup.sh",
            output: "Synthetic repository inspection\nPinned Rust: 1.97.1\nRoot and phone: separate Cargo workspaces\nNative inputs: protobuf, OpenSSL, SQLite, libclang, compiler tools\nBuild policy: lease; target outside checkout\nDeclared application services: none",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "admission.review",
            input: "demo-setup-01 · exact source/base · dedicated machine · root Rust checks · install privileges · 20-minute deadline",
            output: "Synthetic approval saved: demo-approval-01\nProject/workspace and source pinned\nSystem installation restricted to the dedicated setup/builder machines\nSelected model credential: private fixture reference only\nNetwork: approved package and source registries\nSource publication: not requested\nProfile: Rust toolchain/library\nNo real resource allocation or provider request",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "The repository needs protobuf and native development headers. It also has two Cargo workspaces. I'll install the packages and try locked dependency fetches for both; every command result stays in the record.",
        ),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "recipe.update",
            input: "expected_draft=0 · native packages + Rust 1.97.1 · root and mobile locked fetch · no start services",
            output: "Synthetic draft created: demo-recipe-r1\nInstall: native tools → pinned Rust → root fetch --locked → mobile fetch --locked\nStart: empty; library profile\nChecks: pinned toolchain, offline root fetch, representative ATIF/lease behavior\nStage: Installing · Save unavailable",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "command.run",
            input: "command=demo-install-01 · recipe=demo-recipe-r1 · cwd=/workspace/openagents · approval=demo-approval-01",
            output: "Synthetic failed command: demo-install-01\nstdout:\n  Native packages installed\n  Rust 1.97.1 selected\n  Root cargo fetch --locked: complete\nstderr:\n  error: the lock file crates/openagents-mobile/Cargo.lock needs to be updated but --locked was passed\nProcess exit: 101\nFull output saved\nStage: Install needs repair",
            state: ToolState::Failed,
        }),
        DemoMessage::Assistant(
            "The native tools and root dependencies are ready. The phone workspace refuses its locked fetch because its lockfile needs an update. The failed attempt stays in the record. I can set up the Linux root workspace without changing that file.",
        ),
        DemoMessage::User("Root workspace is enough. Keep the phone lockfile unchanged."),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "recipe.update",
            input: "expected_draft=1 · remove mobile fetch · retain locked root fetch · profile=root-rust-library-v1",
            output: "Synthetic immutable revision: demo-recipe-r2\nParent: demo-recipe-r1\nDiff:\n  - cargo fetch --manifest-path crates/openagents-mobile/Cargo.toml --locked\n  + # Root workspace only; phone is excluded\nSource files changed: 0\nChecks frozen: demo-root-check-plan-v1\nStage: Repairing install · Save unavailable",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "command.run",
            input: "command=demo-install-02 · recipe=demo-recipe-r2 · dedicated setup machine",
            output: "Synthetic command: demo-install-02\nstdout:\n  Native package requirements satisfied\n  rustc 1.97.1; cargo 1.97.1\n  protoc 3.21.12; OpenSSL 3.0.13; SQLite 3.45.1\n  Root cargo fetch --locked: complete\nstderr: empty\nProcess exit: 0\nOriginal failed command demo-install-01 remains failed\nStage: Install passed",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "The repaired recipe succeeds. I'll run the frozen root checks, then rebuild from the clean base so the reusable image doesn't depend on manual fixes in this setup machine.",
        ),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "checks.run",
            input: "demo-root-check-plan-v1 · protected check artifacts · build lease · external target slot",
            output: "Synthetic setup check results\nToolchain inventory: matches Rust 1.97.1\ncargo fetch --locked --offline: exit 0\nATIF representative behavior: passed\nLease corrupt-table behavior: 1 passed; 0 failed\nFocused formatting: exit 0\nApplication startup: not applicable to library profile\nDeclared checks passed; full output saved",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "build.start",
            input: "request=demo-build-request-02 · source=a28e2c8992 · recipe=demo-recipe-r2 · base=demo-runtime-2026-10-08",
            output: "Synthetic build request saved: demo-build-02\nBuilder: demo-builder-02; separate from setup machine\nMode: clean-base install\nExact source, recipe, runtime, and check-plan inputs frozen\nProvider operation: demo-build-operation-02\nStage: Building · installation in progress",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "build.status",
            input: "build=demo-build-02 · resume original operation; no replacement request",
            output: "Synthetic build result\nClean-base install exit: 0\nCapture policy: required Cargo/Rust/toolchain paths included\nPrivate credential mounts: removed before capture\nSnapshot name: demo-openagents-env-02; owned and never replaced\nSnapshot ID: demo-snapshot-02\nManifest: demo-candidate-manifest-02\nSnapshot state: ready\nBuilder deletion: acknowledged\nStage: Candidate ready · fresh verification required",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "The clean build is ready. A separate verifier will restore that exact candidate and check it before reinstalling anything. It will report problems rather than repair the image it is checking.",
        ),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "verify.start",
            input: "build=demo-build-02 · candidate=demo-candidate-manifest-02 · plan=demo-root-check-plan-v1",
            output: "Synthetic verification saved: demo-verify-02\nChild task: demo-fresh-verifier\nFresh machine: demo-verifier-03\nExact candidate snapshot: demo-snapshot-02\nProtected check plan: demo-root-check-plan-v1\nSource/image repair authority: none\nStage: Restoring fresh image",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "verify.restore",
            input: "child=demo-fresh-verifier · machine=demo-verifier-03 · expected_manifest=demo-candidate-manifest-02",
            output: "Synthetic child verifier result\nRestored name-to-snapshot mapping: exact\nManifest: matches candidate\nRequired files: present\nMachine differs from setup and builder\nToolchain inventory: matches recipe\nUntouched baseline: checked before any recipe rerun\nStage: Checking fresh image",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "verify.checks",
            input: "child=demo-fresh-verifier · untouched demo-verifier-03 · frozen root checks · build lease",
            output: "Synthetic child check results\ncargo fetch --locked --offline: exit 0; cached dependencies available\nATIF representative behavior: passed\nLease corrupt-table behavior: 1 passed; 0 failed; exit 0\nRepository edits: 0\nMissing packages repaired: 0\nApplication server: not required by library profile\nFull child call arguments and outputs saved",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "verify.idempotence",
            input: "recipe=demo-recipe-r2 · disposable fork=demo-rerun-04 · after untouched baseline checks",
            output: "Synthetic idempotence result\nFork uses demo-snapshot-02\nExact recipe rerun: exit 0\nDeclared toolchain/dependency inventory: unchanged\nSealed candidate: unchanged\nFork and verifier deletion: acknowledged\nStage: Verification passed",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "Fresh restore passes, and the exact recipe also reruns successfully on a disposable fork. The root tools and checks are ready. Phone builds and application services are outside this version's scope.",
        ),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "propose",
            input: "recipe=demo-recipe-r2 · build=demo-build-02 · verifier=demo-verify-02 · expected_project_selection=0",
            output: "Synthetic review: OpenAgents · Root Rust\nCandidate: demo-env-v1\nSource: a28e2c8992\nInstall revision: 2; start services: none\nImage: demo-snapshot-02; manifest: demo-candidate-manifest-02\nProfile: root Rust toolchain/library\nClean install, untouched fresh checks, and idempotence: passed\nEvidence: every recorded call has a result; no gaps or truncation\nSetup, builder, verifier, rerun cleanup: acknowledged\nUsage: synthetic elapsed time only; no provider bill or purchase\nStage: Ready to save; project default still unchanged",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "Ready to save OpenAgents · Root Rust v1. It contains the native tools, pinned Rust toolchain, and cached root dependencies. Review the install revision, fresh verifier, and complete output before selecting Save. Editing the recipe requires a new build and verification.",
        ),
        DemoMessage::User("Save this verified version as the project's environment."),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "promote",
            input: "reviewed_request=demo-save-02 · expected_draft=2 · expected_project_selection=0 · version=demo-env-v1 · verifier=demo-verify-02",
            output: "Synthetic promotion result\nRequest: demo-save-02\nReviewed recipe/image/check identities match\nSelected version: demo-env-v1\nProject selection revision: 0 → 1\nOutcome: saved\nRepeated request returns this same result\nSource publication: none\nStage: Saved",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "Saved as OpenAgents · Root Rust v1. New project tasks will use this verified version.",
        ),
        DemoMessage::User("Start a new task with that environment and check the lease behavior."),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "resolve",
            input: "task=demo-first-task · project_selection=1 · source=a28e2c8992 · requested_check=lease corrupt-table behavior",
            output: "Synthetic task start\nEnvironment pinned before allocation: demo-env-v1\nExact snapshot: demo-snapshot-02\nManifest: demo-candidate-manifest-02\nRecipe: demo-recipe-r2\nSource/toolchain/lock compatibility: unchanged\nMachine: demo-work-05; a new machine\nLater promotion cannot change this task's pinned environment",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "startup",
            input: "task=demo-first-task · machine=demo-work-05 · pinned_version=demo-env-v1",
            output: "Synthetic new-task startup\nSnapshot mapping and restored manifest: match v1\nRequired tools and cache files: present\nPer-boot services: none\nStartup outcome: ready\nInstall repair: not needed\nStage: First task running with saved v1",
            state: ToolState::Complete,
        }),
        DemoMessage::Plugin(PluginCall {
            plugin: "environment",
            operation: "command.run",
            input: "task=demo-first-task · frozen lease corrupt-table check · build lease · target outside checkout",
            output: "Synthetic later-task command\nrunning 1 test\nlease corrupt-table behavior ... ok\ntest result: ok. 1 passed; 0 failed\nProcess exit: 0\nEnvironment: demo-env-v1\nFull output saved\nWork machine deleted\nSaved environment image: unchanged",
            state: ToolState::Complete,
        }),
        DemoMessage::Assistant(
            "The new task restored v1 on a different machine and passed the lease check without an install repair. The environment is saved, every recorded result is available, and temporary-machine cleanup is acknowledged. This preview made no real provider requests or purchases.",
        ),
    ],
};
