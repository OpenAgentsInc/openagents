//! Exact destinations and policy versions. Wildcards and IP host selectors are
//! deliberately unsupported by the initial public profile format.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, net::IpAddr};
pub const PROFILE_SCHEMA: &str = "openagents.computers.egress-profile.v1";

/// The initial inspected transport is HTTPS on 443 and the gateway's two
/// policy-filtered DNS resolvers on 53. QUIC and other UDP cannot bypass HTTP
/// destination checks on an address shared by several public hostnames.
pub fn gateway_transport(tcp: bool, port: u16, reserved_dns: bool) -> bool {
    if reserved_dns {
        port == 53
    } else {
        tcp && port == 443
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Host(String);
impl Host {
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        if input.is_empty()
            || input.len() > 1024
            || input.ends_with('.')
            || input.chars().any(|c| c.is_control() || c.is_whitespace())
            || input.contains(['/', '\\', ':', '@', '*', '%', '?', '#', '[', ']'])
        {
            return Err("Use an exact DNS hostname without a port.");
        }
        let url::Host::Domain(name) =
            url::Host::parse(input).map_err(|_| "The hostname is invalid.")?
        else {
            return Err("IP host selectors are not supported.");
        };
        if name.len() > 253
            || !name.contains('.')
            || name.split('.').any(|s| {
                s.is_empty()
                    || s.len() > 63
                    || s.starts_with('-')
                    || s.ends_with('-')
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
        {
            return Err("The hostname is invalid.");
        }
        Ok(Self(name))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_private_name(&self) -> bool {
        [".internal", ".localhost", ".local", ".home.arpa"]
            .iter()
            .any(|suffix| self.0 == &suffix[1..] || self.0.ends_with(suffix))
    }
}
impl TryFrom<String> for Host {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<Host> for String {
    fn from(value: Host) -> Self {
        value.0
    }
}

/// The supported public transport is IPv4. IPv4-mapped addresses use the same
/// address classification; enabling native IPv6 requires separate qualification.
pub fn public_ip(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(ip) => ip,
            None => return false,
        },
    };
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || a == 0
        || a >= 240
        || a == 100 && (64..=127).contains(&b)
        || a == 192 && b == 0 && (c == 0 || c == 2)
        || a == 198 && (b == 18 || b == 19 || b == 51 && c == 100)
        || a == 203 && b == 0 && c == 113)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub name: String,
    pub allow_hosts: BTreeSet<Host>,
    #[serde(default)]
    pub tls_exempt_hosts: BTreeSet<Host>,
}
impl Profile {
    pub fn new(name: impl Into<String>, allow_hosts: BTreeSet<Host>) -> Self {
        Self {
            schema: PROFILE_SCHEMA.into(),
            name: name.into(),
            allow_hosts,
            tls_exempt_hosts: BTreeSet::new(),
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != PROFILE_SCHEMA
            || self.name.is_empty()
            || self.name.len() > 64
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || self.allow_hosts.len() > 128
            || self.allow_hosts.iter().any(Host::is_private_name)
            || !self.tls_exempt_hosts.is_subset(&self.allow_hosts)
        {
            return Err("The network profile is invalid.");
        }
        Ok(())
    }
    /// Pins the entire effective definition, rather than a mutable name.
    pub fn digest(&self) -> Result<String, &'static str> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"openagents.computers.egress-profile.v1\0");
        hash.update(serde_json::to_vec(self).map_err(|_| "The network profile is invalid.")?);
        Ok(format!("{:x}", hash.finalize()))
    }
    pub fn permits(&self, name: &str, resolved: IpAddr) -> bool {
        self.validate().is_ok()
            && public_ip(resolved)
            && Host::parse(name).is_ok_and(|host| self.allow_hosts.contains(&host))
    }
    pub fn selector(&self) -> Result<String, &'static str> {
        Ok(format!("{}@sha256:{}", self.name, self.digest()?))
    }
    /// Full native override, frozen at creation. Exact DNS allows still require
    /// gateway DNS attribution. Private ranges and native IPv6 remain denied.
    pub fn space_policy(&self) -> Result<serde_json::Value, &'static str> {
        self.validate()?;
        let deny = [
            "0.0.0.0/8",
            "10.0.0.0/8",
            "100.64.0.0/10",
            "127.0.0.0/8",
            "169.254.0.0/16",
            "172.16.0.0/12",
            "192.0.0.0/24",
            "192.0.2.0/24",
            "192.168.0.0/16",
            "198.18.0.0/15",
            "198.51.100.0/24",
            "203.0.113.0/24",
            "224.0.0.0/4",
            "240.0.0.0/4",
            "::/0",
        ];
        let mut allow = self
            .allow_hosts
            .iter()
            .map(|h| serde_json::json!({"domain":h.as_str()}))
            .collect::<Vec<_>>();
        allow.extend([
            serde_json::json!({"ip":"1.1.1.1"}),
            serde_json::json!({"ip":"8.8.8.8"}),
        ]);
        Ok(serde_json::json!({"version":"v1","tls_interception":"on",
            "egress":{"default_action":"deny",
                "allow":allow,
                "deny":deny.iter().map(|cidr|serde_json::json!({"cidr":cidr})).collect::<Vec<_>>()},
            "network_credential_rules":[],
            "tls_interception_exemptions":self.tls_exempt_hosts.iter().map(|h|serde_json::json!({"domain":h.as_str()})).collect::<Vec<_>>() }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}
impl Method {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "GET" => Some(Self::Get),
            "HEAD" => Some(Self::Head),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            "OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }
}
pub fn canonical_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 4096
        && !path.chars().any(|c| c.is_control() || c.is_whitespace())
        && !path.contains(['\\', '%', '?', '#'])
        && !path.split('/').any(|part| part == "." || part == "..")
        && !path.contains("//")
}
pub fn path_within(path: &str, prefix: &str) -> bool {
    canonical_path(path)
        && canonical_path(prefix)
        && (prefix == "/"
            || path == prefix
            || path
                .strip_prefix(prefix.trim_end_matches('/'))
                .is_some_and(|rest| rest.starts_with('/')))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub host: Host,
    pub port: u16,
    pub path_prefix: String,
    pub methods: BTreeSet<Method>,
}
impl Destination {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.port == 0
            || self.host.is_private_name()
            || self.methods.is_empty()
            || !canonical_path(&self.path_prefix)
        {
            return Err("The credential destination is invalid.");
        }
        Ok(())
    }
    pub fn matches(&self, host: &str, port: u16, path: &str, method: &str) -> bool {
        self.validate().is_ok()
            && port == self.port
            && Host::parse(host).is_ok_and(|h| h == self.host)
            && path_within(path, &self.path_prefix)
            && Method::parse(method).is_some_and(|m| self.methods.contains(&m))
    }
}

#[cfg(test)]
mod tests;
