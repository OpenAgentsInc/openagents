//! The scripted demo conversations. Every message, call, output, machine,
//! and identifier here is synthetic: nothing runs, no provider or account
//! is involved, and no value comes from a real run.
//!
//! The environment onboarding chat follows the proposed flow in
//! `docs/cloud/example-cursor-cloud-agent-onboarding/environment-onboarding.md`
//! and the reference tool order in `cursor-environment-onboarding-tool-sequence.md`:
//! discover, write a recipe, install and repair, build a clean image, verify
//! a fresh machine, review, save.

use maud::{Markup, Render, html};
use openagents_ui::content::{
    ActivityStatus, CodeBlock, FileChanges, MarkdownRoot, MarkdownSize, ResultCard, Step, Steps,
    Table, ToolCall, ToolGroup,
};
use openagents_ui::icons::Icon;
use openagents_ui::shell::Message;

/// One scripted chat.
pub(crate) struct DemoChat {
    /// The URL segment: `/demo/{slug}`.
    pub slug: &'static str,
    /// The title in the sidebar and the thread header.
    pub title: &'static str,
    /// The thread's turns, oldest first.
    pub turns: fn() -> Vec<Message>,
}

/// Every chat, in sidebar order. The first is `/demo`'s default.
pub(crate) const CHATS: [DemoChat; 3] = [
    DemoChat {
        slug: "environment",
        title: "Set up the repository environment",
        turns: environment,
    },
    DemoChat {
        slug: "lease-fix",
        title: "Fix the lease table reset",
        turns: lease_fix,
    },
    DemoChat {
        slug: "benchmark",
        title: "Benchmark tonight's build",
        turns: benchmark,
    },
];

/// The chat at `slug`.
pub(crate) fn find(slug: &str) -> Option<&'static DemoChat> {
    CHATS.iter().find(|chat| chat.slug == slug)
}

/// An assistant turn: prose, then any activity under it.
fn says(prose: Markup) -> Message {
    Message::assistant(MarkdownRoot::new(prose)).author("OpenAgents")
}

fn acts(content: impl Render) -> Message {
    Message::assistant(content).author("OpenAgents")
}

fn read(path: &str) -> ToolCall {
    ToolCall::new(Icon::FileDocument, "Read").detail(path)
}

fn searched(query: &str) -> ToolCall {
    ToolCall::new(Icon::Search, "Searched").detail(query)
}

fn ran(command: &str) -> ToolCall {
    ToolCall::new(Icon::Terminal, "Ran").detail(command)
}

fn output(text: &str) -> CodeBlock {
    CodeBlock::new(text).copyable(false)
}

const RECIPE: &str = "\
#!/usr/bin/env bash
# Install for the OpenAgents workspace (revision 1).
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends \\
  build-essential pkg-config clang cmake \\
  protobuf-compiler libprotobuf-dev \\
  libssl-dev libsqlite3-dev libclang-dev \\
  git ripgrep jq bubblewrap python3

rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
rustup default 1.97.1

cargo fetch --locked --manifest-path Cargo.toml
cargo fetch --locked --manifest-path crates/openagents-mobile/Cargo.toml";

const RECIPE_DIFF: &str = "\
 cargo fetch --locked --manifest-path Cargo.toml
-cargo fetch --locked --manifest-path crates/openagents-mobile/Cargo.toml
+# Root workspace only. The phone workspace is outside this environment.";

