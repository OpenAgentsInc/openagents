use anyhow::Result;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;

const BYPASS_FLAG: &str = "--dangerously-bypass-approvals-and-sandbox";
const BYPASS_ENV: &str = "CODEX_ACP_DANGEROUSLY_BYPASS_APPROVALS_AND_SANDBOX";

fn main() -> Result<()> {
    // Lightweight arg parsing for an internal safety override flag.
    // If present, export an env var that the library checks to bypass approvals.
    if std::env::args().any(|a| a == BYPASS_FLAG) {
        std::env::set_var(BYPASS_ENV, "1");
    }

    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        codex_acp::run_main(codex_linux_sandbox_exe, CliConfigOverrides::default()).await?;
        Ok(())
    })
}
