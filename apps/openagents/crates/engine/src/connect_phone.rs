//! Connect your phone (#11998): the engine half of the window that shows a
//! rotating connect code (NIP-HOST, "Showing a connect code").
//!
//! The device-connection host makes and cancels the invitations; the window
//! decides when to rotate them. [`LocalRpc`] serves the two methods in
//! [`openagents_rpc::connect_phone`] only on this computer's own
//! transports (in process and on localhost). The relay host and the
//! NIP-HOST app RPC reach the plain engine service, which doesn't know
//! these methods, so another device can never make a code.

use async_trait::async_trait;
use openagents_rpc::connect_phone::{
    CANCEL_CONNECT_CODES, CREATE_CONNECT_CODE, CancelConnectCodes, ConnectCode,
};
use openagents_rpc::{RpcError, RpcReply, RpcService};
use std::sync::{Arc, Mutex};

/// The environment variable that turns the device connection off. It is on
/// unless this is `0`, `off`, `false`, or `no`.
pub const NIP_HOST_VAR: &str = "OPENAGENTS_NIP_HOST";

/// Whether the device connection runs, from [`NIP_HOST_VAR`]'s value.
pub fn nip_host_on(value: Option<&str>) -> bool {
    !value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        )
    })
}

/// The running host's connect codes, while the device connection runs.
pub(crate) type Slot = Arc<Mutex<Option<coder_host::ConnectCodes>>>;

const UNAVAILABLE: &str = "This computer can't show a connect code right now.";

fn codes(slot: &Slot) -> Result<coder_host::ConnectCodes, RpcError> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .ok_or_else(|| RpcError::Failed(UNAVAILABLE.into()))
}

/// Make a new connect code.
pub(crate) fn create(slot: &Slot) -> Result<ConnectCode, RpcError> {
    let issued = codes(slot)?.issue().map_err(|error| {
        tracing::warn!(%error, "connect code refused");
        RpcError::Failed(UNAVAILABLE.into())
    })?;
    Ok(ConnectCode {
        invitation: issued.invitation,
        link: issued.code.encode_link(),
        text: issued.code.encode(),
        issued_at: issued.code.issued_at(),
        expires_at: issued.expires_at,
        label: issued.code.label().to_owned(),
        // #12000 adds the desktop's account-link request here.
        account_link: None,
    })
}

/// Cancel connect codes. With the host gone there is nothing to cancel.
pub(crate) fn cancel(slot: &Slot, invitations: &[String]) -> Result<(), RpcError> {
    let Ok(codes) = codes(slot) else {
        return Ok(());
    };
    let mut failed = false;
    for invitation in invitations {
        if let Err(error) = codes.cancel(invitation) {
            tracing::warn!(%error, "connect code cancel failed");
            failed = true;
        }
    }
    if failed {
        return Err(RpcError::Failed(
            "Couldn't cancel an earlier connect code.".into(),
        ));
    }
    Ok(())
}

/// The engine service for this computer's own app: the plain service plus
/// the Connect your phone methods.
pub struct LocalRpc {
    pub(crate) inner: Arc<dyn RpcService>,
    pub(crate) slot: Slot,
}

