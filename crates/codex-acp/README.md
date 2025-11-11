# codex-acp (shim)

This folder contains source files for reference only and is not intended to be
built inside the OpenAgents repository. The actual `codex-acp` project lives in
its own repository and depends on the full ACP runtime and Codex crates.

To use codex-acp with OpenAgents:

1. Build the real codex-acp repository:

   cd /Users/christopherdavid/code/codex-acp
   cargo build --release

2. Point OpenAgents to the built binary via env var so the backend can spawn it:

   export OA_CODEX_ACP_ROOT=/Users/christopherdavid/code/codex-acp
   # or explicitly:
   export OA_ACP_AGENT_CMD=/Users/christopherdavid/code/codex-acp/target/release/codex-acp

3. Run the app and use the “Test ACP” button.

Note: We no longer attempt to cargo-run `codex-acp` from inside the Tauri process
(to avoid toolchain/edition/missing-resource issues). The backend expects a
prebuilt `codex-acp` binary available either in:

- OA_ACP_AGENT_CMD
- OA_CODEX_ACP_ROOT/target/{release,debug}/codex-acp
- PATH (named `codex-acp`)

If none is found, the backend returns a clear error with build hints.
