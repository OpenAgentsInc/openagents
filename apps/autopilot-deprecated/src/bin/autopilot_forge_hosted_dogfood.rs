use std::process::ExitCode;

fn main() -> ExitCode {
    match autopilot_desktop::forge_hosted_harness::main_entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::from(1)
        }
    }
}
