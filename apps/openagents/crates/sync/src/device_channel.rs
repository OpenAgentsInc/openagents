//! Opt-in unary app RPC on a route proved by the host connector.
pub use coder_access::{
    Code,
    app_rpc::{Answer, CAPABILITY, Request},
};
use coder_access::{
    app_rpc::Method,
    protocol::{Operation, Outcome},
};
pub use coder_host::Error as HostError;
use serde_json::Value;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait HostLink: Send + Sync {
    async fn app_call(&self, request: Request) -> Result<Answer, coder_host::Error>;
}
#[async_trait::async_trait]
impl HostLink for coder_host::client::Link {
    async fn app_call(&self, request: Request) -> Result<Answer, coder_host::Error> {
        match self.call(Operation::AppRpc { app: request }).await? {
            Outcome::AppRpc { app } => Ok(app),
            _ => Err(coder_host::Error::Access(coder_access::Error::new(
                Code::Malformed,
                "unexpected app reply",
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("This computer refused the request.")]
    Refused(Code),
    #[error("Couldn't reach this computer. Try again later.")]
    Unavailable,
    #[error("The computer hasn't confirmed this command. Check the session before trying again.")]
    Uncertain { command_id: String },
}

pub struct DeviceChannel {
    link: Arc<dyn HostLink>,
    enabled: bool,
}
impl DeviceChannel {
    /// `link` comes from `Connector::link` for this paired computer.
    /// Enable only after the person opts in and the host advertises `app-rpc-v1`.
    pub fn new(link: Arc<dyn HostLink>, opted_in: bool, capabilities: &[String]) -> Self {
        Self {
            link,
            enabled: opted_in
                && capabilities
                    .iter()
                    .any(|c| c == coder_access::app_rpc::CAPABILITY),
        }
    }
    /// `None` asks the caller to use its existing WebSocket transport.
    /// A refused or unanswered command never falls back automatically.
    pub async fn call(&self, method: &str, params: Value) -> Result<Option<Value>, Failure> {
        if !self.enabled {
            return Ok(None);
        }
        let Ok(method) = serde_json::from_value::<Method>(Value::String(method.into())) else {
            return Ok(None);
        };
        let request = Request { method, params };
        request.validate().map_err(|e| Failure::Refused(e.code))?;
        let result = self.link.app_call(request.clone()).await;
        match result {
            Ok(answer) if answer.answers(&request) && answer.validate().is_ok() => {
                Ok(Some(answer.value))
            }
            Err(coder_host::Error::Access(error)) if error.code == Code::Unsupported => Ok(None),
            Err(coder_host::Error::Access(error))
                if matches!(
                    error.code,
                    Code::MissingRight
                        | Code::Forbidden
                        | Code::Revoked
                        | Code::Expired
                        | Code::Denied
                        | Code::Malformed
                        | Code::Bounds
                ) =>
            {
                Err(Failure::Refused(error.code))
            }
            _ if method.reads_only() => Err(Failure::Unavailable),
            _ => Err(Failure::Uncertain {
                command_id: request.params["entry"]["id"].as_str().unwrap().into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Link {
        calls: AtomicUsize,
        code: Code,
    }
    #[async_trait::async_trait]
    impl HostLink for Link {
        async fn app_call(&self, _: Request) -> Result<Answer, coder_host::Error> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(coder_host::Error::Access(coder_access::Error::new(
                self.code, "fixture",
            )))
        }
    }
    fn channel(code: Code, opted_in: bool, capable: bool) -> (DeviceChannel, Arc<Link>) {
        let link = Arc::new(Link {
            calls: AtomicUsize::new(0),
            code,
        });
        let capabilities = if capable {
            vec![coder_access::app_rpc::CAPABILITY.into()]
        } else {
            vec![]
        };
        (
            DeviceChannel::new(link.clone(), opted_in, &capabilities),
            link,
        )
    }
    #[tokio::test]
    async fn old_hosts_disabled_devices_and_streams_stay_on_websocket() {
        for (opted_in, capable) in [(false, true), (true, false)] {
            let (channel, link) = channel(Code::Unsupported, opted_in, capable);
            assert_eq!(channel.call("ListModels", json!({})).await, Ok(None));
            assert_eq!(link.calls.load(Ordering::Relaxed), 0);
        }
        let (channel, link) = channel(Code::Unsupported, true, true);
        assert_eq!(channel.call("WatchDocMessages", json!({})).await, Ok(None));
        assert_eq!(link.calls.load(Ordering::Relaxed), 0);
    }
    #[tokio::test]
    async fn only_unsupported_allows_fallback_after_sending() {
        let params = json!({"chatId":"chat", "entry":{"id":"original-command"}});
        let (unsupported, _) = channel(Code::Unsupported, true, true);
        assert_eq!(
            unsupported.call("RelayCommand", params.clone()).await,
            Ok(None)
        );
        let (revoked, _) = channel(Code::Revoked, true, true);
        assert_eq!(
            revoked.call("RelayCommand", params.clone()).await,
            Err(Failure::Refused(Code::Revoked))
        );
        let (unavailable, link) = channel(Code::Unavailable, true, true);
        assert_eq!(
            unavailable.call("RelayCommand", params).await,
            Err(Failure::Uncertain {
                command_id: "original-command".into()
            })
        );
        assert_eq!(link.calls.load(Ordering::Relaxed), 1);
    }
}
