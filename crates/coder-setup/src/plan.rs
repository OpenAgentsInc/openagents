//! What `coder link` records for `coder host serve`: the relays, the
//! workspaces, and a WebSocket listener on the tailnet address that the host
//! advertises as a `tailnet` hint.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use coder_host::settings::{AdvertiseSetting, ServeSettings, TlsSetting};

use crate::tailscale::Node;

/// The relay every linked host serves unless told otherwise.
pub const DEFAULT_RELAY: &str = "wss://relay.openagents.com/";
/// The WebSocket direct-channel port. The host service's TCP listener stays
/// on loopback `47100`.
pub const DEFAULT_PORT: u16 = 47101;

/// Certificate files `tailscale cert` wrote for the node's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsFiles {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Build the settings to record. Relays and workspaces given now replace or
/// add to what an earlier run recorded, so a re-run without them keeps them.
#[must_use]
pub fn settings(
    previous: &ServeSettings,
    relays: &[String],
    workspaces: &BTreeMap<String, PathBuf>,
    node: &Node,
    port: u16,
    tls: Option<&TlsFiles>,
) -> ServeSettings {
    let relays = if !relays.is_empty() {
        relays.to_vec()
    } else if !previous.relays.is_empty() {
        previous.relays.clone()
    } else {
        vec![DEFAULT_RELAY.to_owned()]
    };
    let mut all = previous.workspaces.clone();
    all.extend(workspaces.iter().map(|(k, v)| (k.clone(), v.clone())));
    let mut settings = ServeSettings::new(relays, all);
    settings.listen_websocket = Some(SocketAddr::from((node.ip, port)));
    settings.allow_nonloopback = true;
    let tls = tls.zip(node.dns_name.as_deref());
    let url = match tls {
        Some((files, name)) => {
            settings.websocket_tls = Some(TlsSetting {
                cert: files.cert.clone(),
                key: files.key.clone(),
                name: name.to_owned(),
            });
            format!("wss://{name}:{port}/")
        }
        None => format!("ws://{}:{port}/", node.ip),
    };
    settings.advertise = vec![AdvertiseSetting {
        class: "tailnet".into(),
        address: url,
    }];
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn node(dns: Option<&str>) -> Node {
        Node {
            ip: Ipv4Addr::new(100, 101, 102, 103),
            dns_name: dns.map(str::to_owned),
            certificates: dns.is_some(),
        }
    }

    #[test]
    fn a_tls_listener_advertises_its_tailnet_name() {
        let files = TlsFiles {
            cert: "/h/tls/chain.pem".into(),
            key: "/h/tls/key.pem".into(),
        };
        let workspaces = BTreeMap::from([("openagents".into(), PathBuf::from("/w/oa"))]);
        let settings = super::settings(
            &ServeSettings::default(),
            &[],
            &workspaces,
            &node(Some("box.tail0.ts.net")),
            DEFAULT_PORT,
            Some(&files),
        );
        assert_eq!(settings.relays, [DEFAULT_RELAY]);
        assert_eq!(
            settings.listen_websocket,
            Some("100.101.102.103:47101".parse().unwrap())
        );
        assert!(settings.allow_nonloopback);
        assert_eq!(
            settings.websocket_tls.as_ref().unwrap().name,
            "box.tail0.ts.net"
        );
        assert_eq!(settings.advertise[0].class, "tailnet");
        assert_eq!(
            settings.advertise[0].address,
            "wss://box.tail0.ts.net:47101/"
        );
        assert_eq!(settings.workspaces, workspaces);
    }

    #[test]
    fn without_a_certificate_the_hint_is_plain_ws_and_reruns_keep_settings() {
        let mut previous = ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([("old".into(), PathBuf::from("/w/old"))]),
        );
        previous.listen_websocket = Some("100.9.9.9:1".parse().unwrap());
        let files = TlsFiles {
            cert: "/c".into(),
            key: "/k".into(),
        };
        // A certificate without a DNS name cannot be advertised.
        let settings = super::settings(
            &previous,
            &[],
            &BTreeMap::new(),
            &node(None),
            9000,
            Some(&files),
        );
        assert_eq!(settings.relays, ["wss://relay.example/"]);
        assert!(settings.workspaces.contains_key("old"));
        assert_eq!(settings.websocket_tls, None);
        assert_eq!(settings.advertise[0].address, "ws://100.101.102.103:9000/");
        // Workspaces given now add to the recorded ones.
        let more = BTreeMap::from([("new".into(), PathBuf::from("/w/new"))]);
        let settings = super::settings(&previous, &[], &more, &node(None), 9000, None);
        assert_eq!(settings.workspaces.len(), 2);
    }
}
