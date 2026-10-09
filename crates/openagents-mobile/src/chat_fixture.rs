//! An offline chat worker for simulator screenshots: it answers each turn
//! with the chat router's wire fields (a prepared answer with follow-ups,
//! then a dispatch offer, a read-only command, and a screen offer) so the
//! phone's router controls can be seen without a worker that sends them.
//!
//! Honored only in debug builds (`Launch::chat_fixture`). It reaches no
//! network, and it chooses its answer by the turn's position in the
//! conversation, never by reading the message.

use crate::basic_coder::{Door, Reply, Role, Turn, lock};
use crate::router::Context;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct ChatFixture;

/// The reply to the conversation's `n`th user message, from zero, and the
/// router's fields beside it.
fn script(n: usize, computer_ready: bool) -> (&'static str, Vec<Value>) {
    match n % 4 {
        0 => (
            "We are OpenAgents. In this chat we answer questions, explain things, and help you \
             plan and write. When something needs a computer, like reading or changing a \
             repository or running commands, we send Coder, our coding agent, to a \
             computer you've connected.",
            vec![
                json!({"v": 2, "type": "judgment", "verdict": "respond", "set": "chat-router-v1",
                    "route": "meta", "tier": "canned", "answer": "meta.who@1", "answer_p": 0.94,
                    "needs_specifics": 0.05, "lane": "chat"}),
                json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1",
                    "tier": "canned", "answer": "meta.who@1", "route": "meta",
                    "bank": "chat-answers-v1@fixture",
                    "followups": [
                        {"id": "meta.model", "label": "What model powers this chat?"},
                        {"id": "meta.coder", "label": "What can Coder do?"},
                        {"id": "meta.privacy", "label": "Is this chat private?"}]}),
            ],
        ),
        1 => (
            if computer_ready {
                "Working on finding the flaky test in crates/coder and fixing it."
            } else {
                "That needs a computer. Connect one first."
            },
            vec![
                json!({"v": 2, "type": "judgment", "verdict": "respond", "set": "chat-router-v1",
                    "route": "work.dispatch", "tier": "offer", "lane": "computer"}),
                json!({"v": 2, "type": "offer", "offer": "run_coder",
                    "target": "connected_computer", "label": "Run Coder"}),
                json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1",
                    "tier": "offer", "route": "work.dispatch"}),
            ],
        ),
        2 => (
            "We can check that from here, on this phone.",
            vec![
                json!({"v": 2, "type": "judgment", "verdict": "respond", "set": "chat-router-v1",
                    "route": "cli", "tier": "offer", "lane": "chat"}),
                json!({"v": 2, "type": "offer", "offer": "cli", "argv": ["computer", "list"],
                    "effect": "read_only", "runs_on": "this_device", "confirm": true}),
                json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1",
                    "tier": "offer", "route": "cli"}),
            ],
        ),
        _ => (
            "We'll never ask for your recovery words, and no one from OpenAgents will. Keep \
             them offline; anyone who has them has your bitcoin.",
            vec![
                json!({"v": 2, "type": "judgment", "verdict": "respond", "set": "chat-router-v1",
                    "route": "wallet", "tier": "canned", "answer": "wallet.never_share@1",
                    "answer_p": 0.91, "needs_specifics": 0.1, "lane": "chat"}),
                json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "wallet",
                    "label": "Open Wallet"}),
                json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1",
                    "tier": "canned", "answer": "wallet.never_share@1", "route": "wallet",
                    "bank": "chat-answers-v1@fixture",
                    "followups": [{"id": "wallet.backup", "label": "How do I back up my wallet?"}]}),
            ],
        ),
    }
}

impl Door for ChatFixture {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let asked = turns.iter().filter(|turn| turn.role == Role::User).count();
        let (text, fields) = script(asked.saturating_sub(1), context.computer_ready);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let mut reply = lock(&reply);
            for field in &fields {
                match field["type"].as_str() {
                    Some("judgment") => reply.meta.judged(field),
                    Some("offer") => reply.meta.offered(field),
                    Some("result") => reply.meta.resulted(field),
                    _ => {}
                }
            }
            reply.model = Some("bank:chat-answers-v1".into());
            reply.text = text.into();
            reply.done = true;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scripted_turn_reads_as_router_fields() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for n in 1..=4 {
            let reply = Arc::new(Mutex::new(Reply::default()));
            let turns = vec![Turn::user("hi"); n];
            runtime.block_on(ChatFixture.ask(turns, Context::default(), reply.clone()));
            let reply = lock(&reply).clone();
            assert!(reply.done && !reply.text.is_empty());
            let meta = reply.meta;
            assert!(meta.canned() || !meta.offers.is_empty(), "{n}: {meta:?}");
        }
    }
}
