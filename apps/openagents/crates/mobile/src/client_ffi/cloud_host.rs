//! Our cloud (#11852): start and stop the account's engine on a cloud
//! computer. Once its run starts, the engine shows as one more device, named
//! [`CloudHostView::device_name`], in the normal device list.
use super::{CoreAccount, CoreError, CoreResult};
use openagents_account::openagents::{Error, cloud_host::CloudHost};

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CloudHostView {
    /// The engine's run is going (it may still be connecting).
    pub running: bool,
    /// The cloud computer's state, when there is one.
    pub computer_state: Option<String>,
    /// The name the engine's device shows under.
    pub device_name: String,
    /// When the current run ends on its own.
    pub until_unix: Option<u64>,
}

impl From<CloudHost> for CloudHostView {
    fn from(host: CloudHost) -> Self {
        Self {
            running: host.running(),
            computer_state: host.computer.map(|c| c.state),
            device_name: host.device_name,
            until_unix: host.until_unix,
        }
    }
}

/// What the person reads when Our cloud can't do what they asked.
pub(crate) fn message(error: &Error) -> CoreError {
    let message = match error {
        Error::Refused {
            code: Some("needs_claude"),
            ..
        } => "Add your Claude key in Settings on openagents.com to run Claude Code on Our cloud.",
        Error::Refused {
            code: Some("quota_exhausted"),
            ..
        } => "Delete an unused cloud computer before starting Our cloud.",
        Error::Forbidden
        | Error::Denied
        | Error::Refused {
            code: Some("invite_only"),
            ..
        } => "Our cloud isn't available for this account yet.",
        Error::Unauthorized | Error::ExpiredSession | Error::InvalidCredential => {
            return CoreError::Auth {
                message: "Log in again to continue.".into(),
            };
        }
        Error::Network { .. } => {
            return CoreError::Network {
                message: "Couldn't reach OpenAgents. Check your connection.".into(),
            };
        }
        _ => "Our cloud isn't available right now. Try again later.",
    };
    CoreError::HostError {
        message: message.into(),
    }
}

#[derive(Clone, Copy)]
enum Action {
    Status,
    Start,
    Stop,
}

impl CoreAccount {
    async fn cloud_host_call(&self, action: Action) -> CoreResult<CloudHostView> {
        let Some(lease) = self.controller().lease() else {
            return Err(CoreError::Auth {
                message: "Log in to use Our cloud.".into(),
            });
        };
        let task = openagents_client::runtime::shared().spawn(async move {
            let (client, session) = (lease.client(), lease.session());
            let result = match action {
                Action::Status => client.cloud_host(session, &lease.cancel).await,
                Action::Start => client.start_cloud_host(session, &lease.cancel).await,
                Action::Stop => client.stop_cloud_host(session, &lease.cancel).await,
            };
            if !lease.is_current() {
                return Err(Error::Unauthorized);
            }
            result
        });
        match task.await {
            Ok(Ok(host)) => Ok(host.into()),
            Ok(Err(error)) => Err(message(&error)),
            Err(_) => Err(CoreError::HostError {
                message: "Our cloud isn't available right now. Try again later.".into(),
            }),
        }
    }
}

#[uniffi::export]
impl CoreAccount {
    /// Our cloud's current state.
    pub async fn cloud_host(&self) -> CoreResult<CloudHostView> {
        self.cloud_host_call(Action::Status).await
    }
    /// Start Our cloud (or return the run already going).
    pub async fn start_cloud_host(&self) -> CoreResult<CloudHostView> {
        self.cloud_host_call(Action::Start).await
    }
    /// End Our cloud's engine and stop its computer.
    pub async fn stop_cloud_host(&self) -> CoreResult<CloudHostView> {
        self.cloud_host_call(Action::Stop).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_read_plainly() {
        let needs = message(&Error::Refused {
            status: 409,
            code: Some("needs_claude"),
        });
        assert!(needs.to_string().contains("Claude key"));
        let invite = message(&Error::Forbidden);
        assert!(
            invite
                .to_string()
                .contains("isn't available for this account")
        );
        assert!(matches!(
            message(&Error::Unauthorized),
            CoreError::Auth { .. }
        ));
    }
}
