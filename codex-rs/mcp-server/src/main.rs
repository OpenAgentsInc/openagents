use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_mcp_server::run_main;

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        run_main(codex_linux_sandbox_exe, CliConfigOverrides::default()).await?;
        Ok(())
    })
}
