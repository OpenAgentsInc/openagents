//! Bounded authenticated SMTP submission over implicit TLS. No retry is automatic.
//! A final positive DATA reply means acceptance, never confirmed delivery.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_MIME: usize = 1024 * 1024;
const MAX_REPLY: usize = 8192;
const MAX_REPLY_LINES: usize = 32;
const TOTAL_SECONDS: u64 = 60;
const STEP_SECONDS: u64 = 15;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Authentication {
    Plain,
    Xoauth2,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub authentication: Authentication,
}
impl Config {
    pub(in crate::task::sales) fn check(&self) -> Result<()> {
        header(&self.server, 253)?;
        header(&self.username, 256)?;
        if !self.server.is_ascii()
            || !self.server.contains('.')
            || self.server.parse::<std::net::IpAddr>().is_ok()
            || self.server.split('.').any(|part| {
                part.is_empty()
                    || part.len() > 63
                    || part.starts_with('-')
                    || part.ends_with('-')
                    || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            || self.server.ends_with(".localhost")
            || self.server.ends_with(".local")
            || self.port != 465
            || !self.username.is_ascii()
        {
            return Err(
                "SMTP requires a declared public hostname and implicit TLS on port 465".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub message_sha256: String,
    pub delivery: Delivery,
    pub reply_code: Option<u16>,
    pub reference_sha256: String,
    pub tls: Validation,
    pub authentication: Validation,
}
impl Observation {
    fn new(
        mime: &[u8],
        delivery: Delivery,
        code: Option<u16>,
        reference: &[u8],
        tls: Validation,
        authentication: Validation,
    ) -> Self {
        Self {
            message_sha256: digest(mime),
            delivery,
            reply_code: code,
            reference_sha256: digest(reference),
            tls,
            authentication,
        }
    }
}
fn public_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 240
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
                || ip.octets()[..3] == [192, 0, 0]
                || ip.octets()[..3] == [192, 88, 99]
                || ip.octets()[0] == 198 && matches!(ip.octets()[1], 18 | 19))
        }
        std::net::IpAddr::V6(ip) => {
            // Require native global unicast; refuse special, translated, and tunnel ranges.
            // IANA: https://www.iana.org/assignments/iana-ipv6-special-registry
            let parts = ip.segments();
            parts[0] & 0xe000 == 0x2000
                && !(parts[0] == 0x2001 && (parts[1] < 0x200 || parts[1] == 0x0db8))
                && parts[0] != 0x2002
                && !(parts[0] == 0x3fff && parts[1] < 0x1000)
        }
    }
}
/// Only the native outbox can submit approved bytes. Credentials stay in this host.
pub(in crate::task::sales) async fn submit(
    config: &Config,
    sender: &str,
    recipient: &str,
    mime: &[u8],
    secret: &MailboxSecret,
    cancel: &AtomicBool,
    before_data: &mut dyn FnMut() -> Result<()>,
) -> Result<Observation> {
    config.check()?;
    if address(sender)? != sender || address(recipient)? != recipient {
        return Err("SMTP envelope identity is malformed".into());
    }
    validate_mime(mime)?;
    let missing = |delivery| {
        Observation::new(
            mime,
            delivery,
            None,
            b"unavailable",
            Validation::Unknown,
            Validation::Unknown,
        )
    };
    if cancel.load(Ordering::SeqCst) {
        return Ok(missing(Delivery::Cancelled));
    }
    let deadline = Instant::now() + Duration::from_secs(TOTAL_SECONDS);
    let addresses = match tokio::time::timeout(
        Duration::from_secs(STEP_SECONDS),
        tokio::net::lookup_host((config.server.as_str(), config.port)),
    )
    .await
    {
        Ok(Ok(found)) => found.take(9).collect::<Vec<_>>(),
        _ => return Ok(missing(Delivery::Failed)),
    };
    if addresses.is_empty()
        || addresses.len() > 8
        || addresses.iter().any(|a| !public_address(a.ip()))
    {
        return Err("SMTP server resolved outside the public provider boundary".into());
    }
    let approved_addresses = addresses.clone();
    let mut tcp = None;
    for target in addresses {
        if cancel.load(Ordering::SeqCst) {
            return Ok(missing(Delivery::Cancelled));
        }
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(STEP_SECONDS));
        if remaining.is_zero() {
            break;
        }
        if let Ok(Ok(stream)) =
            tokio::time::timeout(remaining, tokio::net::TcpStream::connect(target)).await
        {
            tcp = Some(stream);
            break;
        }
    }
    let Some(tcp) = tcp else {
        return Ok(missing(Delivery::Failed));
    };
    let peer = tcp
        .peer_addr()
        .map_err(|_| "SMTP peer identity is unavailable")?;
    if !public_address(peer.ip()) || !approved_addresses.contains(&peer) {
        return Err("SMTP peer differs from the approved public endpoint".into());
    }
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "SMTP TLS configuration is unavailable")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(config.server.clone())
        .map_err(|_| "SMTP TLS server name is unavailable")?;
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls));
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_secs(STEP_SECONDS));
    let mut stream = match tokio::time::timeout(remaining, connector.connect(name, tcp)).await {
        Ok(Ok(stream)) => stream,
        _ => {
            return Ok(Observation::new(
                mime,
                Delivery::AuthenticationFailed,
                None,
                b"TLS refused",
                Validation::Failed,
                Validation::Unknown,
            ));
        }
    };
    Ok(transaction(
        &mut stream,
        config,
        sender,
        recipient,
        mime,
        secret,
        cancel,
        deadline,
        before_data,
    )
    .await)
}
pub(in crate::task::sales) fn validate_mime(mime: &[u8]) -> Result<()> {
    if mime.is_empty()
        || mime.len() > MAX_MIME
        || !mime.is_ascii()
        || !mime.ends_with(b"\r\n")
        || !mime.windows(4).any(|p| p == b"\r\n\r\n")
        || mime
            .split(|b| *b == b'\n')
            .any(|line| line.len() > 999 || (!line.is_empty() && !line.ends_with(b"\r")))
        || mime
            .iter()
            .enumerate()
            .any(|(i, b)| *b == 0 || (*b == b'\r' && mime.get(i + 1) != Some(&b'\n')))
    {
        return Err("SMTP approved MIME is malformed or exceeds its bound".into());
    }
    Ok(())
}
struct Reply {
    code: u16,
    bytes: Vec<u8>,
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
async fn bounded<T>(
    future: impl std::future::Future<Output = std::io::Result<T>>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<T> {
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_secs(STEP_SECONDS));
    if remaining.is_zero() {
        return Err(std::io::ErrorKind::TimedOut.into());
    }
    let operation = tokio::time::timeout(remaining, future);
    tokio::pin!(operation);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        tokio::select! {
            result = &mut operation => return result.map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))?,
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }
}
async fn reply<S: AsyncRead + Unpin>(
    stream: &mut S,
    cancel: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<Reply> {
    let mut bytes = vec![];
    let mut code = None;
    for _ in 0..MAX_REPLY_LINES {
        let start = bytes.len();
        loop {
            if bytes.len() >= MAX_REPLY || bytes.len() - start >= 512 {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let mut b = [0];
            bounded(stream.read_exact(&mut b), cancel, deadline).await?;
            bytes.push(b[0]);
            if b[0] == b'\n' {
                break;
            }
        }
        let line = &bytes[start..];
        if line.len() < 6
            || !line.ends_with(b"\r\n")
            || !line[..3].iter().all(u8::is_ascii_digit)
            || !matches!(line[3], b' ' | b'-')
        {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let found = u16::from(line[0] - b'0') * 100
            + u16::from(line[1] - b'0') * 10
            + u16::from(line[2] - b'0');
        if code.is_some_and(|old| old != found) {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        code = Some(found);
        if line[3] == b' ' {
            return Ok(Reply { code: found, bytes });
        }
    }
    Err(std::io::ErrorKind::InvalidData.into())
}
async fn command<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    bytes: &[u8],
    cancel: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<Reply> {
    bounded(stream.write_all(bytes), cancel, deadline).await?;
    bounded(stream.flush(), cancel, deadline).await?;
    reply(stream, cancel, deadline).await
}
async fn transaction<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    config: &Config,
    sender: &str,
    recipient: &str,
    mime: &[u8],
    secret: &MailboxSecret,
    cancel: &AtomicBool,
    deadline: Instant,
    before_data: &mut dyn FnMut() -> Result<()>,
) -> Observation {
    let mut authentication = Validation::Unknown;
    let mut handed_off = false;
    let mut last_code = None;
    let result = async {
        let greeting = reply(stream, cancel, deadline).await?;
        last_code = Some(greeting.code);
        if greeting.code != 220 {
            return Ok((Delivery::Failed, greeting));
        }
        let domain = sender.split_once('@').map(|(_, d)| d).unwrap_or("invalid");
        let ehlo = command(
            stream,
            format!("EHLO {domain}\r\n").as_bytes(),
            cancel,
            deadline,
        )
        .await?;
        last_code = Some(ehlo.code);
        let advertised = String::from_utf8_lossy(&ehlo.bytes).to_ascii_uppercase();
        let mechanism = match config.authentication {
            Authentication::Plain => "PLAIN",
            Authentication::Xoauth2 => "XOAUTH2",
        };
        let supported = advertised.lines().any(|line| {
            line.get(4..).is_some_and(|line| {
                line.split_ascii_whitespace().next() == Some("AUTH")
                    && line
                        .split_ascii_whitespace()
                        .skip(1)
                        .any(|m| m == mechanism)
            })
        });
        if ehlo.code != 250 || !supported {
            authentication = Validation::Failed;
            return Ok((Delivery::AuthenticationFailed, ehlo));
        }
        if matches!(config.authentication, Authentication::Xoauth2) && secret.expose().contains(&1)
        {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        before_data().map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        let mut auth = match config.authentication {
            Authentication::Plain => {
                let mut bytes = vec![0];
                bytes.extend_from_slice(config.username.as_bytes());
                bytes.push(0);
                bytes.extend_from_slice(secret.expose());
                bytes
            }
            Authentication::Xoauth2 => {
                let mut bytes = format!("user={}\x01auth=Bearer ", config.username).into_bytes();
                bytes.extend_from_slice(secret.expose());
                bytes.extend_from_slice(b"\x01\x01");
                bytes
            }
        };
        let mut auth_command =
            format!("AUTH {mechanism} {}\r\n", STANDARD.encode(&auth)).into_bytes();
        auth.fill(0);
        let authorized = command(stream, &auth_command, cancel, deadline).await;
        auth_command.fill(0);
        let authorized = authorized?;
        last_code = Some(authorized.code);
        if authorized.code != 235 {
            authentication = Validation::Failed;
            return Ok((Delivery::AuthenticationFailed, authorized));
        }
        authentication = Validation::Passed;
        before_data().map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        let from = command(
            stream,
            format!("MAIL FROM:<{sender}>\r\n").as_bytes(),
            cancel,
            deadline,
        )
        .await?;
        last_code = Some(from.code);
        if from.code != 250 {
            return Ok((Delivery::Failed, from));
        }
        before_data().map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        let to = command(
            stream,
            format!("RCPT TO:<{recipient}>\r\n").as_bytes(),
            cancel,
            deadline,
        )
        .await?;
        last_code = Some(to.code);
        if !matches!(to.code, 250 | 251) {
            return Ok((
                if matches!(to.code, 550 | 551 | 553) {
                    Delivery::HardBounce
                } else {
                    Delivery::Failed
                },
                to,
            ));
        }
        let data = command(stream, b"DATA\r\n", cancel, deadline).await?;
        last_code = Some(data.code);
        if data.code != 354 {
            return Ok((Delivery::Failed, data));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
        }
        before_data().map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        // Once any body handoff starts, a lost acknowledgment is never safe to replay.
        handed_off = true;
        let mut wire = Vec::with_capacity(mime.len() + 16);
        for line in mime.split_inclusive(|b| *b == b'\n') {
            if line.starts_with(b".") {
                wire.push(b'.');
            }
            wire.extend_from_slice(line);
        }
        wire.extend_from_slice(b".\r\n");
        let written = bounded(stream.write_all(&wire), cancel, deadline).await;
        wire.fill(0);
        written?;
        bounded(stream.flush(), cancel, deadline).await?;
        let accepted = reply(stream, cancel, deadline).await?;
        last_code = Some(accepted.code);
        let delivery = if accepted.code == 250 {
            Delivery::Accepted
        } else if (400..600).contains(&accepted.code) {
            Delivery::Failed
        } else {
            Delivery::Unknown
        };
        Ok::<_, std::io::Error>((delivery, accepted))
    }
    .await;
    match result {
        Ok((delivery, response)) => Observation::new(
            mime,
            delivery,
            Some(response.code),
            &response.bytes,
            Validation::Passed,
            authentication,
        ),
        Err(_) => Observation::new(
            mime,
            if handed_off {
                Delivery::Unknown
            } else if cancel.load(Ordering::SeqCst) {
                Delivery::Cancelled
            } else {
                Delivery::Failed
            },
            last_code,
            b"SMTP observation unavailable",
            Validation::Passed,
            authentication,
        ),
    }
}

#[cfg(test)]
mod tests;
