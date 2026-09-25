//! `evidence.phase_timing`: run a command once more under a lightweight
//! profiler, bounded, and report where its time goes.
//!
//! No code is edited. A Python program runs under `cProfile`, with
//! `faulthandler` dumping the stack if the bound passes first; a shell
//! script runs traced, with a timestamp on every top-level command; any
//! other command runs under `perf stat` or `/usr/bin/time -v` when the
//! host has one, and with the host's own wall clock otherwise. Which of
//! those the host has is decided where the command runs, not here.

use serde::{Deserialize, Serialize};

/// How a command is profiled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profiler {
    /// `python3 -m cProfile`, with `faulthandler`.
    Python,
    /// `bash -x` with a timestamp on every top-level command.
    Shell,
    /// `perf stat`, `/usr/bin/time -v`, or the wall clock.
    Process,
}

/// The line that starts the profiler's report in the output.
pub const MARK: &str = "=== coder-one phase timing ===";

/// Rows of a profile the report shows.
pub const ROWS: usize = 12;

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A simple command: no pipe, list, redirection into a subshell, or
/// substitution, so wrapping it doesn't change what it runs.
fn simple(command: &str) -> bool {
    !command.contains(['|', ';', '&', '`', '$', '(', ')', '<', '>', '\n'])
}

