use crate::{RpcError, RpcReply, RpcService, memory_client};
use openagents_sync::device_channel::{
    Answer, CAPABILITY, DeviceChannel, HostError, HostLink, Request,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Host;
#[async_trait::async_trait]
impl HostLink for Host {
    async fn app_call(&self, request: Request) -> Result<Answer, HostError> {
        if request.method.reads_only() {
            Ok(Answer {
                method: request.method,
                value: json!("host"),
            })
        } else {
            Err(HostError::Config("fixture transport failure".into()))
        }
    }
}
struct WebSocket(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl RpcService for WebSocket {
    async fn handle(&self, _: &str, _: Value) -> Result<RpcReply, RpcError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        RpcReply::value(&json!("websocket"))
    }
}
#[tokio::test]
async fn opted_in_unary_calls_use_host_but_uncertain_commands_never_use_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = memory_client(Arc::new(WebSocket(calls.clone())));
    client.set_device_channel(Some(Arc::new(DeviceChannel::new(
        Arc::new(Host),
        true,
        &[CAPABILITY.into()],
    ))));
    assert_eq!(
        client.call("ListModels", json!({})).await.unwrap(),
        json!("host")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        client.call("WatchDocMessages", json!({})).await.unwrap(),
        json!("websocket")
    );
    let error = client
        .call(
            "RelayCommand",
            json!({"chatId":"chat","entry":{"id":"original-command"}}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hasn't confirmed"));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    client.set_device_channel(None);
    assert_eq!(
        client.call("ListModels", json!({})).await.unwrap(),
        json!("websocket")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}
