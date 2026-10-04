//! SOCKS5 proxy configuration shared by every connection the client opens.

/// A SOCKS5 proxy every outbound connection is routed through.
///
/// Hostnames are always resolved by the proxy, never locally, so a DNS query
/// never reveals which host is being reached. There is deliberately no knob to
/// turn that off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    /// Proxy host. An IP address or a name resolvable on the local network:
    /// reaching the proxy itself is the one lookup that cannot go through it.
    pub host: String,
    pub port: u16,
    /// Username for SOCKS5 username/password authentication (RFC 1929).
    /// Both this and `password` must be set for authentication to be offered.
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    /// A proxy at `host:port` with no authentication.
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            username: None,
            password: None,
        }
    }

    /// Credentials, when both halves are present. A username without a
    /// password (or the reverse) is not a usable RFC 1929 exchange.
    #[must_use]
    pub fn credentials(&self) -> Option<(&str, &str)> {
        match (&self.username, &self.password) {
            (Some(user), Some(pass)) => Some((user.as_str(), pass.as_str())),
            _ => None,
        }
    }

    /// `host:port`, the form both the reqwest proxy URL and the WebSocket
    /// dialer use.
    ///
    /// A bare IPv6 literal is bracketed: without brackets its own colons read
    /// as the port separator, and neither a URL nor a socket address parses.
    #[must_use]
    pub fn address(&self) -> String {
        if self.host.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// The `socks5h` URL reqwest needs. The `h` suffix is what makes reqwest
    /// hand the hostname to the proxy instead of resolving it locally.
    ///
    /// Credentials are carried in the URL userinfo, percent-encoded so that a
    /// password containing `:`, `@` or `/` cannot break out of its component.
    #[must_use]
    pub fn reqwest_url(&self) -> String {
        match self.credentials() {
            Some((user, pass)) => format!(
                "socks5h://{}:{}@{}",
                percent_encode_userinfo(user),
                percent_encode_userinfo(pass),
                self.address()
            ),
            None => format!("socks5h://{}", self.address()),
        }
    }
}

/// Percent-encodes everything outside the unreserved set, which is stricter
/// than the userinfo grammar allows but keeps the encoding trivially correct.
fn percent_encode_userinfo(value: &str) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[macros::test_all]
    fn url_uses_socks5h_so_dns_stays_remote() {
        let proxy = ProxyConfig::new("127.0.0.1", 9050);
        assert_eq!(proxy.reqwest_url(), "socks5h://127.0.0.1:9050");
    }

    #[macros::test_all]
    fn ipv6_host_is_bracketed() {
        let proxy = ProxyConfig::new("::1", 9050);
        assert_eq!(proxy.address(), "[::1]:9050");
        assert_eq!(proxy.reqwest_url(), "socks5h://[::1]:9050");
    }

    #[macros::test_all]
    fn already_bracketed_ipv6_host_is_left_alone() {
        assert_eq!(ProxyConfig::new("[::1]", 9050).address(), "[::1]:9050");
    }

    #[macros::test_all]
    fn credentials_need_both_halves() {
        let mut proxy = ProxyConfig::new("127.0.0.1", 9050);
        proxy.username = Some("user".to_string());
        assert_eq!(proxy.credentials(), None);
        proxy.password = Some("pass".to_string());
        assert_eq!(proxy.credentials(), Some(("user", "pass")));
    }

    #[macros::test_all]
    fn userinfo_specials_are_encoded() {
        let proxy = ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: 9050,
            username: Some("us er".to_string()),
            password: Some("p@ss:/word".to_string()),
        };
        assert_eq!(
            proxy.reqwest_url(),
            "socks5h://us%20er:p%40ss%3A%2Fword@127.0.0.1:9050"
        );
    }
}
