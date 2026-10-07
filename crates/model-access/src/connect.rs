//! Connect OpenRouter: sign in at OpenRouter and get a key back, so nobody
//! copies or pastes one (BYOK, #10176, `docs/byok/2026-10-02-byok-openrouter.md`
//! section 3, "Easiest add").
//!
//! OpenRouter's OAuth PKCE flow:
//!
//! 1. Make a random verifier and its S256 challenge ([`Pkce`]).
//! 2. Listen on a loopback port ([`Callback`]) and open the browser at
//!    [`auth_url`]: `https://openrouter.ai/auth?callback_url=http://localhost:PORT/callback&code_challenge=…&code_challenge_method=S256&state=…`.
//! 3. The person signs in and approves; OpenRouter sends the browser to the
//!    callback with `?code=…&state=…` ([`read_callback`]).
//! 4. Trade the code and the verifier for a key at
//!    `POST https://openrouter.ai/api/v1/auth/keys` ([`exchange_body`],
//!    [`read_exchange`]).
//!
//! The key that comes back is the person's own, made under their account;
//! the caller then tests and keeps it exactly as a pasted one. Nothing here
//! prints or logs a key, a code, or the verifier.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::ApiKey;

/// OpenRouter's sign-in page.
pub const AUTH_URL: &str = "https://openrouter.ai/auth";
/// Where a code is traded for a key.
pub const KEYS_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
/// The callback's path on the loopback port.
pub const CALLBACK_PATH: &str = "/callback";
/// How long to wait for the person to finish signing in.
pub const WAIT: Duration = Duration::from_secs(300);
/// The label the new key gets at OpenRouter.
pub const KEY_LABEL: &str = "OpenAgents";

/// URL-safe base64 without padding (RFC 4648 section 5).
#[must_use]
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let take = chunk.len() + 1;
        for i in 0..take {
            out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// The S256 challenge of `verifier`: base64url of its SHA-256, no padding.
#[must_use]
pub fn challenge(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

fn random(n: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes)
        .map_err(|_| "This computer couldn't make a random code.".to_owned())?;
    Ok(base64url(&bytes))
}

/// One sign-in's verifier, its challenge, and the `state` that ties the
/// callback to this sign-in. `Debug` never shows the verifier.
#[derive(Clone)]
pub struct Pkce {
    verifier: String,
    challenge: String,
    state: String,
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce")
            .field("challenge", &self.challenge)
            .finish_non_exhaustive()
    }
}

impl Pkce {
    /// A fresh verifier (43 characters from 32 random bytes) and state.
    ///
    /// # Errors
    /// The operating system gave no randomness.
    pub fn new() -> Result<Self, String> {
        Ok(Self::from_parts(random(32)?, random(16)?))
    }

    /// From a known verifier and state (tests).
    #[must_use]
    pub fn from_parts(verifier: String, state: String) -> Self {
        let challenge = challenge(&verifier);
        Pkce {
            verifier,
            challenge,
            state,
        }
    }

    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(value) => {
                        out.push(value);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The sign-in page for `callback` and `pkce`.
#[must_use]
pub fn auth_url(callback: &str, pkce: &Pkce) -> String {
    format!(
        "{AUTH_URL}?callback_url={}&code_challenge={}&code_challenge_method=S256&state={}&key_label={}",
        percent_encode(callback),
        percent_encode(pkce.challenge()),
        percent_encode(pkce.state()),
        percent_encode(KEY_LABEL),
    )
}

/// What a request to the loopback port said.
#[derive(Clone, PartialEq, Eq)]
pub enum Reply {
    /// The code to trade for a key.
    Code(String),
    /// The person declined, or OpenRouter reported an error: its words.
    Declined(String),
    /// Not the callback (a favicon, a stray request); keep waiting.
    Other,
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reply::Code(_) => f.write_str("Code(***)"),
            Reply::Declined(why) => f.debug_tuple("Declined").field(why).finish(),
            Reply::Other => f.write_str("Other"),
        }
    }
}

/// Read a request line (`GET /callback?code=…&state=… HTTP/1.1`). A
/// callback whose `state` is not `state` is refused as declined, so
/// another page cannot hand this sign-in a code.
#[must_use]
pub fn read_callback(request_line: &str, state: &str) -> Reply {
    let mut parts = request_line.split_whitespace();
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Reply::Other;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != CALLBACK_PATH {
        return Reply::Other;
    }
    let mut code = None;
    let mut got_state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(value);
        match name {
            "code" => code = Some(value),
            "state" => got_state = Some(value),
            "error" | "error_description" if error.is_none() => error = Some(value),
            _ => {}
        }
    }
    if let Some(error) = error {
        return Reply::Declined(format!("OpenRouter said: {error}"));
    }
    if got_state.as_deref() != Some(state) {
        return Reply::Declined("That sign-in didn't come from this computer; try again.".into());
    }
    match code {
        Some(code) if !code.is_empty() => Reply::Code(code),
        _ => Reply::Declined("OpenRouter sent no code; try again.".into()),
    }
}

