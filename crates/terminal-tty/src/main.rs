//! Plain terminal shell entry point.
fn main() {
    #[cfg(unix)]
    {
        let root = std::env::current_dir().expect("current directory");
        let shell = std::env::var_os("SHELL")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/bin/sh".into());
        let helper = std::env::var_os("OPENAGENTS_TTY_HELPER")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "openagents".into());
        match terminal_tty::runner::run(&root, &shell, &helper) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("openagents-tty: {error}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("Hook-only shells are unavailable on this platform.");
        std::process::exit(1);
    }
}
