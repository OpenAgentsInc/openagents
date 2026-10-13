//! `openagents chat send MESSAGE --project PROJECT` (#11542): start a new
//! chat inside one of the person's projects on openagents.com and stream
//! its reply here.
//!
//! The chat is the account's, not this computer's: it starts through the
//! same server path the website's composer takes when a chat starts in a
//! project (`POST /v1/threads {request_id, text, project}`), so it is
//! answered by OpenAgents on openagents.com, reads the project's
//! repository, and shows in the project in the website's left panel, in
//! `openagents projects threads PROJECT`, and on the phone. PROJECT is a
//! name, repository, or id, found as `openagents projects show` finds it.
//! The reply streams by reading the chat until OpenAgents has answered.

use std::io::Write;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Failure, event, failed, message};
use crate::keys::Site;
use crate::{Args, EXIT_FAILURE, Output};

/// How often the chat is read while OpenAgents answers.
const POLL: Duration = Duration::from_millis(700);

/// The options and switches that pick a thread on this computer, which a
/// chat started in a project doesn't have.
const LOCAL_ONLY: &[&str] = &["thread", "scratch", "local", "socket", "no-run", "run-coder"];

/// `chat send --project`.
pub(super) fn send(output: &Output, args: &Args, project: &str) -> Result<u8, Failure> {
    if let Some(name) = LOCAL_ONLY
        .iter()
        .find(|name| args.option(name).is_some() || args.switch(name))
    {
        return Err(Failure::Refused(format!(
            "--project starts a new chat in your account on openagents.com, so it doesn't take --{name}"
        )));
    }
    let text = message(args.positional())?;
    let timeout = match args.number("timeout", 120) {
        Ok(timeout) if timeout > 0 => Duration::from_secs(timeout),
        _ => {
            return Err(Failure::Usage(
                "--timeout is a positive number of seconds".into(),
            ));
        }
    };
    let site = Site::open(None).map_err(failed)?;
    let found = crate::projects::named(&site, project).map_err(Failure::Refused)?;
    let project_id = found["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failed("The website sent a project without an id."))?
        .to_owned();
    let name = found["name"].as_str().unwrap_or(project).to_owned();
    let id = uuid::Uuid::new_v4().to_string();
    Site::answer(
        site.http
            .post(site.url("/v1/threads"))
            .bearer_auth(site.saved.token())
            .json(&body(&id, &text, &project_id))
            .send(),
    )
    .map_err(|message| {
        if message.contains("{request_id (a UUID), text}") {
            failed("openagents.com doesn't start chats in a project from here yet; start it on the website for now")
        } else {
            failed(message)
        }
    })?;
    event(
        output,
        json!({
            "event": "accepted",
            "thread": id,
            "request": id,
            "new": true,
            "backend": "account",
            "project": {"id": project_id, "name": name},
        }),
    );
    let ended = follow(output, &site, &id, timeout);
    eprintln!("thread {id} in project {name}");
    ended
}

/// The start request: the request id names the chat.
fn body(id: &str, text: &str, project: &str) -> Value {
    json!({"request_id": id, "text": text, "project": project})
}

/// Read the chat until OpenAgents has answered, printing its reply as it
/// grows.
fn follow(output: &Output, site: &Site, id: &str, timeout: Duration) -> Result<u8, Failure> {
    let started = Instant::now();
    let mut printed = String::new();
    loop {
        let thread = Site::answer(
            site.http
                .get(site.url(&format!("/v1/threads/{id}")))
                .bearer_auth(site.saved.token())
                .send(),
        )
        .map_err(failed)?;
        let reply = reply(&thread);
        if let Some(more) = reply.strip_prefix(printed.as_str()).filter(|more| !more.is_empty()) {
            event(
                output,
                json!({"event": "partial", "thread": id, "text": reply, "delta": more}),
            );
            if !output.json() {
                print!("{more}");
                let _ = std::io::stdout().flush();
            }
            printed = reply.clone();
        }
        if thread["thread"]["working"] != true {
            if reply.is_empty() {
                let message = "OpenAgents didn't answer; open the chat on openagents.com to try again";
                event(
                    output,
                    json!({"event": "failure", "thread": id, "message": message, "stopped": false}),
                );
                eprintln!("openagents chat: {message}");
                return Ok(EXIT_FAILURE);
            }
            event(
                output,
                json!({"event": "result", "thread": id, "text": reply}),
            );
            if !output.json() {
                println!();
            }
            return Ok(0);
        }
        if started.elapsed() >= timeout {
            let message = "stopped waiting for the reply; OpenAgents may still finish it";
            if !output.json() && !printed.is_empty() {
                println!();
            }
            event(
                output,
                json!({"event": "failure", "thread": id, "message": message, "stopped": true, "partial": printed}),
            );
            eprintln!("openagents chat: {message}");
            return Ok(EXIT_FAILURE);
        }
        std::thread::sleep(POLL);
    }
}

/// The reply so far: the chat's last message when it is OpenAgents's,
/// after the first message.
fn reply(thread: &Value) -> String {
    let messages = thread["messages"].as_array().cloned().unwrap_or_default();
    match messages.as_slice() {
        [_, .., last] if last["role"] == "assistant" => {
            last["text"].as_str().unwrap_or_default().to_owned()
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_names_the_chat_and_its_project() {
        assert_eq!(
            body("5f0c", "Testing. Hello.", "prj_0123456789abcdef"),
            json!({"request_id": "5f0c", "text": "Testing. Hello.", "project": "prj_0123456789abcdef"})
        );
    }

    #[test]
    fn the_reply_is_the_last_message_when_it_is_openagents() {
        let asked = json!({"thread": {"working": true}, "messages": [{"role": "user", "text": "Hi"}]});
        assert_eq!(reply(&asked), "");
        let answering = json!({"messages": [
            {"role": "user", "text": "Hi"},
            {"role": "assistant", "text": "Hello"},
        ]});
        assert_eq!(reply(&answering), "Hello");
        assert_eq!(reply(&json!({})), "");
    }
}
