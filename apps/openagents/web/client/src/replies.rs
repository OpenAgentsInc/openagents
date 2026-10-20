//! Device-room RPC replies, including bounded ephemeral streams.
use anyhow::{Result, anyhow};
use futures::channel::{mpsc, oneshot};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) enum Reply {
    Value(oneshot::Sender<Result<Value>>),
    Stream(mpsc::Sender<Result<Value>>),
}

/// Returns a request to cancel when its consumer cannot keep up.
pub(crate) fn route(entries: &mut HashMap<u64, Reply>, frame: &Value) -> Option<u64> {
    let id = frame["id"].as_u64()?;
    if let Some(Reply::Stream(tx)) = entries.get_mut(&id) {
        if let Some(error) = frame["err"].as_str() {
            let _ = tx.try_send(Err(anyhow!("{error}")));
            entries.remove(&id);
        } else if frame["done"] == true {
            entries.remove(&id);
        } else if let Some(item) = frame.get("item") {
            if tx.try_send(Ok(item.clone())).is_err() {
                entries.remove(&id);
                return Some(id);
            }
        }
        return None;
    }
    let Some(Reply::Value(tx)) = entries.remove(&id) else {
        return None;
    };
    let answer = match frame.get("err").and_then(Value::as_str) {
        Some(error) => Err(anyhow!(
            "{}",
            error.strip_prefix("failed: ").unwrap_or(error)
        )),
        None => Ok(frame.get("ok").cloned().unwrap_or(Value::Null)),
    };
    let _ = tx.send(answer);
    None
}

pub(crate) fn fail_all(entries: &mut HashMap<u64, Reply>, message: &str) {
    for (_, tx) in entries.drain() {
        match tx {
            Reply::Value(tx) => {
                let _ = tx.send(Err(anyhow!("{message}")));
            }
            Reply::Stream(mut tx) => {
                let _ = tx.try_send(Err(anyhow!("{message}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, executor::block_on};
    use serde_json::json;

    #[test]
    fn captions_stream_until_done_alongside_unary_calls() {
        block_on(async {
            let (stream, mut events) = mpsc::channel(8);
            let (call, answer) = oneshot::channel();
            let mut pending = HashMap::from([(1, Reply::Stream(stream)), (2, Reply::Value(call))]);
            route(&mut pending, &json!({"id":1,"ok":{"stream":true}}));
            for item in [
                json!({"type":"snapshot"}),
                json!({"type":"partial","text":"Hello"}),
                json!({"type":"final","text":"Hello there"}),
            ] {
                assert_eq!(route(&mut pending, &json!({"id":1,"item":item})), None);
                assert_eq!(events.next().await.unwrap().unwrap(), item);
            }
            route(&mut pending, &json!({"id":2,"ok":{"sequence":4}}));
            assert_eq!(answer.await.unwrap().unwrap(), json!({"sequence":4}));
            route(&mut pending, &json!({"id":1,"done":true}));
            assert!(events.next().await.is_none());
            assert!(pending.is_empty());
        });
    }

    #[test]
    fn a_slow_caption_consumer_cancels_without_growing_memory() {
        let (stream, _events) = mpsc::channel(1);
        let mut pending = HashMap::from([(1, Reply::Stream(stream))]);
        let cancelled =
            (0..10).find_map(|_| route(&mut pending, &json!({"id":1,"item":"caption"})));
        assert_eq!(cancelled, Some(1));
        assert!(pending.is_empty());
    }

    #[test]
    fn refusal_and_disconnect_end_streams_and_calls() {
        block_on(async {
            let (stream, mut events) = mpsc::channel(2);
            let mut pending = HashMap::from([(1, Reply::Stream(stream))]);
            route(&mut pending, &json!({"id":1,"err":"access ended"}));
            assert!(
                events
                    .next()
                    .await
                    .unwrap()
                    .unwrap_err()
                    .to_string()
                    .contains("access ended")
            );
            assert!(events.next().await.is_none());
            let (stream, mut events) = mpsc::channel(2);
            let (call, answer) = oneshot::channel();
            pending.extend([(2, Reply::Stream(stream)), (3, Reply::Value(call))]);
            fail_all(&mut pending, "connection closed");
            assert!(events.next().await.unwrap().is_err());
            assert!(events.next().await.is_none());
            assert!(answer.await.unwrap().is_err());
            assert!(pending.is_empty());
        });
    }
}
