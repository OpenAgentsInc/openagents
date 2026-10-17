//! Adapter from admitted host requests to the existing app RPC service.
use coder_access::{
    Code,
    app_rpc::{Answer, Handler, Request},
};
use openagents_rpc::{RpcError, RpcReply, RpcService};
use std::{sync::Arc, time::Duration};

/// Install this on a Coder host with `Running::set_app_rpc`.
/// The host calls it on its blocking request worker after checking the grant.
pub struct AppRpc {
    service: Arc<dyn RpcService>,
    runtime: tokio::runtime::Handle,
}
impl AppRpc {
    pub fn new(
        service: Arc<dyn RpcService>,
        runtime: tokio::runtime::Handle,
    ) -> Result<Self, Code> {
        if runtime.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Err(Code::Unsupported);
        }
        Ok(Self { service, runtime })
    }
}
impl Handler for AppRpc {
    fn call(
        &self,
        _: &str,
        _: &str,
        _: Option<(&str, u64)>,
        request: &Request,
    ) -> Result<Answer, Code> {
        request.validate().map_err(|error| error.code)?;
        let service = self.service.clone();
        let request = request.clone();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let task = self.runtime.spawn(async move {
            let result = match service
                .handle(request.method.as_str(), request.params)
                .await
            {
                Ok(RpcReply::Value(value)) => {
                    let answer = Answer {
                        method: request.method,
                        value,
                    };
                    answer
                        .validate()
                        .map_err(|error| error.code)
                        .map(|()| answer)
                }
                Ok(RpcReply::Stream(_)) => Err(Code::Unavailable),
                Err(RpcError::UnknownMethod(_)) => Err(Code::Unsupported),
                Err(RpcError::BadParams(_)) => Err(Code::Malformed),
                Err(_) => Err(Code::Unavailable),
            };
            let _ = send.send(result);
        });
        match receive.recv_timeout(Duration::from_secs(8)) {
            Ok(result) => result,
            Err(_) => {
                task.abort();
                Err(Code::Unavailable)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::app_rpc::Method;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Service(Mutex<std::collections::BTreeSet<String>>);
    #[async_trait::async_trait]
    impl RpcService for Service {
        async fn handle(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<RpcReply, RpcError> {
            match method {
                openagents_rpc::methods::LIST_MODELS => RpcReply::value(&json!(["fixture-model"])),
                openagents_rpc::methods::RELAY_COMMAND => {
                    let id = params["entry"]["id"].as_str().unwrap().to_owned();
                    let fresh = self.0.lock().unwrap().insert(id);
                    RpcReply::value(&json!({"fresh": fresh}))
                }
                _ => Err(RpcError::UnknownMethod(method.into())),
            }
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_keeps_the_method_and_the_original_command_id() {
        let service = Arc::new(Service::default());
        let adapter = AppRpc::new(service.clone(), tokio::runtime::Handle::current()).unwrap();
        tokio::task::spawn_blocking(move || {
            let read = Request {
                method: Method::ListModels,
                params: json!({}),
            };
            assert_eq!(
                adapter
                    .call("request", "device", None, &read)
                    .unwrap()
                    .value,
                json!(["fixture-model"])
            );
            let command = Request {
                method: Method::RelayCommand,
                params: json!({"chatId":"chat", "entry":{"id":"original-command"}}),
            };
            assert_eq!(
                adapter
                    .call("first", "device", None, &command)
                    .unwrap()
                    .value,
                json!({"fresh":true})
            );
            assert_eq!(
                adapter
                    .call("retry", "device", None, &command)
                    .unwrap()
                    .value,
                json!({"fresh":false})
            );
        })
        .await
        .unwrap();
        assert_eq!(
            *service.0.lock().unwrap(),
            std::collections::BTreeSet::from(["original-command".into()])
        );
    }
}