#[async_trait]
impl RpcService for LocalRpc {
    async fn handle(&self, method: &str, params: serde_json::Value) -> Result<RpcReply, RpcError> {
        match method {
            CREATE_CONNECT_CODE => {
                let slot = self.slot.clone();
                // The access store writes a file; keep it off the async workers.
                let code = tokio::task::spawn_blocking(move || create(&slot))
                    .await
                    .map_err(|_| RpcError::Failed(UNAVAILABLE.into()))??;
                RpcReply::value(&code)
            }
            CANCEL_CONNECT_CODES => {
                let params: CancelConnectCodes = serde_json::from_value(params)
                    .map_err(|error| RpcError::BadParams(error.to_string()))?;
                let slot = self.slot.clone();
                tokio::task::spawn_blocking(move || cancel(&slot, &params.invitations))
                    .await
                    .map_err(|_| RpcError::Failed(UNAVAILABLE.into()))??;
                RpcReply::value(&serde_json::json!({}))
            }
            _ => self.inner.handle(method, params).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    #[async_trait]
    impl RpcService for Echo {
        async fn handle(&self, method: &str, _: serde_json::Value) -> Result<RpcReply, RpcError> {
            Err(RpcError::UnknownMethod(method.into()))
        }
    }

    fn value(reply: RpcReply) -> serde_json::Value {
        match reply {
            RpcReply::Value(value) => value,
            RpcReply::Stream(_) => panic!("a value"),
        }
    }

    async fn host(temp: &std::path::Path) -> coder_host::Running {
        let access = temp.join("access");
        let mut config = coder_host::Config::new(access, vec!["ws://127.0.0.1:9/".into()], 1);
        config.policy = coder_access::RelayPolicy::LoopbackTest;
        config.iroh = Some(coder_host::config::Iroh::loopback());
        config.label = "Studio Mac".into();
        config.telemetry = false;
        config.advertise_listener = false;
        config.keys = Some(coder_host::serve::keys::Keys(Arc::new(
            coder_host::serve::keys::FileKeySource::new(temp.join("keys")),
        )));
        coder_host::start(config, Arc::new(coder_host::NoTasks))
            .await
            .unwrap()
    }

    #[test]
    fn the_device_connection_is_on_unless_turned_off() {
        assert!(nip_host_on(None));
        assert!(nip_host_on(Some("1")));
        assert!(nip_host_on(Some("")));
        for off in ["0", "off", "FALSE", " no "] {
            assert!(!nip_host_on(Some(off)), "{off}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_window_gets_a_connect_link_and_cancels_it() {
        let temp = tempfile::tempdir().unwrap();
        let running = host(temp.path()).await;
        let slot: Slot = Arc::new(Mutex::new(Some(running.connect_codes())));
        let rpc = LocalRpc {
            inner: Arc::new(Echo),
            slot: slot.clone(),
        };

        let code: ConnectCode = serde_json::from_value(value(
            rpc.handle(CREATE_CONNECT_CODE, serde_json::json!({}))
                .await
                .unwrap(),
        ))
        .unwrap();
        assert!(code.link.starts_with("https://openagents.com/connect#"));
        assert!(code.text.starts_with("openagents-connect:"));
        assert_eq!(code.expires_at, code.issued_at + 300);
        assert_eq!(code.label, "Studio Mac");
        assert!(code.account_link.is_none());
        let parsed = openagents_connect::code::ConnectCode::parse_shape(&code.link).unwrap();
        assert_eq!(parsed.invitation(), code.invitation);
        assert_eq!(parsed.host(), running.host_key());

        // A second code is a different invitation.
        let next: ConnectCode = serde_json::from_value(value(
            rpc.handle(CREATE_CONNECT_CODE, serde_json::json!({}))
                .await
                .unwrap(),
        ))
        .unwrap();
        assert_ne!(next.invitation, code.invitation);
        rpc.handle(
            CANCEL_CONNECT_CODES,
            serde_json::json!({"invitations": [code.invitation, next.invitation]}),
        )
        .await
        .unwrap();

        // Every other method reaches the plain service.
        assert!(matches!(
            rpc.handle("ListChats", serde_json::json!({})).await,
            Err(RpcError::UnknownMethod(_))
        ));

        // With the device connection off, the window says so in plain words.
        slot.lock().unwrap().take();
        let Err(RpcError::Failed(message)) =
            rpc.handle(CREATE_CONNECT_CODE, serde_json::json!({})).await
        else {
            panic!("refused");
        };
        assert!(oa_copy::violations(&message, &[]).is_empty(), "{message}");
        running.shutdown().await;
    }
}
