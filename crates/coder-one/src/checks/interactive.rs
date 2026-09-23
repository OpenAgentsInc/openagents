//! Interactive behavior: drive the submitted interface the way its user
//! would, and observe a scratch result.
//!
//! - **program**: through the interface, start a host-authored program
//!   that puts the terminal in raw mode, send it a token, and read the
//!   file it writes: the token reversed. A terminal that only runs
//!   one-shot commands never gives the program a terminal.
//! - **interrupt**: start a long foreground command, send control C, then
//!   a command that writes a marker. The shell must still run it.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use super::{Bounds, Context, Ineligible, Relation, Scenario, Verdict, base_name};
use crate::minitask::process;

/// The token the program scenario types: a host choice.
const TOKEN: &str = "q7Zk2x";

/// The marker the interrupt scenario writes: a host choice.
const MARKER: &str = "still-usable-4821";

/// Seconds between typing the foreground command and sending control C,
/// one round each: a host choice that spans an interrupt sent as the
/// command starts and one sent once it runs.
pub const DELAYS: [f64; 3] = [0.05, 0.3, 1.0];

/// A raw-mode program: it writes `<result>.ready` once the terminal is
/// raw, reads keys up to Enter, and writes them reversed to `<result>`.
const PROGRAM: &str = r#"import os, sys, termios, tty
fd = sys.stdin.fileno()
old = termios.tcgetattr(fd)
tty.setraw(fd)
try:
    open(sys.argv[1] + ".ready", "w").write("ready")
    os.write(1, b"token> ")
    data = b""
    while True:
        ch = os.read(fd, 1)
        if not ch or ch in (b"\r", b"\n"):
            break
        data += ch
    open(sys.argv[1], "w").write(data.decode(errors="replace")[::-1])
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
"#;

/// The driver: imports the candidate, constructs it, sends keys through
/// its method with each call bounded, and prints its observations as JSON.
const DRIVER: &str = r#"import json, os, signal, sys, threading, time
work, module, cls, method, mode, program, result, token = sys.argv[1:9]
DELAYS = [float(d) for d in sys.argv[9].split(",")]
sys.path.insert(0, work)
os.chdir(work)
obs = []
def done():
    print(json.dumps(obs)); sys.stdout.flush()
    try:
        os.killpg(os.getpgid(0), signal.SIGKILL)
    finally:
        os._exit(0)
def bounded(fn, *args, seconds=4.0):
    box = {}
    def call():
        try:
            box["value"] = fn(*args)
        except BaseException as error:
            box["error"] = type(error).__name__ + ": " + str(error)
    thread = threading.Thread(target=call, daemon=True)
    thread.start()
    thread.join(seconds)
    if thread.is_alive():
        return "blocked", None
    if "error" in box:
        return "error", box["error"]
    return "ok", box.get("value")
try:
    terminal_class = getattr(__import__(module), cls)
except BaseException as error:
    obs.append({"step": "import", "status": "error", "error": type(error).__name__, "detail": str(error)[:300]})
    done()
status, value = bounded(terminal_class)
obs.append({"step": "construct", "status": status, "detail": value if status == "error" else None})
if status != "ok":
    done()
terminal = value
send = getattr(terminal, method)
def keys(text, wait):
    status, value = bounded(send, text, wait)
    obs.append({"step": "send", "keys": text, "status": status, "detail": value if status == "error" else None})
    return status
def wait_file(path, seconds, want=None):
    end = time.time() + seconds
    while time.time() < end:
        if os.path.exists(path):
            text = open(path).read()
            if want is None or want in text:
                return text
        time.sleep(0.05)
    return open(path).read() if os.path.exists(path) else None
if mode == "program":
    keys("python3 " + program + " " + result + "\n", 0.5)
    ready = wait_file(result + ".ready", 3)
    obs.append({"step": "ready", "seen": ready is not None})
    keys(token, 0.2)
    keys("\n", 0.5)
    obs.append({"step": "result", "content": wait_file(result, 3)})
