//! Cloud computer addresses and their matching session hosts.
use openagents_proto::{Device, DeviceOrigin};

pub fn valid_id(id: &str) -> bool {
    id.strip_prefix("cmp_")
        .is_some_and(|s| s.len() == 20 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Match by the computer ID, never a display name another device can share.
pub fn session_host<'a>(devices: &'a [Device], computer: &str) -> Option<&'a Device> {
    if !valid_id(computer) {
        return None;
    }
    devices.iter().find(|d| matches!(&d.origin, DeviceOrigin::ManagedCloud { computer_id } if computer_id == computer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cloud_computers_match_session_hosts_by_id_not_name() {
        let id = "cmp_0123456789abcdef0123";
        let devices: Vec<Device> = serde_json::from_value(json!([
            {"id":"personal", "name":"Our cloud", "platform":"linux", "lastSeenAt":null},
            {"id":"engine", "name":"A renamed cloud", "platform":"linux", "lastSeenAt":null,
             "origin":{"kind":"managedCloud", "computer_id":id}}
        ]))
        .unwrap();
        assert_eq!(session_host(&devices, id).unwrap().id, "engine");
        assert!(session_host(&devices, "cmp_aaaaaaaaaaaaaaaaaaaa").is_none());
        assert!(session_host(&devices, "../stop").is_none());
        assert!(!valid_id("cmp_0123456789abcdef012/"));
        assert!(!valid_id("cmp_0123456789abcdef0123?kind=cloud"));
    }
}
