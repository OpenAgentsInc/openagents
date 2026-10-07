//! `pylon`: the same commands as `openagents pylon`, in a small binary a
//! provider machine can build without the rest of the workspace.

fn main() -> std::process::ExitCode {
    let mut words: Vec<String> = std::env::args().skip(1).collect();
    let json = if let Some(i) = words.iter().position(|w| w == "--json") {
        words.remove(i);
        true
    } else {
        false
    };
    std::process::ExitCode::from(pylon::cli::run(json, &words))
}
