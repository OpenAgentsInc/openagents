//! Web addresses typed into the browser or linked from a chat, checked
//! before anything opens them.

#[cfg(not(target_family = "wasm"))]
use std::net::IpAddr;

#[cfg(not(target_family = "wasm"))]
pub fn loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => host == "localhost" || host.ends_with(".localhost"),
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    }
}

pub fn normalize_address(input: &str) -> Result<String, &'static str> {
    let text = input.trim();
    if text.is_empty() {
        return Err("Enter a website or localhost address.");
    }
    if text.chars().any(|c| c.is_control()) {
        return Err("This address contains invalid characters.");
    }
    // A bare host:port looks like a URI scheme to a URL parser. Only accept
    // that ambiguity when the suffix is an actual numeric port.
    let authority = text.split(['/', '?', '#']).next().unwrap_or(text);
    let host_port = authority
        .rsplit_once(':')
        .is_some_and(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()));
    let explicit =
        text.contains("://") || (text.contains(':') && !host_port && !text.starts_with('['));
    let address = if explicit {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    checked(&address, explicit)
}

#[cfg(not(target_family = "wasm"))]
fn checked(address: &str, explicit: bool) -> Result<String, &'static str> {
    let mut parsed =
        url::Url::parse(address).map_err(|_| "Enter a valid website or localhost address.")?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Only http and https addresses are supported.");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Use an address without an embedded username or password.");
    }
    if !explicit && loopback(&parsed) {
        let _ = parsed.set_scheme("http");
    }
    Ok(parsed.into())
}

/// The browser's own URL parser (the same WHATWG rules `url` follows), so
/// the page doesn't carry `url`'s international-domain tables.
#[cfg(target_family = "wasm")]
fn checked(address: &str, explicit: bool) -> Result<String, &'static str> {
    let parsed =
        web_sys::Url::new(address).map_err(|_| "Enter a valid website or localhost address.")?;
    if !matches!(parsed.protocol().as_str(), "http:" | "https:") || parsed.hostname().is_empty() {
        return Err("Only http and https addresses are supported.");
    }
    if !parsed.username().is_empty() || !parsed.password().is_empty() {
        return Err("Use an address without an embedded username or password.");
    }
    if !explicit && loopback_host(&parsed.hostname()) {
        parsed.set_protocol("http");
    }
    Ok(parsed.href())
}

/// The browser serializes an IPv4 host as four decimal parts and the IPv6
/// loopback as `[::1]`.
#[cfg(target_family = "wasm")]
fn loopback_host(host: &str) -> bool {
    let ipv4 = host.split('.').collect::<Vec<_>>();
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "[::1]"
        || (ipv4.len() == 4
            && ipv4[0] == "127"
            && ipv4
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())))
}

/// Chat links require an explicit web authority; never infer a scheme or
/// silently strip controls, credentials, malformed escapes or backslashes.
pub fn transcript_address(input: &str) -> Result<String, &'static str> {
    let lower = input.to_ascii_lowercase();
    let authority = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or("Only explicit http and https links are supported.")?;
    if input.chars().any(|c| c.is_control() || c.is_whitespace())
        || input.contains('\\')
        || authority.is_empty()
        || authority.starts_with(['/', '?', '#'])
    {
        return Err("This link contains an invalid address.");
    }
    for (i, byte) in input.bytes().enumerate() {
        if byte == b'%'
            && !input
                .as_bytes()
                .get(i + 1..i + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
        {
            return Err("This link contains an invalid escape.");
        }
    }
    normalize_address(input)
}
