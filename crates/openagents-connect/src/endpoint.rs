//! The iroh endpoint every OpenAgents side binds.
//!
//! `presets::Minimal` (a crypto provider and nothing else), then either our
//! relay alone (`RelayMode::Custom`) or no relay, and a
//! [`MemoryLookup`] that the caller feeds from a scanned code and from
//! NIP-REACH hints. There is no n0 relay, n0 DNS, PKARR, or DHT.

use std::net::SocketAddr;

use iroh::address_lookup::MemoryLookup;
use iroh::endpoint::{BindError, presets};
use iroh::{Endpoint, EndpointAddr, RelayMode, RelayUrl, SecretKey};

use crate::{Code, ENROLL_ALPN, Error, REACH_ALPN, RELAY_URL, Result};

/// Which relay an endpoint uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relay {
    /// No relay: direct addresses only. Tests and same-network use.
    Disabled,
    /// One relay, ours unless configured otherwise.
    Custom(RelayUrl),
}

impl Relay {
    /// The OpenAgents relay, [`RELAY_URL`].
    ///
    /// # Panics
    /// Never: the constant is a valid URL, which a unit test checks.
    #[must_use]
    pub fn openagents() -> Self {
        Self::Custom(RELAY_URL.parse().expect("RELAY_URL is a valid URL"))
    }

    fn mode(&self) -> RelayMode {
        match self {
            Self::Disabled => RelayMode::Disabled,
            Self::Custom(url) => RelayMode::custom([url.clone()]),
        }
    }
}

/// How to bind an endpoint.
#[derive(Clone, Debug)]
pub struct EndpointConfig {
    pub relay: Relay,
    /// IP sockets to bind. Empty binds iroh's defaults (every interface, a
    /// random port, IPv4 and IPv6).
    pub bind: Vec<SocketAddr>,
    /// ALPNs this endpoint answers. A host answers [`host_alpns`]; a device
    /// that only dials answers none.
    pub alpns: Vec<Vec<u8>>,
}

impl EndpointConfig {
    /// A host: our relay, default sockets, both ALPNs.
    #[must_use]
    pub fn host() -> Self {
        Self {
            relay: Relay::openagents(),
            bind: Vec::new(),
            alpns: host_alpns(),
        }
    }

    /// A device that only dials: our relay, default sockets, no ALPNs.
    #[must_use]
    pub fn device() -> Self {
        Self {
            relay: Relay::openagents(),
            bind: Vec::new(),
            alpns: Vec::new(),
        }
    }

    /// Relays disabled, bound to `127.0.0.1` on a random port. For tests.
    #[must_use]
    pub fn loopback(alpns: Vec<Vec<u8>>) -> Self {
        Self {
            relay: Relay::Disabled,
            bind: vec![SocketAddr::from(([127, 0, 0, 1], 0))],
            alpns,
        }
    }
}

/// The ALPNs a host answers.
#[must_use]
pub fn host_alpns() -> Vec<Vec<u8>> {
    vec![ENROLL_ALPN.to_vec(), REACH_ALPN.to_vec()]
}

/// A bound endpoint and the lookup that resolves peers for it.
#[derive(Clone, Debug)]
pub struct ConnectEndpoint {
    pub endpoint: Endpoint,
    pub lookup: MemoryLookup,
}

impl ConnectEndpoint {
    /// Bind an endpoint with `secret` as its iroh key.
    ///
    /// # Errors
    /// `malformed` for an unusable bind address; `unavailable` when the
    /// sockets cannot be bound.
    pub async fn bind(secret: SecretKey, config: EndpointConfig) -> Result<Self> {
        let lookup = MemoryLookup::new();
        let mut builder = Endpoint::builder(presets::Minimal)
            .secret_key(secret)
            .relay_mode(config.relay.mode())
            .alpns(config.alpns)
            .address_lookup(lookup.clone());
        if !config.bind.is_empty() {
            builder = builder.clear_ip_transports();
            for addr in config.bind {
                builder = builder
                    .bind_addr(addr)
                    .map_err(|_| Error::new(Code::Malformed, "bind address"))?;
            }
        }
        let endpoint = builder.bind().await.map_err(bind_error)?;
        Ok(Self { endpoint, lookup })
    }

    /// Remember addresses for a peer, from a scanned code or a NIP-REACH
    /// hint. Later dials by endpoint ID alone use them.
    pub fn remember(&self, addr: EndpointAddr) {
        self.lookup.add_endpoint_info(addr);
    }

    /// This endpoint's address with only the sockets it bound, with an
    /// unspecified address replaced by loopback. What a same-machine test
    /// dials.
    #[must_use]
    pub fn local_addr(&self) -> EndpointAddr {
        let mut addr = EndpointAddr::new(self.endpoint.id());
        for socket in self.endpoint.bound_sockets() {
            let socket = match socket {
                SocketAddr::V4(v4) if v4.ip().is_unspecified() => {
                    SocketAddr::from(([127, 0, 0, 1], v4.port()))
                }
                SocketAddr::V6(v6) if v6.ip().is_unspecified() => {
                    SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, v6.port()))
                }
                other => other,
            };
            addr = addr.with_ip_addr(socket);
        }
        addr
    }

    /// Close the endpoint and every connection on it.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

fn bind_error(_: BindError) -> Error {
    Error::new(Code::Unavailable, "could not bind the endpoint")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_constant_parses() {
        let Relay::Custom(url) = Relay::openagents() else {
            panic!("custom relay");
        };
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("iroh.openagents.com"));
    }
}
