//! Direct-to-designer LNURL invoices and settlement checks.
use super::*;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Invoice {
    pub bolt11: String,
    pub hash: String,
    pub verify: String,
    pub expires: u64,
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0))
                || (a == 198 && (b == 18 || b == 19 || b == 51 && c == 100))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(ip));
            }
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] == 0xdb8 || s[1] == 0 || s[1] & 0xfff0 == 0x0010))
                && s[0] != 0x2002
        }
    }
}
fn url(value: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(value).map_err(|_| "Use a Lightning wallet with an HTTPS address.")?;
    if value.len() > 4096
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some_and(|p| p != 443)
    {
        return Err("Use a Lightning wallet with a public HTTPS address.".into());
    }
    Ok(url)
}
pub(super) fn address(value: &str) -> Result<reqwest::Url, String> {
    let (name, host) = value
        .trim()
        .split_once('@')
        .filter(|(name, host)| {
            !name.is_empty()
                && name.len() <= 80
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
                && !host.contains(['@', '/', ':', '?', '#'])
        })
        .ok_or("Enter a Lightning address, such as you@your-wallet.com.")?;
    let mut url = url(&format!("https://{host}"))?;
    url.path_segments_mut()
        .map_err(|_| "Check the Lightning address.")?
        .push(".well-known")
        .push("lnurlp")
        .push(name);
    Ok(url)
}
#[cfg(test)]
pub(super) static TEST_SERVICE: OnceLock<String> = OnceLock::new();
async fn get(url: &reqwest::Url) -> Result<Value, String> {
    static NETWORK: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let _permit = tokio::time::timeout(
        Duration::from_secs(2),
        NETWORK
            .get_or_init(|| tokio::sync::Semaphore::new(4))
            .acquire(),
    )
    .await
    .map_err(|_| "Payment checks are busy. Try again in a moment.")?
    .map_err(|_| "The wallet connection stopped.")?;
    #[cfg(test)]
    if url.host_str() == Some("wallet.test") {
        if let Some(base) = TEST_SERVICE.get() {
            let target = format!(
                "{base}{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default()
            );
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|_| "The wallet couldn't be reached.")?;
            return response(
                client
                    .get(target)
                    .send()
                    .await
                    .map_err(|_| "The wallet couldn't be reached.")?,
            )
            .await;
        }
    }
    let host = url.host_str().ok_or("Check the wallet address.")?;
    let addresses: Vec<SocketAddr> =
        tokio::time::timeout(Duration::from_secs(3), tokio::net::lookup_host((host, 443)))
            .await
            .map_err(|_| "The wallet couldn't be reached.")?
            .map_err(|_| "The wallet couldn't be reached.")?
            .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err("Use a Lightning wallet on a public internet address.".into());
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addresses)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "The wallet couldn't be reached.")?;
    response(
        client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| "The wallet couldn't be reached.")?,
    )
    .await
}
async fn response(mut reply: reqwest::Response) -> Result<Value, String> {
    if !reply.status().is_success() {
        return Err("The Lightning wallet couldn't answer. Try again later.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = reply
        .chunk()
        .await
        .map_err(|_| "The wallet reply couldn't be read.")?
    {
        if bytes.len() + chunk.len() > 256 * 1024 {
            return Err("The wallet reply is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "The wallet reply couldn't be read.".into())
}
pub(super) async fn create(destination: &str, sats: u64, now: u64) -> Result<Invoice, String> {
    let service = address(destination)?;
    let offer = get(&service).await?;
    let amount = sats.checked_mul(1000).ok_or("Choose a smaller price.")?;
    if offer["tag"] != "payRequest"
        || !offer["minSendable"].as_u64().is_some_and(|v| v <= amount)
        || !offer["maxSendable"].as_u64().is_some_and(|v| v >= amount)
    {
        return Err("This wallet can't receive the listed price. Ask the designer to check their Lightning address.".into());
    }
    let metadata = offer["metadata"]
        .as_str()
        .ok_or("The wallet didn't describe its invoice.")?;
    let metadata_rows: Vec<Vec<Value>> =
        serde_json::from_str(metadata).map_err(|_| "The wallet didn't describe its invoice.")?;
    if !metadata_rows.iter().any(|r| {
        r.first().and_then(Value::as_str) == Some("text/plain")
            && r.get(1).is_some_and(Value::is_string)
    }) {
        return Err("The wallet didn't describe its invoice.".into());
    }
    let mut callback = url(offer["callback"]
        .as_str()
        .ok_or("The wallet didn't offer an invoice.")?)?;
    let query: Vec<(String, String)> = callback
        .query_pairs()
        .filter(|(k, _)| k != "amount")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    callback.set_query(None);
    callback
        .query_pairs_mut()
        .extend_pairs(query)
        .append_pair("amount", &amount.to_string());
    let reply = get(&callback).await?;
    let bolt11 = reply["pr"]
        .as_str()
        .ok_or("The wallet didn't offer an invoice.")?;
    let invoice = nostr::x402::decode_invoice(bolt11)
        .map_err(|_| "The wallet sent an invalid Lightning invoice.")?;
    let expires = invoice
        .created_at()
        .checked_add(invoice.expiry_seconds())
        .ok_or("The wallet sent an invalid expiry.")?;
    let hash: [u8; 32] = Sha256::digest(metadata.as_bytes()).into();
    if invoice.currency() != "bc"
        || invoice.amount_msat() != amount
        || invoice.description_hash() != hash
        || invoice.created_at() > now.saturating_add(60)
        || expires <= now.saturating_add(30)
        || expires > now.saturating_add(86400)
    {
        return Err(
            "The wallet invoice doesn't match this purchase. No payment was requested.".into(),
        );
    }
    let verify=url(reply["verify"].as_str().ok_or("This wallet doesn't support payment checks. Ask the designer to use a wallet with LNURL-verify.")?)?;
    Ok(Invoice {
        bolt11: bolt11.into(),
        hash: hex(&invoice.payment_hash()),
        verify: verify.into(),
        expires,
    })
}
pub(super) async fn settled(invoice: &Invoice) -> Result<bool, String> {
    let result = get(&url(&invoice.verify)?).await?;
    if result["status"] == "ERROR" {
        return Err("The wallet couldn't check this payment. Try again later.".into());
    }
    if result["settled"] != true {
        return Ok(false);
    }
    if result["pr"].as_str() != Some(invoice.bolt11.as_str()) {
        return Err("The wallet answered for a different invoice.".into());
    }
    let preimage = result["preimage"]
        .as_str()
        .and_then(unhex)
        .filter(|p| p.len() == 32)
        .ok_or("The wallet didn't provide a valid payment proof.")?;
    if hex(&Sha256::digest(preimage)) != invoice.hash {
        return Err("The wallet's payment proof doesn't match this invoice.".into());
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn wallet_addresses_cannot_target_private_services() {
        for address in [
            "a@127.0.0.1",
            "a@localhost",
            "a@wallet.test/path",
            "a@wallet.test:80",
            "a@user@wallet.test",
        ] {
            let parsed = super::address(address);
            if let Ok(url) = parsed {
                assert!(
                    url.host_str() == Some("localhost")
                        || url.host_str().is_some_and(|s| s.parse::<IpAddr>().is_ok())
                );
            }
        }
        for ip in [
            "127.0.0.1",
            "10.0.0.2",
            "169.254.169.254",
            "100.64.0.1",
            "192.168.1.2",
            "::1",
            "fc00::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert!(public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(get(&url("https://127.0.0.1/").unwrap()).await.is_err());
        assert!(url("http://wallet.test").is_err());
        assert!(url("https://name:secret@wallet.test").is_err());
        assert!(url("https://wallet.test:444").is_err());
    }
}
