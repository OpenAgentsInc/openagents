//! Commands run in the task checkout through `supervise`: a process group
//! of their own, a wall deadline, and capped output.

use std::path::PathBuf;
use std::time::Duration;

use crate::agent::Shell;
use crate::state::Observation;

/// Output past this many bytes a stream is counted and dropped.
const STREAM_CAP: usize = 32 * 1024;

/// How many trailing lines of each command's output the console shows.
const CONSOLE_LINES: usize = 40;

/// A shell bound to one checkout.
pub struct Checkout {
    pub workdir: PathBuf,
    pub deadline: Duration,
}

impl Shell for Checkout {
    async fn run(&mut self, command: &str) -> Observation {
        println!("  $ {command}");
        let mut prepared = std::process::Command::new("bash");
        prepared
            .arg("-c")
            .arg(command)
            .current_dir(&self.workdir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("PYTHONDONTWRITEBYTECODE", "1");
        let ended = supervise::Job::from_command(prepared)
            .bounded(supervise::Limits::within(self.deadline).keeping(STREAM_CAP))
            .run()
            .await;

        let mut output = ended.stdout.marked();
        if !ended.stderr.is_empty() {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(&ended.stderr.marked());
        }
        if let supervise::Ending::TimedOut = ended.ending {
            output.push_str(&format!(
                "\n[killed: the {}s command deadline passed]",
                self.deadline.as_secs()
            ));
        }
        if let supervise::Ending::Failed(why) = &ended.ending {
            output.push_str(&format!("\n[could not run: {why}]"));
        }

        print_tail(&output);
        println!(
            "  └ {} in {:.1}s, {} bytes",
            ended.ending,
            ended.elapsed.as_secs_f64(),
            ended.bytes()
        );
        Observation {
            exit: ended.ending.code(),
            output,
            truncated: ended.truncated(),
        }
    }
}

fn print_tail(output: &str) {
    let lines: Vec<&str> = output.lines().collect();
    let skip = lines.len().saturating_sub(CONSOLE_LINES);
    if skip > 0 {
        println!("  │ … {skip} earlier lines");
    }
    for line in &lines[skip..] {
        println!("  │ {line}");
    }
}
