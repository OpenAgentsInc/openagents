//! Our cloud (#11852): the account's OpenAgents engine on one of its cloud
//! computers. Starting it makes the computer the first time and runs the
//! engine for up to an hour; the engine then shows as one more of the
//! account's devices. Stop ends it and stops the computer.
use crate::{CancellationToken, Client, Error, Session};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

/// The cloud computer behind Our cloud.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CloudHostComputer {
    pub id: String,
    pub state: String,
}

/// Our cloud's state. `engine` is `running` while the engine's run lasts.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CloudHost {
    pub computer: Option<CloudHostComputer>,
    pub engine: String,
    pub device_name: String,
    pub started_unix: Option<u64>,
    pub until_unix: Option<u64>,
}

impl CloudHost {
    /// Whether the engine is running.
    pub fn running(&self) -> bool {
        self.engine == "running"
    }
}

impl Client {
    async fn cloud_host_call(
        &self,
        session: &Session,
        method: Method,
        segments: &[&str],
        cancel: &CancellationToken,
    ) -> Result<CloudHost, Error> {
        self.check_session(session)?;
        let body = (method != Method::GET).then(|| json!({}));
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.request(method, segments, &[], body, Some(session.token())) => result,
        }
    }

    /// Our cloud's current state.
    pub async fn cloud_host(
        &self,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<CloudHost, Error> {
        self.cloud_host_call(session, Method::GET, &["cloud-host"], cancel)
            .await
    }

    /// Start Our cloud, or return the run already going.
    pub async fn start_cloud_host(
        &self,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<CloudHost, Error> {
        self.cloud_host_call(session, Method::POST, &["cloud-host"], cancel)
            .await
    }

    /// End the engine and stop the computer.
    pub async fn stop_cloud_host(
        &self,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<CloudHost, Error> {
        self.cloud_host_call(session, Method::POST, &["cloud-host", "stop"], cancel)
            .await
    }

    /// End the engine and delete the computer.
    pub async fn delete_cloud_host(
        &self,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<CloudHost, Error> {
        self.cloud_host_call(session, Method::DELETE, &["cloud-host"], cancel)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_answer() {
        let host: CloudHost = serde_json::from_value(json!({
            "computer": {"id": "cmp_0123456789abcdef0123", "name": "Cloud computer 012345",
                "kind": "cloud", "state": "turn", "online": true},
            "engine": "running", "device_name": "Our cloud",
            "started_unix": 10, "until_unix": 3610
        }))
        .unwrap();
        assert!(host.running());
        assert_eq!(host.computer.unwrap().state, "turn");
        let off: CloudHost = serde_json::from_value(json!({
            "computer": null, "engine": "stopped", "device_name": "Our cloud",
            "started_unix": null, "until_unix": null
        }))
        .unwrap();
        assert!(!off.running());
    }
}
