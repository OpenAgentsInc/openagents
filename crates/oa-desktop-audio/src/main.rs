use std::io::{self, BufRead, Write};
use oa_desktop_audio::{HelperCommand, MediaLifecycle};

fn main() {
    // Closed newline-delimited JSON control protocol. Raw media never crosses
    // this channel; production capture/socket/playback backends remain inside
    // this process and die automatically when stdin closes with the parent.
    let stdin = io::stdin(); let mut stdout = io::stdout(); let mut lifecycle = MediaLifecycle::default();
    for line in stdin.lock().lines() {
        let state = match line.ok().and_then(|line| serde_json::from_str::<HelperCommand>(&line).ok()) {
            Some(command) => lifecycle.apply(command).clone(),
            None => oa_desktop_audio::HelperState::Refused { reason: "invalid_control_frame".into() },
        };
        if serde_json::to_writer(&mut stdout, &state).is_err() { break }
        if writeln!(&mut stdout).is_err() || stdout.flush().is_err() { break }
    }
}