/// Leading `NAME=value` words, split off.
fn env_prefix(words: &[&str]) -> usize {
    words
        .iter()
        .take_while(|w| {
            w.split_once('=').is_some_and(|(n, _)| {
                !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
        })
        .count()
}

/// The profiler for `command` and the command that runs it under the
/// profiler, whose inner bound is `inner_sec` seconds.
#[must_use]
pub fn plan(command: &str, inner_sec: u64) -> (Profiler, String) {
    let trimmed = command.trim();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let env = env_prefix(&words);
    let rest = &words[env..];
    let env_words = words[..env].join(" ");
    let bound = |inner: &str| {
        format!(
            "if command -v timeout >/dev/null 2>&1; then timeout {} {inner}; else {inner}; fi",
            if inner.starts_with("python") {
                format!("-s ABRT {inner_sec}")
            } else {
                inner_sec.to_string()
            }
        )
    };
    if simple(trimmed)
        && let Some(program) = rest.first()
        && matches!(*program, "python" | "python3")
    {
        // The interpreter's own flags stay; the target is a script or `-m`.
        let target = rest[1..]
            .iter()
            .position(|w| *w == "-m" || !w.starts_with('-'));
        if let Some(at) = target.filter(|at| !rest[1..1 + at].contains(&"-c")) {
            let flags = rest[1..1 + at].join(" ");
            let target_and_args = rest[1 + at..].join(" ");
            let inner = format!(
                "{} -X faulthandler {flags} -m cProfile -o \"$P\" {target_and_args}",
                program
            )
            .replace("  ", " ");
            let inner = if env_words.is_empty() {
                inner
            } else {
                format!("env {env_words} {inner}")
            };
            let script = format!(
                "P=\"${{TMPDIR:-/tmp}}/coder-one-profile-$$\"; {}; s=$?; echo {} >&2; \
                 if [ -s \"$P\" ]; then {program} -c \"import pstats,sys; \
                 pstats.Stats(sys.argv[1], stream=sys.stderr).sort_stats('cumulative')\
                 .print_stats({ROWS})\" \"$P\"; fi; rm -f \"$P\"; exit $s",
                bound(&inner),
                quote(MARK),
            );
            return (Profiler::Python, script);
        }
    }
    if simple(trimmed)
        && env == 0
        && let Some(program) = rest.first()
    {
        let script = match *program {
            "sh" | "bash" if rest.get(1).is_some_and(|w| w.ends_with(".sh")) => {
                Some(rest[1..].join(" "))
            }
            p if p.ends_with(".sh") => Some(rest.join(" ")),
            _ => None,
        };
        if let Some(script) = script {
            let inner = format!(
                "bash -c {}",
                quote(&format!(
                    "exec 9>\"$T\"; BASH_XTRACEFD=9; PS4='+ ${{EPOCHREALTIME}} '; set -x; . {script}"
                ))
            );
            let out = format!(
                "T=\"${{TMPDIR:-/tmp}}/coder-one-trace-$$\"; export T; {}; s=$?; echo {} >&2; \
                 head -c 200000 \"$T\" >&2; rm -f \"$T\"; exit $s",
                bound(&inner),
                quote(MARK)
            );
            return (Profiler::Shell, out);
        }
    }
    let inner = quote(trimmed);
    let out = format!(
        "if command -v perf >/dev/null 2>&1; then perf stat -x, -o \"${{TMPDIR:-/tmp}}/coder-one-perf-$$\" -- sh -c {inner}; s=$?; echo {mark} >&2; cat \"${{TMPDIR:-/tmp}}/coder-one-perf-$$\" >&2; rm -f \"${{TMPDIR:-/tmp}}/coder-one-perf-$$\"; \
         elif [ -x /usr/bin/time ]; then /usr/bin/time -v -o \"${{TMPDIR:-/tmp}}/coder-one-time-$$\" sh -c {inner}; s=$?; echo {mark} >&2; cat \"${{TMPDIR:-/tmp}}/coder-one-time-$$\" >&2; rm -f \"${{TMPDIR:-/tmp}}/coder-one-time-$$\"; \
         else sh -c {inner}; s=$?; echo {mark} >&2; fi; exit $s",
        mark = quote(MARK)
    );
    (Profiler::Process, out)
}

/// One profiled run, as the report reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profiled {
    pub stdout: String,
    pub stderr: String,
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub milliseconds: u64,
}

/// What follows [`MARK`] in `stderr`, or `None` when the mark never
/// printed.
fn after_mark(stderr: &str) -> Option<&str> {
    stderr.find(MARK).map(|at| &stderr[at + MARK.len()..])
}

/// The top rows of a `pstats` report.
fn python_rows(report: &str) -> Vec<String> {
    let mut lines = report.lines().skip_while(|l| !l.contains("ncalls"));
    let header = lines.next();
    let mut rows: Vec<String> = Vec::new();
    if let Some(header) = header {
        rows.push(header.trim_end().to_string());
    }
    rows.extend(
        lines
            .filter(|l| !l.trim().is_empty())
            .take(ROWS)
            .map(|l| l.trim_end().to_string()),
    );
    if rows.len() <= 1 {
        return Vec::new();
    }
    let total = report
        .lines()
        .find(|l| l.contains("function calls") && l.contains("seconds"))
        .map(|l| l.trim().to_string());
    total.into_iter().chain(rows).collect()
}

/// Top-level commands of a timestamped `bash -x` trace, with the seconds
/// each took: until the next top-level command, and the last until
/// `end_sec` after the first.
#[must_use]
pub fn shell_phases(trace: &str, total_sec: f64) -> Vec<(String, f64)> {
    // Each line's depth is its count of `+`; a deeper command repeats it.
    let mut all: Vec<(usize, f64, String)> = Vec::new();
    for line in trace.lines() {
        let depth = line.chars().take_while(|c| *c == '+').count();
        let Some(rest) = line[depth..].strip_prefix(' ') else {
            continue;
        };
        let Some((stamp, command)) = rest.split_once(' ') else {
            continue;
        };
        if depth > 0
            && let Ok(at) = stamp.parse::<f64>()
        {
            all.push((depth, at, command.to_string()));
        }
    }
    let Some(first) = all.first().map(|(_, t, _)| *t) else {
        return Vec::new();
    };
    // The script's own `. script` line wraps the rest one level deeper.
    if all.len() > 1 && all[0].2.starts_with(". ") {
        all.remove(0);
    }
    let top = all.iter().map(|(d, _, _)| *d).min().unwrap_or(1);
    let starts: Vec<(f64, String)> = all
        .into_iter()
        .filter(|(d, _, _)| *d == top)
        .map(|(_, at, command)| (at, command))
        .collect();
    let end = first + total_sec;
    let mut phases = Vec::new();
    for (i, (at, command)) in starts.iter().enumerate() {
        let next = starts.get(i + 1).map_or(end, |(t, _)| *t);
        phases.push((command.clone(), (next - at).max(0.0)));
    }
    phases
}

/// The lines of a `/usr/bin/time -v` or `perf stat -x,` report worth
/// showing.
fn process_rows(report: &str) -> Vec<String> {
    report
        .lines()
        .map(str::trim)
        .filter(|l| {
            [
                "Elapsed (wall clock)",
                "User time",
                "System time",
                "Percent of CPU",
                "Maximum resident set size",
                "Voluntary context switches",
                "task-clock",
                "context-switches",
                "cpu-migrations",
                "page-faults",
            ]
            .iter()
            .any(|k| l.contains(k))
        })
        .map(str::to_string)
        .collect()
}

/// The evidence text for one profiled run of `command`, bounded by
/// `bound_sec`.
#[must_use]
pub fn report(profiler: Profiler, command: &str, run: &Profiled, bound_sec: u64) -> String {
    let seconds = run.milliseconds as f64 / 1000.0;
    let ended = if run.timed_out {
        format!("was stopped at the {bound_sec} s bound")
    } else {
        match run.exit {
            Some(0) => format!("finished in {seconds:.1} s"),
            Some(code) => format!("exited {code} after {seconds:.1} s"),
            None => format!("ended by a signal after {seconds:.1} s"),
        }
    };
    let mut text = format!(
        "The host ran `{}` once more under a profiler; it {ended}.\n",
        crate::judge::clip(command, 160)
    );
    let tail = after_mark(&run.stderr);
    match profiler {
        Profiler::Python => {
            let rows = tail.map(python_rows).unwrap_or_default();
            if rows.is_empty() {
                // A stack faulthandler dumped at the bound, when it did.
                let stack: Vec<&str> = run
                    .stderr
                    .lines()
                    .filter(|l| l.trim_start().starts_with("File \""))
                    .take(8)
                    .collect();
                if stack.is_empty() {
                    text.push_str("No profile was written.\n");
                } else {
                    text.push_str(
                        "No profile was written; the stack when the bound passed, innermost \
                         first:\n",
                    );
                    for line in stack {
                        text.push_str(&format!("  {}\n", line.trim()));
                    }
                }
            } else {
                text.push_str("Functions by cumulative time (cProfile):\n");
                for row in rows {
                    text.push_str(&format!("  {row}\n"));
                }
            }
        }
        Profiler::Shell => {
            let mut phases = tail.map(|t| shell_phases(t, seconds)).unwrap_or_default();
            if phases.is_empty() {
                text.push_str("The trace had no timestamps.\n");
            } else {
                let total: f64 = phases.iter().map(|(_, s)| s).sum::<f64>().max(1e-9);
                let count = phases.len();
                phases.sort_by(|a, b| b.1.total_cmp(&a.1));
                text.push_str(&format!(
                    "Top-level commands by wall time ({count} traced):\n"
                ));
                for (command, s) in phases.iter().take(8) {
                    text.push_str(&format!(
                        "  {s:>8.2} s  {:>3.0}%  {}\n",
                        100.0 * s / total,
                        crate::judge::clip(command, 140)
                    ));
                }
            }
        }
        Profiler::Process => {
            let rows = tail.map(process_rows).unwrap_or_default();
            if rows.is_empty() {
                text.push_str(
                    "The host has neither `perf` nor `/usr/bin/time`, so only the wall time is \
                     known.\n",
                );
            } else {
                for row in rows {
                    text.push_str(&format!("  {row}\n"));
                }
            }
        }
    }
    text.trim_end().to_string()
}
