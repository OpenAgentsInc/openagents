//! Computer and model choices for the new-session picker.
use super::{ChatConfig, CoreClient, CoreResult, on_runtime};

/// Put online Macs first for a new draft without a saved computer choice.
/// Keep unavailable computers in the list so a saved choice stays visible.
pub(super) fn execution_devices(
    devices: &[openagents_client::DeviceView],
) -> Vec<super::DeviceView> {
    let mut hosts: Vec<_> = devices.iter().filter(|d| d.is_execution_host).collect();
    hosts.sort_by_key(|d| (!d.online, d.platform != "macos"));
    hosts.into_iter().map(Into::into).collect()
}

#[uniffi::export]
impl CoreClient {
    /// Keep a supported draft, or choose from the selected host's successful live catalog.
    pub async fn available_chat_config(
        &self,
        device_id: String,
        config: ChatConfig,
    ) -> CoreResult<Option<ChatConfig>> {
        let config = config.try_into()?;
        let client = self.client.clone();
        on_runtime(async move {
            Ok(client
                .available_chat_config(&device_id, config)
                .await
                .map(|config| (&config).into()))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, platform: &str, online: bool) -> openagents_client::DeviceView {
        openagents_client::DeviceView {
            id: id.into(),
            name: id.into(),
            platform: platform.into(),
            online,
            last_seen_ms: None,
            version: None,
            capabilities: vec![],
            is_execution_host: platform != "ios",
            is_self: false,
            session_count: 0,
        }
    }

    #[test]
    fn fresh_drafts_prefer_an_online_mac_over_the_first_registered_computer() {
        let devices = [
            device("coderos", "linux", true),
            device("offline-mac", "macos", false),
            device("online-mac", "macos", true),
            device("phone", "ios", true),
        ];
        let hosts = execution_devices(&devices);
        let ids: Vec<_> = hosts.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["online-mac", "coderos", "offline-mac"]);
        // A saved offline choice remains available to show, without substitution.
        assert!(!hosts.iter().find(|h| h.id == "offline-mac").unwrap().online);
    }

    #[test]
    fn online_computers_precede_offline_macs_and_equal_choices_keep_their_order() {
        let devices = [
            device("offline-mac", "macos", false),
            device("first-server", "linux", true),
            device("second-server", "linux", true),
        ];
        let hosts = execution_devices(&devices);
        let ids: Vec<_> = hosts.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["first-server", "second-server", "offline-mac"]);
        assert!(execution_devices(&[device("phone", "ios", true)]).is_empty());
    }
}