fn environment() -> Vec<Message> {
    vec![
        Message::user("Set up this repository so cloud agents can build and test it."),
        says(html! {
            p { "I'll work out how the repository builds, write an install recipe, and check it on a fresh machine before you save anything." }
        }),
        acts(
            ToolGroup::new("Explored the repository")
                .meta("6 files read, 3 searches")
                .call(searched("rust-toolchain* in /workspace"))
                .call(read("rust-toolchain.toml").body(output("[toolchain]\nchannel = \"1.97.1\"\ncomponents = [\"rustfmt\", \"clippy\"]")))
                .call(read("Cargo.toml"))
                .call(read("AGENTS.md"))
                .call(searched("pkg-config|libssl|protobuf|clang in /workspace"))
                .call(read("scripts/cloud/coder-host-setup.sh"))
                .call(read(".cargo/config.toml"))
                .call(searched("**/Cargo.lock").body(output("Cargo.lock\ncrates/openagents-mobile/Cargo.lock")))
                .call(read("docs/verification.md")),
        ),
        says(html! {
            p { "Here's what the environment needs:" }
            ul {
                li { "Rust " code { "1.97.1" } ", pinned in " code { "rust-toolchain.toml" } "." }
                li { "Native headers for protobuf, OpenSSL, SQLite, and libclang." }
                li { "Two Cargo workspaces: the root and the phone app." }
                li { "No services to start, so this is a toolchain environment." }
            }
            p { "This is the install recipe I'll try first:" }
        }),
        acts(html! {
            (CodeBlock::new(RECIPE).language("sh"))
            (ran("bash cloud-agent-install.sh")
                .status(ActivityStatus::Failed)
                .status_label("Exit 101")
                .open(true)
                .body(output("Native packages installed\nrustc 1.97.1 selected\nRoot cargo fetch --locked: complete\nerror: the lock file crates/openagents-mobile/Cargo.lock needs to be\nupdated but --locked was passed to prevent this")))
        }),
        says(html! {
            p { "The native tools and the root workspace installed. The phone workspace's lockfile doesn't resolve with " code { "--locked" } ", so its fetch failed." }
            p { "I can leave the phone workspace out of this environment and keep its lockfile as it is. Should I?" }
        }),
        Message::user("Yes, root workspace only. Don't change the lockfile."),
        acts(html! {
            (ToolCall::new(Icon::Pencil, "Edited").detail("cloud-agent-install.sh").open(true).body(CodeBlock::new(RECIPE_DIFF).language("diff")))
            (ran("bash cloud-agent-install.sh")
                .status_label("Exit 0")
                .body(output("Native package requirements satisfied\nrustc 1.97.1, cargo 1.97.1\nprotoc 3.21.12, OpenSSL 3.0.13, SQLite 3.45.1\nRoot cargo fetch --locked: complete")))
        }),
        says(html! {
            p { "The recipe passes now. Next I'll build a clean image from the trusted base and check it on a separate machine, so the saved environment doesn't depend on anything I fixed by hand." }
        }),
        acts(html! {
            (Steps::new("Build and verify")
                .step(Step::new("Build a clean image", ActivityStatus::Done).detail("Recipe revision 2 on the trusted base, exit 0"))
                .step(Step::new("Seal the snapshot", ActivityStatus::Done).detail("Snapshot ready; builder deleted"))
                .step(Step::new("Restore on a fresh machine", ActivityStatus::Done).detail("Manifest matches; every file restored"))
                .step(Step::new("Run the checks", ActivityStatus::Done).detail("Before any reinstall, nothing repaired"))
                .step(Step::new("Rerun the recipe", ActivityStatus::Done).detail("Same result; the image is unchanged")))
            (ToolGroup::new("Fresh machine checks")
                .call(ran("rustc --version && protoc --version").body(output("rustc 1.97.1\nlibprotoc 3.21.12")))
                .call(ran("cargo fetch --locked --offline").status_label("Exit 0"))
                .call(ran("cargo test --offline -p coder-lease a_corrupt_table_is_an_error_not_a_reset")
                    .status_label("1 passed")
                    .body(output("running 1 test\ntest table::a_corrupt_table_is_an_error_not_a_reset ... ok\n\ntest result: ok. 1 passed; 0 failed"))))
        }),
        says(html! { p { "Everything checks out on the fresh machine. Here's the version to review:" } }),
        acts(
            ResultCard::new("OpenAgents · Root Rust v1")
                .subtitle("Ready to save")
                .badge(ActivityStatus::Done.badge("Verified"))
                .fact("Source", html! { code { "a28e2c8" } })
                .fact("Toolchain", "Rust 1.97.1")
                .fact("Install recipe", "Revision 2")
                .fact("Start services", "None")
                .fact("Image", html! { code { "demo-snapshot-02" } })
                .fact("Checks", "3 of 3 passed")
                .body(MarkdownRoot::new(html! {
                    p { "Not covered: the phone workspace and application services. They need their own version." }
                }).size(MarkdownSize::Sm))
                .footer("Saving makes this the default for new tasks. Editing the recipe needs a new build and check."),
        ),
        Message::user("Save it."),
        acts(ToolCall::new(Icon::CheckCircle, "Saved environment").detail("Root Rust v1")),
        says(html! {
            p { "Saved. New tasks in this project start from Root Rust v1. Tasks already running keep the version they started with." }
        }),
    ]
}

