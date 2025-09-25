use std::env;
use std::process;

use openagents_lib::headless::{plan_with_fallback, run_once_headless};
use openagents_lib::{tasks_list, task_create, task_get, AutonomyBudget, next_pending_index};

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  master_headless list");
    eprintln!("  master_headless create <name> [read-only]");
    eprintln!("  master_headless plan <task_id> <goal>");
    eprintln!("  master_headless run-once <task_id>");
    eprintln!("  master_headless run-until-done <task_id> [max_steps]");
    eprintln!("  master_headless show <task_id>");
}

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() { print_usage(); process::exit(1); }
    let cmd = args.remove(0);

    match cmd.as_str() {
        "list" => {
            match tasks_list() {
                Ok(list) => {
                    for m in list { println!("{}\t{}\t{:?}", m.id, m.name, m.status); }
                }
                Err(e) => { eprintln!("error: {e}"); process::exit(2); }
            }
        }
        "create" => {
            if args.is_empty() { print_usage(); process::exit(1); }
            let name = args.remove(0);
            let readonly = args.get(0).map(|s| s == "read-only").unwrap_or(false);
            let sandbox = if readonly { "read-only" } else { "danger-full-access" };
            match task_create(&name, AutonomyBudget { approvals: "never".into(), sandbox: sandbox.into(), max_turns: Some(2), max_tokens: Some(10_000), max_minutes: Some(5) }) {
                Ok(t) => { println!("created: {}", t.id); }
                Err(e) => { eprintln!("error: {e}"); process::exit(2); }
            }
        }
        "plan" => {
            if args.len() < 2 { print_usage(); process::exit(1); }
            let id = args.remove(0);
            let goal = args.join(" ");
            match plan_with_fallback(&id, &goal) {
                Ok(t) => { println!("planned: {} subtasks", t.queue.len()); }
                Err(e) => { eprintln!("error: {e}"); process::exit(2); }
            }
        }
        "run-once" => {
            if args.len() < 1 { print_usage(); process::exit(1); }
            let id = args.remove(0);
            match run_once_headless(&id) {
                Ok(t) => { println!("status: {:?}; next: {:?}", t.status, next_pending_index(&t)); }
                Err(e) => { eprintln!("error: {e}"); process::exit(2); }
            }
        }
        "run-until-done" => {
            if args.len() < 1 { print_usage(); process::exit(1); }
            let id = args.remove(0);
            let max_steps: usize = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(25);
            let mut count = 0usize;
            loop {
                match run_once_headless(&id) {
                    Ok(t) => {
                        println!("turn {}; status: {:?}; next: {:?}", count + 1, t.status, next_pending_index(&t));
                        count += 1;
                        if next_pending_index(&t).is_none() || count >= max_steps { break; }
                    }
                    Err(e) => { eprintln!("error: {e}"); process::exit(2); }
                }
            }
        }
        "show" => {
            if args.len() < 1 { print_usage(); process::exit(1); }
            let id = args.remove(0);
            match task_get(&id) {
                Ok(t) => { println!("name: {}\nstatus: {:?}\nqueue_len: {}\nmetrics: turns={}, tokens_in={}, tokens_out={}, minutes={}, retries={}", t.name, t.status, t.queue.len(), t.metrics.turns, t.metrics.tokens_in, t.metrics.tokens_out, t.metrics.wall_clock_minutes, t.metrics.retries); }
                Err(e) => { eprintln!("error: {e}"); process::exit(2); }
            }
        }
        _ => { print_usage(); process::exit(1); }
    }
}
