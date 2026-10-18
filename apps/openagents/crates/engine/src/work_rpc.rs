//! The app's Work/Pool connection to the public SDK host service.
use openagents_rpc::{RpcError, RpcReply};
use openagents_sdk::work::{self as api, Transport as _};
use serde_json::Value;

fn operation(method: &str) -> Option<&'static str> {
    [
        api::START,
        api::LIST,
        api::EVENTS,
        api::CANCEL,
        api::POOL_STATUS,
        api::POOL_UP,
        api::POOL_DOWN,
    ]
    .into_iter()
    .find(|name| *name == method)
}

pub(super) fn handles(method: &str) -> bool {
    operation(method).is_some()
}

pub(super) async fn handle(
    service: &coder_cloud::work::Service,
    method: &str,
    params: Value,
) -> Result<RpcReply, RpcError> {
    let method = operation(method)
        .ok_or_else(|| RpcError::Failed("Choose a Work or Pool operation.".into()))?;
    let cancel = openagents_sdk::CancellationToken::new();
    // Keep typed refusals, including the owner's command, across engine RPC.
    RpcReply::value(&service.request(method, params, &cancel).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_sdk::work::{Error, PoolStatus, WorkRequest};
    struct Fake;
    impl coder_cloud::work::Runner for Fake {
        fn admit(&self, _: &WorkRequest) -> Result<(), Error> {
            Err(Error::refused("No run in this fixture."))
        }
        fn execute(&self, _: &coder_cloud::Store, _: &str) -> Result<(), Error> {
            Err(Error::refused("No run in this fixture."))
        }
        fn pool(&self, method: &str, agent: &str, _: u32) -> Result<PoolStatus, Error> {
            assert_eq!(agent, "agt_reviewer");
            if method == api::POOL_UP {
                Err(Error {
                    code: "refused".into(),
                    message: "The pool needs an owner grant.".into(),
                    owner_command: Some(
                        "openagents cloud up --hosts 1 --agent agt_reviewer".into(),
                    ),
                })
            } else {
                Ok(PoolStatus {
                    slots: 3,
                    ..Default::default()
                })
            }
        }
    }
    #[tokio::test]
    async fn connection_preserves_public_sdk_results_and_owner_commands() {
        let root = tempfile::tempdir().unwrap();
        let service = coder_cloud::work::Service::new(root.path(), std::sync::Arc::new(Fake));
        let RpcReply::Value(value) = handle(
            &service,
            api::POOL_STATUS,
            serde_json::json!({"agent":"agt_reviewer"}),
        )
        .await
        .unwrap() else {
            panic!("expected a unary reply")
        };
        let result: Result<PoolStatus, Error> = serde_json::from_value(value).unwrap();
        assert_eq!(result.unwrap().slots, 3);
        let RpcReply::Value(value) = handle(
            &service,
            api::POOL_UP,
            serde_json::json!({"agent":"agt_reviewer","hosts":1}),
        )
        .await
        .unwrap() else {
            panic!("expected a unary reply")
        };
        let result: Result<PoolStatus, Error> = serde_json::from_value(value).unwrap();
        assert_eq!(
            result.unwrap_err().owner_command.as_deref(),
            Some("openagents cloud up --hosts 1 --agent agt_reviewer")
        );
        assert!(handles(api::START) && handles(api::CANCEL));
        assert!(!handles("unrelated"));
    }
}