else:
    for n, delay in enumerate(DELAYS):
        if keys("sleep 30\n", delay) == "blocked":
            # A blocked terminal stays blocked; later rounds say nothing new.
            obs.append({"step": "result", "round": n, "delay": delay, "content": None})
            break
        keys("\x03", 0.3)
        target = result + "." + str(n)
        keys("echo " + token + " > " + target + "\n", 0.5)
        obs.append({"step": "result", "round": n, "delay": delay, "content": wait_file(target, 3, token)})
close = getattr(terminal, "close", None)
if close is not None:
    bounded(close, seconds=2)
done()
"#;

/// The interface a candidate implements, from the instruction and the
/// files.
struct Interface {
    module: String,
    class: String,
    method: String,
    file: String,
}

fn interface(context: &Context<'_>) -> Result<Interface, String> {
    let instruction = &context.task.instruction;
    let (module, class) = instruction
        .split('`')
        .skip(1)
        .step_by(2)
        .find_map(|span| {
            let words: Vec<&str> = span.split_whitespace().collect();
            match words.as_slice() {
                ["from", module, "import", class] => {
                    Some(((*module).to_string(), (*class).to_string()))
                }
                _ => None,
            }
        })
        .ok_or("the instruction names no `from MODULE import CLASS` interface")?;
    let file = format!("{module}.py");
    let (path, source) = context
        .candidate
        .file_named(&file)
        .ok_or_else(|| format!("the candidate has no {file}"))?;
    let method = context
        .candidate
        .provided
        .values()
        .chain(std::iter::once(source))
        .find_map(|text| {
            text.lines().find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("def ")?;
                let (name, args) = rest.split_once('(')?;
                args.starts_with("self, keystrokes")
                    .then(|| name.to_string())
            })
        })
        .ok_or("no method of the interface takes keystrokes")?;
    // Whatever the candidate imports from the task must be there too.
    for line in source.lines() {
        if let Some(rest) = line.trim().strip_prefix("from ")
            && let Some((imported, _)) = rest.split_once(" import ")
            && imported.starts_with("base")
            && !context
                .candidate
                .provided
                .contains_key(&format!("{imported}.py"))
            && context
                .candidate
                .file_named(&format!("{imported}.py"))
                .is_none()
        {
            return Err(format!(
                "the candidate imports {imported}.py, which the task provided and the check doesn't have"
            ));
        }
    }
    Ok(Interface {
        module,
        class,
        method,
        file: path.clone(),
    })
}