/// The body that trades `code` for a key.
#[must_use]
pub fn exchange_body(code: &str, pkce: &Pkce) -> Value {
    json!({
        "code": code,
        "code_verifier": pkce.verifier(),
        "code_challenge_method": "S256",
    })
}

/// Read the trade's answer: `{"key": "…"}` on success.
///
/// # Errors
/// One line a person reads.
pub fn read_exchange(status: Option<u16>, body: &[u8]) -> Result<ApiKey, String> {
    let Some(status) = status else {
        return Err("Couldn't reach OpenRouter; try again.".into());
    };
    if !(200..300).contains(&status) {
        return Err(match status {
            400 | 403 => "OpenRouter didn't accept that sign-in; try again.".into(),
            _ => format!("OpenRouter answered {status}; try again."),
        });
    }
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    match value.get("key").and_then(Value::as_str) {
        Some(key) if !key.trim().is_empty() => Ok(ApiKey::new(key)),
        _ => Err("OpenRouter sent no key; try again.".into()),
    }
}

/// The loopback listener the browser returns to.
#[derive(Debug)]
pub struct Callback {
    listener: TcpListener,
    port: u16,
}

const DONE_BODY: &str = "<h1>Connected</h1><p>OpenRouter is connected. You can close this tab and go back to OpenAgents.</p>";
const FAILED_BODY: &str = "<h1>Not connected</h1><p>OpenRouter wasn't connected. Go back to OpenAgents and try again.</p>";

/// A callback page in Paper Mono. The one-off local listener has no route
/// for the font, so the page carries it inline.
fn callback_page(body: &str) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>OpenAgents</title><style>{}body{{font-family:\"Paper Mono\",monospace;padding:3em}}</style><body>{body}",
        paper_mono::font_face_inline()
    )
}

fn respond(stream: &mut TcpStream, status: &str, page: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.flush();
}

impl Callback {
    /// Listen on a free loopback port.
    ///
    /// # Errors
    /// No port could be opened.
    pub fn bind() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|_| "Couldn't open a port on this computer for the sign-in.".to_owned())?;
        let port = listener
            .local_addr()
            .map_err(|_| "Couldn't open a port on this computer for the sign-in.".to_owned())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|_| "Couldn't open a port on this computer for the sign-in.".to_owned())?;
        Ok(Callback { listener, port })
    }

    /// The `callback_url` to give OpenRouter.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://localhost:{}{CALLBACK_PATH}", self.port)
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Wait up to `wait` for the callback; answer the browser with a page
    /// that says whether it worked. `cancelled` is asked between looks.
    ///
    /// # Errors
    /// Declined, timed out, or cancelled: one line.
    pub fn wait(
        &self,
        state: &str,
        wait: Duration,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, String> {
        let until = Instant::now() + wait;
        loop {
            if cancelled() {
                return Err("Connecting OpenRouter was cancelled.".into());
            }
            if Instant::now() >= until {
                return Err("OpenRouter sign-in took too long; try again.".into());
            }
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut line = String::new();
                    if BufReader::new(&stream).read_line(&mut line).is_err() {
                        continue;
                    }
                    match read_callback(line.trim_end(), state) {
                        Reply::Code(code) => {
                            respond(&mut stream, "200 OK", &callback_page(DONE_BODY));
                            return Ok(code);
                        }
                        Reply::Declined(why) => {
                            respond(&mut stream, "200 OK", &callback_page(FAILED_BODY));
                            return Err(why);
                        }
                        Reply::Other => respond(&mut stream, "404 Not Found", ""),
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

/// Trade `code` for a key at `url` (`KEYS_URL`, or a test server).
///
/// # Errors
/// One line a person reads.
#[cfg(feature = "http")]
pub fn exchange(url: &str, code: &str, pkce: &Pkce) -> Result<ApiKey, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "Couldn't reach OpenRouter; try again.".to_owned())?;
    match client.post(url).json(&exchange_body(code, pkce)).send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.bytes().map(|b| b.to_vec()).unwrap_or_default();
            read_exchange(Some(status), &body)
        }
        Err(_) => read_exchange(None, &[]),
    }
}

