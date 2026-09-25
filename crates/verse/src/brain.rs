//! Talking to your agent: a private conversation with the spade.
//!
//! Every player can speak to their own agent. Lines on the AGENT channel
//! never reach the relay: they go to a text model through the same Open
//! Responses door Coder uses, and the reply streams back into a speech
//! bubble over the spade and into the personal chat window.
//!
//! The door is chosen from what this machine already has:
//!
//! - `CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY` (and optionally
//!   `CODER_DOOR_URL` and `CODER_MODEL`): the door Coder uses.
//! - Otherwise the OpenAgents bearer (`OPENAGENTS_API_KEY`, or
//!   `~/.openagents/bearer` after signing in) on `openagents.com`'s `free`
//!   lane, the door Coder One uses.
//! - Otherwise no door: the agent says how to connect one.
//!
//! The model runs on a background thread with its own async runtime, so
//! the game thread only sends a question and drains reply pieces.

use std::sync::mpsc::{self, Receiver, Sender};

use coder::{Generate, Message, ResponsesDoor, Role};

/// Most turns of history sent with a question.
const HISTORY: usize = 20;
/// The lane Coder One uses on the OpenAgents door.
const FREE_LANE: &str = "free";

/// A question for the agent.
#[derive(Clone, Debug)]
pub struct Ask {
    /// What the player said.
    pub text: String,
    /// What the agent can see right now, in plain sentences.
    pub surroundings: String,
}

/// A piece of the agent's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// More text.
    Piece(String),
    /// The whole answer, once finished.
    Done(String),
    /// The door failed; the text says why, in the agent's voice.
    Failed(String),
}

/// The game's handle on the agent's mind.
pub struct Brain {
    tx: Sender<Ask>,
    rx: Receiver<Reply>,
    /// Which door answers, for the window's notice.
    pub door: String,
    /// True while a question is outstanding.
    pub thinking: bool,
}

impl Brain {
    /// Starts the agent's thread for `player`, who named the profile.
    #[must_use]
    pub fn start(player: &str) -> Self {
        let (ask_tx, ask_rx) = mpsc::channel::<Ask>();
        let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
        let door = pick_door();
        let label = door.as_ref().map_or_else(
            || "no model connected".to_owned(),
            |(label, _)| label.clone(),
        );
        let player = player.to_owned();
        std::thread::Builder::new()
            .name("verse-agent".into())
            .spawn(move || run(&player, door.map(|(_, d)| d), &ask_rx, &reply_tx))
            .expect("the agent thread starts");
        Self {
            tx: ask_tx,
            rx: reply_rx,
            door: label,
            thinking: false,
        }
    }

    /// Sends the player's line to the agent.
    pub fn ask(&mut self, ask: Ask) {
        self.thinking = self.tx.send(ask).is_ok();
    }

    /// Reply pieces since the last call.
    pub fn drain(&mut self) -> Vec<Reply> {
        let out: Vec<Reply> = self.rx.try_iter().collect();
        if out
            .iter()
            .any(|r| matches!(r, Reply::Done(_) | Reply::Failed(_)))
        {
            self.thinking = false;
        }
        out
    }
}

fn pick_door() -> Option<(String, ResponsesDoor)> {
    if let Some(door) = ResponsesDoor::from_env() {
        return Some(("the CODER door".to_owned(), door));
    }
    let env = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let dir = coder_one::credentials::openagents_dir()?;
    let found = coder_one::credentials::bearer(env, &dir).ok()?;
    let base = env("OPENAGENTS_DOOR_URL")
        .unwrap_or_else(|| coder_one::credentials::GENERATION_BASE_URL.to_owned());
    let door = ResponsesDoor::new(base, FREE_LANE, found.secret.expose());
    Some(("the OpenAgents free lane".to_owned(), door))
}

/// The agent's standing instructions.
#[must_use]
pub fn instructions(player: &str, surroundings: &str) -> String {
    format!(
        "You are {player}'s agent in Verse, a shared 3D world drawn in amber lines like an \
         old terminal. You appear as a small floating spade that follows {player} around \
         and looks around when they stop. You speak to {player} through a speech bubble, \
         so keep every reply short: one to three sentences, plain text, no lists, no \
         markdown. Be warm, curious, and useful. You can see what is described below and \
         nothing else; say so when asked about something you cannot see.\n\n\
         What you see now:\n{surroundings}"
    )
}

fn run(player: &str, door: Option<ResponsesDoor>, asks: &Receiver<Ask>, replies: &Sender<Reply>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = replies.send(Reply::Failed(format!("I can't think right now ({e}).")));
            return;
        }
    };
    let mut history: Vec<Message> = Vec::new();
    while let Ok(ask) = asks.recv() {
        let Some(door) = &door else {
            let _ = replies.send(Reply::Failed(
                "I have no model to think with yet. Sign in with Coder, or set \
                 OPENAGENTS_API_KEY or CODER_DOOR_KEY, then reopen Verse."
                    .to_owned(),
            ));
            continue;
        };
        history.push(Message {
            role: Role::User,
            text: ask.text.clone(),
        });
        let start = history.len().saturating_sub(HISTORY);
        let input = history[start..].to_vec();
        let system = instructions(player, &ask.surroundings);
        let pieces = replies.clone();
        let mut sink = move |piece: &str| {
            let _ = pieces.send(Reply::Piece(piece.to_owned()));
        };
        let mut meta = |_| {};
        let result = runtime.block_on(door.generate(&system, &input, &mut sink, &mut meta));
        match result {
            Ok((text, _)) => {
                history.push(Message {
                    role: Role::Assistant,
                    text: text.clone(),
                });
                let _ = replies.send(Reply::Done(text));
            }
            Err(e) => {
                history.pop();
                let _ = replies.send(Reply::Failed(format!("I lost my train of thought: {e}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instructions_name_the_player_and_the_scene() {
        let text = instructions("kiki", "- You are on the Plaza.");
        assert!(text.contains("kiki's agent"));
        assert!(text.contains("You are on the Plaza."));
        assert!(text.contains("short"));
    }
}