/// Builds the interactive scenarios that apply.
///
/// # Errors
///
/// Returns why none applies.
pub fn build(context: &Context<'_>) -> Result<Vec<Scenario>, Vec<Ineligible>> {
    let not = |why: String| {
        Err(vec![Ineligible {
            kind: "interactive".to_string(),
            why,
        }])
    };
    if !context
        .task
        .instruction
        .to_lowercase()
        .contains("interactive")
    {
        return not("the instruction asks for no interactive behavior".to_string());
    }
    let found = match interface(context) {
        Ok(found) => found,
        Err(why) => return not(why),
    };
    let described = format!(
        "from {} import {}; {}().{}(keys, wait_sec)",
        found.module, found.class, found.class, found.method
    );
    let candidate = context.candidate.digest();
    let mut scenarios = Vec::new();
    let mut ineligible = Vec::new();
    let programs = context.requirements_saying(&[&["interactive", "program"]]);
    if programs.is_empty() {
        ineligible.push(Ineligible {
            kind: "interactive.program".to_string(),
            why: "no requirement asks for interactive programs".to_string(),
        });
    } else {
        scenarios.push(Scenario {
            id: "interactive.program".to_string(),
            kind: "interactive.program".to_string(),
            requirements: programs.iter().map(|r| r.id.clone()).collect(),
            spans: context.spans_of(&programs),
            applies: vec![
                format!("the instruction names the interface {described}"),
                format!("the candidate provides {}", found.file),
            ],
            interface: described.clone(),
            bounds: Bounds { seconds: 25, processes: 4 },
            effects: vec![
                "copies the candidate into a scratch directory and runs it there".to_string(),
                "starts the candidate's shell and a host-authored raw-mode program".to_string(),
            ],
            candidate: candidate.clone(),
            input: atif::digest(&json!({ "program": PROGRAM, "token": TOKEN })),
            seed: None,
            expected: Relation {
                statement: "A program started through the interface gets a terminal in raw mode, reads the staged keys up to Enter, and writes them reversed to a scratch file.".to_string(),
                derivation: "The instruction asks for support of interactive programs typed into an interactive shell; a raw-mode program reads only from a terminal, and the reversed token shows it read the keys.".to_string(),
            },
            params: json!({ "token": TOKEN, "enter": "\\n", "derived_from": "host choices; the interface from the instruction and the provided files" }),
        });
    }
    let interrupts =
        context.requirements_saying(&[&["control c"], &["x03"], &["modifier"], &["ctrl"]]);
    if interrupts.is_empty() {
        ineligible.push(Ineligible {
            kind: "interactive.interrupt".to_string(),
            why: "no requirement asks for control keys".to_string(),
        });
    } else {
        scenarios.push(Scenario {
            id: "interactive.interrupt".to_string(),
            kind: "interactive.interrupt".to_string(),
            requirements: interrupts.iter().map(|r| r.id.clone()).collect(),
            spans: context.spans_of(&interrupts),
            applies: vec![
                format!("the instruction names the interface {described}"),
                "a requirement names control C".to_string(),
            ],
            interface: described,
            bounds: Bounds { seconds: 40, processes: 4 },
            effects: vec![
                "copies the candidate into a scratch directory and runs it there".to_string(),
                "starts the candidate's shell and, once per round, a 30-second foreground command".to_string(),
            ],
            candidate,
            input: atif::digest(&json!({ "marker": MARKER })),
            seed: None,
            expected: Relation {
                statement: "After control C interrupts a foreground command, the shell runs the next command typed.".to_string(),
                derivation: "The instruction asks for control C in an interactive shell; interrupting a foreground command must leave the shell usable.".to_string(),
            },
            params: json!({ "marker": MARKER, "foreground": "sleep 30", "delays": DELAYS, "derived_from": "host choices" }),
        });
    }
    if scenarios.is_empty() {
        Err(ineligible)
    } else {
        Ok(scenarios)
    }
}