const LEASE_DIFF: &str = "\
 pub fn load(path: &Path) -> Result<Table, LeaseError> {
     let bytes = fs::read(path)?;
-    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
+    serde_json::from_slice(&bytes).map_err(|source| LeaseError::Corrupt {
+        path: path.to_owned(),
+        source,
+    })
 }";

fn lease_fix() -> Vec<Message> {
    vec![
        Message::user("openagents lease list wipes the lease table when the file is corrupt. It should fail instead."),
        acts(
            ToolGroup::new("Investigated")
                .call(searched("fn load in crates/coder-lease/src"))
                .call(read("crates/coder-lease/src/table.rs"))
                .call(ran("cargo test -p coder-lease")
                    .status(ActivityStatus::Failed)
                    .status_label("1 failed")
                    .body(output("test table::a_corrupt_table_is_an_error_not_a_reset ... FAILED\n\nassertion failed: Table::load(&path).is_err()\n\ntest result: FAILED. 11 passed; 1 failed"))),
        ),
        says(html! {
            p { code { "Table::load" } " treats a parse error as an empty table, so the next write replaces the corrupt file. I'll return the error instead, so nothing is overwritten." }
        }),
        acts(html! {
            (ToolCall::new(Icon::Pencil, "Edited").detail("crates/coder-lease/src/table.rs").body(CodeBlock::new(LEASE_DIFF).language("diff")))
            (ToolCall::new(Icon::Pencil, "Edited").detail("crates/coder-lease/src/error.rs"))
            (ran("cargo test -p coder-lease").status_label("12 passed").body(output("test result: ok. 12 passed; 0 failed")))
        }),
        says(html! {
            p { "Fixed. A corrupt table now stops with an error that names the file, and the file is left as it was. The regression test passes with the rest of the crate." }
        }),
        acts(
            FileChanges::new()
                .file("crates/coder-lease/src/table.rs", 4, 1)
                .file("crates/coder-lease/src/error.rs", 6, 0),
        ),
    ]
}

fn benchmark() -> Vec<Message> {
    let families = Table::new()
        .label("Results so far")
        .header(["Task family", "Done", "Passed", "Failed"])
        .row(["Build and test", "12", "12", "0"])
        .row(["Debugging", "9", "8", "1"])
        .row(["Performance", "6", "5", "1"])
        .numeric(1)
        .numeric(2)
        .numeric(3);
    vec![
        Message::user("Run the terminal benchmark against tonight's build on a cloud machine."),
        acts(html! {
            (ToolCall::new(Icon::Play, "Started cloud job").detail("terminal-bench · 40 tasks · Root Rust v1"))
            (Steps::new("Cloud job")
                .step(Step::new("Start a machine", ActivityStatus::Done).detail("Root Rust v1 restored in 48 s"))
                .step(Step::new("Build the release", ActivityStatus::Done).detail("6 min 12 s"))
                .step(Step::new("Run the tasks", ActivityStatus::Running).detail("27 of 40 tasks"))
                .step(Step::new("Collect the results", ActivityStatus::Waiting))
                .step(Step::new("Delete the machine", ActivityStatus::Waiting)))
        }),
        says(html! {
            p { "27 of 40 tasks are done and 25 passed. Both failures ran out of time, so I'll look at those once the run finishes." }
        }),
        acts(families),
    ]
}