/// The whole sign-in: listen, call `open` with the sign-in page (it opens
/// the browser, or prints the link; its error ends the sign-in), wait for
/// the callback, and trade the code for a key. The caller tests and keeps
/// the key.
///
/// # Errors
/// One line a person reads.
#[cfg(feature = "http")]
pub fn connect(
    open: &dyn Fn(&str) -> Result<(), String>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ApiKey, String> {
    let pkce = Pkce::new()?;
    let callback = Callback::bind()?;
    open(&auth_url(&callback.url(), &pkce))?;
    let code = callback.wait(pkce.state(), WAIT, cancelled)?;
    exchange(KEYS_URL, &code, &pkce)
}

/// Open `url` in the person's browser; false when no opener ran.
#[must_use]
pub fn open_browser(url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    spawned.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_s256_of_the_verifier() {
        // RFC 7636 appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        let pkce = Pkce::new().unwrap();
        assert_eq!(pkce.verifier().len(), 43);
        assert_eq!(pkce.challenge(), challenge(pkce.verifier()));
        assert_ne!(pkce.verifier(), Pkce::new().unwrap().verifier());
        assert!(!format!("{pkce:?}").contains(pkce.verifier()));
    }

    #[test]
    fn the_sign_in_page_carries_the_callback_and_challenge() {
        let pkce = Pkce::from_parts("v".repeat(43), "st".into());
        let url = auth_url("http://localhost:4567/callback", &pkce);
        assert!(url.starts_with(
            "https://openrouter.ai/auth?callback_url=http%3A%2F%2Flocalhost%3A4567%2Fcallback&"
        ));
        assert!(url.contains(&format!("code_challenge={}", pkce.challenge())));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=st"));
        assert!(!url.contains(pkce.verifier()));
    }

    #[test]
    fn a_callback_reads_as_a_code_only_with_this_state() {
        assert_eq!(
            read_callback("GET /callback?code=abc%2Fd&state=s1 HTTP/1.1", "s1"),
            Reply::Code("abc/d".into())
        );
        assert!(matches!(
            read_callback("GET /callback?code=abc&state=other HTTP/1.1", "s1"),
            Reply::Declined(_)
        ));
        assert!(matches!(
            read_callback("GET /callback?error=access_denied&state=s1 HTTP/1.1", "s1"),
            Reply::Declined(why) if why.contains("access_denied")
        ));
        assert_eq!(
            read_callback("GET /favicon.ico HTTP/1.1", "s1"),
            Reply::Other
        );
        assert_eq!(
            read_callback("POST /callback?code=a&state=s1 HTTP/1.1", "s1"),
            Reply::Other
        );
        assert!(!format!("{:?}", Reply::Code("secret".into())).contains("secret"));
    }

    #[test]
    fn the_exchange_answer_reads_as_a_key_or_a_line() {
        let key = read_exchange(Some(200), br#"{"key":"sk-or-v1-abc"}"#).unwrap();
        assert_eq!(key.expose(), "sk-or-v1-abc");
        assert_eq!(
            read_exchange(Some(400), b"{}").unwrap_err(),
            "OpenRouter didn't accept that sign-in; try again."
        );
        assert!(read_exchange(Some(200), b"{}").is_err());
        assert!(read_exchange(None, b"").is_err());
        let pkce = Pkce::from_parts("ver".into(), "s".into());
        assert_eq!(
            exchange_body("c", &pkce),
            json!({"code": "c", "code_verifier": "ver", "code_challenge_method": "S256"})
        );
    }

    #[test]
    fn the_loopback_port_hands_back_the_code() {
        let callback = Callback::bind().unwrap();
        let port = callback.port();
        assert_eq!(callback.url(), format!("http://localhost:{port}/callback"));
        let browser = std::thread::spawn(move || {
            use std::io::Read;
            // A stray request first, then the callback.
            for target in ["/favicon.ico", "/callback?code=the-code&state=s9"] {
                let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
                write!(stream, "GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
                let mut page = String::new();
                let _ = stream.read_to_string(&mut page);
                if target.starts_with("/callback") {
                    assert!(page.contains("Connected"), "{page}");
                }
            }
        });
        let code = callback
            .wait("s9", Duration::from_secs(10), &|| false)
            .unwrap();
        assert_eq!(code, "the-code");
        browser.join().unwrap();
        let cancelled = Callback::bind()
            .unwrap()
            .wait("s", Duration::from_secs(10), &|| true);
        assert_eq!(
            cancelled.unwrap_err(),
            "Connecting OpenRouter was cancelled."
        );
    }

    /// The trade against a local server that checks the body.
    #[cfg(feature = "http")]
    #[test]
    fn the_exchange_trades_the_code_and_verifier_for_a_key() {
        use std::io::Read;
        let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let pkce = Pkce::from_parts("the-verifier".into(), "s".into());
        let handle = std::thread::spawn(move || {
            for answer in [r#"{"key":"sk-or-v1-fresh"}"#, "{}"] {
                let (mut stream, _) = server.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0usize;
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                assert!(first.starts_with("POST /api/v1/auth/keys "), "{first}");
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(body["code"], "the-code");
                assert_eq!(body["code_verifier"], "the-verifier");
                assert_eq!(body["code_challenge_method"], "S256");
                let status = if answer == "{}" {
                    "400 Bad Request"
                } else {
                    "200 OK"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                    answer.len()
                )
                .unwrap();
            }
        });
        let url = format!("http://127.0.0.1:{port}/api/v1/auth/keys");
        let key = exchange(&url, "the-code", &pkce).unwrap();
        assert_eq!(key.expose(), "sk-or-v1-fresh");
        assert_eq!(
            exchange(&url, "the-code", &pkce).unwrap_err(),
            "OpenRouter didn't accept that sign-in; try again."
        );
        handle.join().unwrap();
    }
}
