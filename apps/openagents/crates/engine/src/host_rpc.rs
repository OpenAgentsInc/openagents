//! Adapter from admitted host requests to the existing app RPC service.
use coder_access::{
    Code,
    app_rpc::{Answer, Handler, Request},
};
use openagents_rpc::{RpcError, RpcReply, RpcService};
use std::{sync::Arc, time::Duration};

/// A running app host. Dropping it closes admission before asynchronous cleanup.
pub struct Hosted {
    host: Option<coder_host::Running>,
    runtime: tokio::runtime::Handle,
}
impl Hosted {
    pub async fn start(
        service: Arc<dyn RpcService>,
        config: coder_host::Config,
    ) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Handle::current();
        let handler = AppRpc::new(service, runtime.clone())
            .map_err(|_| anyhow::anyhow!("This runtime can't serve the device connection."))?;
        let host = coder_host::start(config, Arc::new(coder_host::NoTasks)).await?;
        host.set_app_rpc(Some(Arc::new(handler)));
        host.publish_reach().await;
        Ok(Self {
            host: Some(host),
            runtime,
        })
    }
    pub fn host(&self) -> &coder_host::Running {
        self.host.as_ref().unwrap()
    }
}
impl Drop for Hosted {
    fn drop(&mut self) {
        if let Some(host) = self.host.take() {
            host.set_app_rpc(None);
            host.stop_taking_requests();
            self.runtime.spawn(host.shutdown());
        }
    }
}

struct ProfileKeys {
    keys: Arc<dyn coder_host::serve::keys::AccountKeys>,
    scope: String,
}
impl ProfileKeys {
    fn name(&self, key: coder_host::serve::keys::KeyName) -> String {
        format!("app-device:{}:{}", self.scope, key.as_str())
    }
}
impl coder_host::serve::keys::KeySource for ProfileKeys {
    fn load(
        &self,
        key: coder_host::serve::keys::KeyName,
    ) -> openagents_connect::Result<Option<coder_host::serve::keys::Secret>> {
        self.keys.load_account(&self.name(key))
    }
    fn store(
        &self,
        key: coder_host::serve::keys::KeyName,
        secret: &coder_host::serve::keys::Secret,
    ) -> openagents_connect::Result<()> {
        self.keys.store_account(&self.name(key), secret)
    }
    fn delete(&self, key: coder_host::serve::keys::KeyName) -> openagents_connect::Result<()> {
        self.keys.delete_account(&self.name(key))
    }
}

pub(crate) fn configuration(
    root: &std::path::Path,
    label: String,
) -> anyhow::Result<coder_host::Config> {
    #[cfg(test)]
    anyhow::bail!("Tests must supply an isolated key source and host configuration.");
    #[cfg(not(test))]
    {
        use sha2::{Digest, Sha256};
        let generation = coder_service::generation::advance(root)?;
        let relay = openagents_brand::env_var("OPENAGENTS_NIP_HOST_RELAY")
            .unwrap_or_else(|_| "wss://relay.openagents.com".into());
        let mut config = coder_host::Config::new(root.join("access"), vec![relay], generation);
        config.label = label;
        config.telemetry = false;
        config.advertise_listener = false;
        config.keys = Some(coder_host::serve::keys::Keys(Arc::new(ProfileKeys {
            keys: coder_host::serve::keys::platform_accounts()?,
            scope: format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes())),
        })));
        Ok(config)
    }
}

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
