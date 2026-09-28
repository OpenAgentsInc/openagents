//! `coderos-camera`: one owner of the camera on CoderOS.
//!
//! `coderos-camera serve` opens the camera once and fans every frame out
//! to a `v4l2loopback` node for the programs that read a camera node, to
//! a recording through `ffmpeg`, and to the hand tracker, which publishes
//! one line of landmarks a frame on a socket. Every other word is a verb
//! the command sends to the running daemon over its control socket:
//! `status`, `outputs`, `record start [<path>]`, `record stop`, `hands on`,
//! and `hands off`. `--json` prints the daemon's answer as it came.

mod capture;
mod config;
mod control;
mod fanout;
mod frame;
mod hands;
mod loopback;
mod paths;
mod protocol;
mod publisher;
mod record;
mod serve;

use protocol::{Reply, USAGE, parse_words, render};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut json = false;
    let mut words = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => words.push(arg),
        }
    }
    if words.first().map(String::as_str) == Some("serve") {
        return match serve::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("coderos-camera: {err}");
                ExitCode::from(1)
            }
        };
    }
    let verb = match parse_words(&words) {
        Ok(verb) => verb,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(2);
        }
    };
    let socket = serve::control_path();
    match control::ask(&socket, verb) {
        Ok(reply) => {
            if json {
                match serde_json::to_string(&protocol::Answer::new(reply.clone())) {
                    Ok(text) => println!("{text}"),
                    Err(err) => eprintln!("coderos-camera: {err}"),
                }
            } else {
                println!("{}", render(&reply));
            }
            match reply {
                Reply::Refused(_) | Reply::Unknown => ExitCode::from(1),
                _ => ExitCode::SUCCESS,
            }
        }
        Err(err) => {
            eprintln!("coderos-camera: {err}");
            ExitCode::from(1)
        }
    }
}
