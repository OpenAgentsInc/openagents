//! Finite relay serving over one admitted relay. A resident host service
//! composes this; the loop itself never widens authority or retries effects.
use super::*;
use coder_connect::transport::Receiver;
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};

/// Handle requests until `done` returns true after a handled request, or until
/// the deadline passes. Returns the number of signed replies published.
/// Requests that earn no signed reply are skipped; the loop reconnects with
/// bounded backoff after a relay failure.
pub async fn serve(
    host: &Host,
    relay: &str,
    dispatch: &mut dyn Dispatch,
    deadline: Duration,
    mut done: impl FnMut(&Result<Event>) -> bool + Send,
) -> Result<usize> {
    let secret = host.key()?;
    let end = Instant::now() + deadline;
    let mut published = 0;
    loop {
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(published);
        }
        let mut receiver =
            match timeout(remaining, Receiver::connect(relay, &secret, host.policy())).await {
                Ok(Ok(receiver)) => receiver,
                Ok(Err(_)) | Err(_) => {
                    sleep(Duration::from_secs(1).min(remaining)).await;
                    continue;
                }
            };
        loop {
            let remaining = end.saturating_duration_since(Instant::now());
            let event = match timeout(remaining, receiver.next_request()).await {
                Err(_) => return Ok(published),
                Ok(Err(_)) => break,
                Ok(Ok(event)) => event,
            };
            let result = host.handle_current(&event, relay, dispatch);
            if let Ok(reply) = &result {
                if receiver.publish(reply).await.is_err() {
                    break;
                }
                published += 1;
            }
            if done(&result) {
                return Ok(published);
            }
        }
    }
}

impl Host {
    /// Publish host-signed private artifacts, such as enrollment requests.
    /// Each carries its own recipient; publication grants nothing.
    pub async fn publish(&self, relay: &str, events: &[Event]) -> Result<()> {
        self.policy.validate(relay).map_err(Error::from)?;
        let secret = self.key()?;
        let host = pubkey(&secret);
        for event in events {
            if event.pubkey != host {
                return fail(Code::Forbidden, "only host-signed artifacts are published");
            }
            nostr_transport::artifacts::publish(relay, &secret, event)
                .await
                .map_err(|_| Error::new(Code::Transport, "host artifact was not published"))?;
        }
        Ok(())
    }
}

/// Answer exactly one request with a signed reply, then return that reply.
pub async fn serve_once(
    host: &Host,
    relay: &str,
    dispatch: &mut dyn Dispatch,
    deadline: Duration,
) -> Result<Event> {
    let mut answered = None;
    serve(host, relay, dispatch, deadline, |result| {
        if let Ok(reply) = result {
            answered = Some(reply.clone());
        }
        result.is_ok()
    })
    .await?;
    answered.ok_or_else(|| {
        Error::new(
            Code::Transport,
            "no request was answered before the deadline",
        )
    })
}