/// Runs an interactive scenario.
pub async fn run(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    let Some(python) = process::python() else {
        return Verdict::unavailable(&scenario.id, "python3 is not on PATH");
    };
    let found = match interface(context) {
        Ok(found) => found,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let work = scratch.join("work");
    if std::fs::create_dir_all(&work).is_err() {
        return Verdict::unavailable(&scenario.id, "cannot create the scratch copy");
    }
    for (path, text) in context.candidate.provided.iter().chain(
        context
            .candidate
            .files
            .iter()
            .filter(|(p, _)| p.ends_with(".py")),
    ) {
        let _ = std::fs::write(work.join(base_name(path)), text);
    }
    let program = scratch.join("interact.py");
    let _ = std::fs::write(&program, PROGRAM);
    let result = scratch.join("result.txt");
    let program_mode = scenario.kind == "interactive.program";
    let mut command = Command::new(python);
    command
        .arg("-c")
        .arg(DRIVER)
        .arg(&work)
        .arg(&found.module)
        .arg(&found.class)
        .arg(&found.method)
        .arg(if program_mode { "program" } else { "interrupt" })
        .arg(&program)
        .arg(&result)
        .arg(if program_mode { TOKEN } else { MARKER })
        .arg(
            DELAYS
                .iter()
                .map(f64::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
        .current_dir(&work)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("HOME", &work);
    let ran = process::run(command, Duration::from_secs(scenario.bounds.seconds)).await;
    let observations: Vec<Value> = ran
        .stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<Vec<Value>>(line).ok())
        .unwrap_or_default();
    let mut verdict = Verdict::new(&scenario.id, "passed");
    verdict.observations = observations.clone();
    if observations.is_empty() {
        verdict.verdict = "inconclusive".to_string();
        verdict.coverage.push(format!(
            "The driver reported nothing{}: {}",
            if ran.killed {
                " before its deadline"
            } else {
                ""
            },
            crate::judge::clip(ran.stderr.trim(), 300)
        ));
        return verdict;
    }
    let step = |name: &'static str| observations.iter().filter(move |o| o["step"] == name);
    if let Some(import) = step("import").next() {
        if import["error"] == "ModuleNotFoundError" {
            verdict.verdict = "unavailable".to_string();
            verdict.coverage.push(format!(
                "The candidate needs a module this host lacks: {}",
                import["detail"].as_str().unwrap_or_default()
            ));
        } else {
            verdict.verdict = "failed".to_string();
            verdict
                .hypotheses
                .push("The submitted module doesn't import.".to_string());
        }
        return verdict;
    }
    if step("construct")
        .next()
        .is_some_and(|o| o["status"] != "ok")
    {
        verdict.verdict = "failed".to_string();
        verdict
            .hypotheses
            .push("The interface can't be constructed, or construction blocks.".to_string());
        return verdict;
    }
    let blocked = step("send").any(|o| o["status"] == "blocked");
    let errored = step("send").any(|o| o["status"] == "error");
    let content = step("result")
        .next()
        .and_then(|o| o["content"].as_str().map(str::to_string));
    let reversed: String = TOKEN.chars().rev().collect();
    if program_mode {
        let ready = step("ready").next().is_some_and(|o| o["seen"] == true);
        if content.as_deref() != Some(reversed.as_str()) {
            verdict.verdict = "failed".to_string();
            if !ready {
                verdict.hypotheses.push("The program never got a terminal: typed commands run without a pseudo-terminal, so raw mode fails.".to_string());
            } else {
                verdict.hypotheses.push("The program started, but the staged keys didn't reach it: input timing, buffering, or line discipline.".to_string());
            }
            if blocked {
                verdict
                    .hypotheses
                    .push("Sending keys blocks while the program runs.".to_string());
            }
            if errored {
                verdict
                    .hypotheses
                    .push("Sending keys raised an error.".to_string());
            }
        }
        verdict.coverage.push("One raw-mode program and one token; full-screen programs that query the terminal are untested.".to_string());
    } else {
        let lost: Vec<f64> = step("result")
            .filter(|o| o["content"].as_str().map(str::trim) != Some(MARKER))
            .filter_map(|o| o["delay"].as_f64())
            .collect();
        if blocked || !lost.is_empty() {
            verdict.verdict = "failed".to_string();
            if blocked {
                verdict.hypotheses.push("Sending keys blocks while a foreground command runs, so control C never arrives.".to_string());
            }
            if !lost.is_empty() && lost.len() < DELAYS.len() {
                verdict.hypotheses.push(format!(
                    "Control C was lost when sent {} s after the command was typed, but not at other delays: whether it reaches the command depends on timing, such as which process holds the terminal when it arrives.",
                    lost.iter().map(f64::to_string).collect::<Vec<_>>().join(" s and ")
                ));
            } else if !lost.is_empty() {
                verdict.hypotheses.push("Control C didn't interrupt the foreground command at any delay, or the shell exited on it.".to_string());
            }
        }
        verdict.coverage.push(format!(
            "One control key, sent {} s after the command; other modifiers and delays are untested.",
            DELAYS.iter().map(f64::to_string).collect::<Vec<_>>().join(", ")
        ));
    }
    verdict
}
